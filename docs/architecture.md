# hermes-test — Full Application Architecture

```mermaid
flowchart TB
    subgraph USER["User's Project"]
        TEST["*.test.ts files"]
        SRC["Source files (hooks, utils)"]
        NM["node_modules/\n(react, axios, date-fns, etc.)"]
    end

    subgraph CLI["Rust CLI (hermes-test-cli)"]
        PARSE["Parse CLI args\n(clap)"]
        DISCOVER["Discover test files\n(walk *.test.ts)"]
        ENTRY["Generate entry file\n(.hermes-test-entry.js)"]
        CHOOSE{Bundler?}
    end

    subgraph BUNDLE["Bundling Phase (~7ms esbuild / ~470ms Metro)"]
        subgraph ESBUILD["esbuild (default)"]
            EB_RESOLVE["Resolve full import graph\n(source + node_modules)"]
            EB_TRANSFORM["Transform TS/JSX → JS"]
            EB_BUNDLE["Bundle into single IIFE"]
            EB_PATCH["Post-bundle transforms\n• hoist ht.mock() calls\n• inject native module require shim"]
        end
        subgraph METRO["Metro (--bundler metro)"]
            M_CONFIG["Load metro.config.js"]
            M_BUILD["runBuild()\n• Babel transform\n• RN-specific resolution\n• .ios.ts / .native.ts"]
        end
    end

    subgraph ENTRY_FILE["Generated Entry File"]
        direction TB
        BOOTSTRAP["globalThis.__React = require('react')\nglobalThis.__ReactTestRenderer = require('react-test-renderer')"]
        IMPORTS["require('./src/useAuth.test.ts')\nrequire('./src/utils.test.ts')\n..."]
        RUN["globalThis.__metroTest.runTests()\n→ stash results on globalThis.__metroTestResults"]
    end

    subgraph HERMES["Hermes Runtime"]
        direction TB
        POLYFILLS["Phase 0: Polyfills\n(process.env, setTimeout,\nsetImmediate, console)"]
        HARNESS["Phase 1: Eval harness.bundle.js (16KB)\n→ globalThis.__metroTest =\n{ test, expect, spy, renderHook,\n  act, waitFor, useMock, runTests }"]
        EVAL_BUNDLE["Phase 2: Eval test bundle\n→ React loaded onto globalThis\n→ tests registered via test()\n→ runTests() executes all"]
        RESULTS["Phase 3: Read results\n→ eval('globalThis.__metroTestResults')"]
    end

    subgraph REACT_BRIDGE["React Singleton Bridge"]
        direction LR
        GLOBAL_R["globalThis.__React"]
        HARNESS_READ["harness hooks.ts\ngetReact() reads global"]
        USER_CODE["User's hooks\nimport { useState } from 'react'"]
        SAME["Same React instance\n(hooks work correctly)"]
    end

    subgraph OUTPUT["Output"]
        PRETTY["Pretty Reporter\n(stderr, colored)"]
        JSON_OUT["JSON Results\n(stdout)"]
        EXIT["Exit code\n0 = pass, 1 = fail"]
    end

    TEST --> PARSE
    PARSE --> DISCOVER
    DISCOVER --> ENTRY
    ENTRY --> CHOOSE

    CHOOSE -->|"--bundler esbuild\n(default)"| EB_RESOLVE
    CHOOSE -->|"--bundler metro"| M_CONFIG

    EB_RESOLVE --> EB_TRANSFORM
    SRC --> EB_RESOLVE
    NM --> EB_RESOLVE
    EB_TRANSFORM --> EB_BUNDLE
    EB_BUNDLE --> EB_PATCH

    M_CONFIG --> M_BUILD
    SRC --> M_BUILD
    NM --> M_BUILD

    ENTRY_FILE -.-> EB_RESOLVE
    ENTRY_FILE -.-> M_BUILD

    EB_PATCH --> POLYFILLS
    M_BUILD --> POLYFILLS

    POLYFILLS --> HARNESS
    HARNESS --> EVAL_BUNDLE
    EVAL_BUNDLE --> RESULTS

    BOOTSTRAP --> GLOBAL_R
    GLOBAL_R --> HARNESS_READ
    GLOBAL_R --> USER_CODE
    HARNESS_READ --> SAME
    USER_CODE --> SAME

    RESULTS --> PRETTY
    RESULTS --> JSON_OUT
    RESULTS --> EXIT
```

## Execution Flow

```mermaid
sequenceDiagram
    participant User as User
    participant CLI as Rust CLI
    participant FS as File System
    participant ESB as esbuild
    participant Hermes as Hermes Engine

    User->>CLI: hermes-test run --root .
    CLI->>FS: Walk for *.test.ts
    FS-->>CLI: [auth.test.ts, utils.test.ts]

    Note over CLI: Generate entry file

    CLI->>FS: Write .hermes-test-entry.js
    CLI->>ESB: Bundle entry (7ms)

    Note over ESB: Resolves FULL import graph:<br/>user source + node_modules<br/>TS → JS transform<br/>Single IIFE output

    ESB-->>CLI: bundle.js (all deps inlined)

    Note over CLI: Patch for Hermes

    CLI->>Hermes: Create runtime
    CLI->>Hermes: Eval harness.bundle.js (16KB)

    Note over Hermes: globalThis.__metroTest ready

    CLI->>Hermes: Eval bundle.js

    Note over Hermes: 1. React → globalThis.__React<br/>2. Tests register via test()<br/>3. runTests() executes all<br/>4. Results → globalThis.__metroTestResults

    CLI->>Hermes: Eval 'globalThis.__metroTestResults'
    Hermes-->>CLI: JSON results

    CLI-->>User: ✓ 8 passed, 8 total
```

## Mock System — Shadow Wrappers & Barrel Delegation

```mermaid
flowchart TB
    subgraph TEST["Test File"]
        MOCK["mockModule('@scope/hooks/errorHandling/useErrorHandling', factory)"]
        IMPORT["import { useMyHook } from '@scope/hooks'"]
    end

    subgraph SHADOW["Shadow Wrapper System (bundler.rs)"]
        direction TB
        SCAN["Scan test files for mockModule() calls"]
        CREATE["Create shadow directory mirroring alias target"]

        subgraph FILES["Shadow Tree"]
            PROXY["Mocked files → Proxy wrapper\n(checks __HT_file_mocks at access time)"]
            BARREL["Barrel index.ts → Proxy with sub-path delegation\n(scans all sub-path mocks for property access)"]
            COPY["Sibling files → Copied\n(relative imports resolve in shadow tree)"]
            SYMLINK["Other files → Symlinked\n(no mocked neighbors)"]
        end
    end

    subgraph RUNTIME["Runtime Mock Resolution"]
        direction TB
        ACCESS["hook accesses useErrorHandling"]
        BARREL_PROXY["Barrel Proxy get trap fires"]
        CHECK1["Check __HT_file_mocks[file]['@scope/hooks']"]
        CHECK2["Scan all keys starting with '@scope/hooks/'\nFind '@scope/hooks/errorHandling/useErrorHandling'"]
        FOUND["Return mock.useErrorHandling"]
        FALLBACK["No mock → return _getReal()[prop]"]
    end

    MOCK --> SCAN
    SCAN --> CREATE
    CREATE --> FILES
    IMPORT --> BARREL_PROXY
    BARREL_PROXY --> CHECK1
    CHECK1 -->|"not found"| CHECK2
    CHECK2 -->|"found"| FOUND
    CHECK2 -->|"not found"| FALLBACK

    style PROXY fill:#c44,color:#fff
    style BARREL fill:#e80,color:#fff
    style COPY fill:#48a,color:#fff
    style SYMLINK fill:#4a9,color:#fff
```

### Key Design Decisions

1. **Shadow wrappers over AST transforms** — No source modification, works with any bundler output
2. **Barrel sub-path delegation** — Barrel Proxy checks ALL registered sub-path mocks, not just exact path match. Solves esbuild's barrel re-export inlining.
3. **Copy barrel files, symlink the rest** — esbuild resolves symlinks to real paths. Copied barrels keep relative imports within the shadow tree. Only barrels + files with mocked siblings are copied.
4. **Lazy loading via `_getReal()`** — Real module only loaded on first property access, avoiding circular dependency crashes.

### withStore / withApiStore (Redux Provider)

```mermaid
flowchart LR
    subgraph PLAIN["withStore (plain)"]
        PS["Identity reducer\n+ __SET_STATE__ / __PATCH__"]
        PP["Provider wraps renderHook"]
    end

    subgraph API["withApiStore (RTK Query)"]
        AS["Per-key reducers\n+ RTK api.reducer + cms.reducer"]
        AM["RTK middleware\n(api.middleware + cms.middleware)"]
        AP["Provider wraps renderHook"]
    end

    PS --> PP
    AS --> AM --> AP
```

### Polyfills (polyfills.js)

Hermes lacks several Web/Node APIs. Polyfills injected via esbuild banner:
- `process.env.NODE_ENV` — React needs this at load time
- `process.nextTick` — Node-style async patterns
- `crypto.getRandomValues` — uuid and crypto-dependent libs
- `MessageChannel` — React 19 scheduler
- `setTimeout/setImmediate` — Timer polyfills
- `AbortController/Headers/URL/URLSearchParams/Request/Response/fetch` — Web APIs for RTK Query

## Dependency Resolution

```mermaid
flowchart LR
    subgraph ENTRY["Generated Entry"]
        E1["require('react') → globalThis"]
        E2["require('./auth.test.ts')"]
    end

    subgraph TEST1["auth.test.ts"]
        T1_HOOK["import { useAuth } from './useAuth'"]
        T1_MT["const { test, renderHook } = globalThis.__metroTest"]
    end

    subgraph HOOK["useAuth.ts"]
        H1["import { useState } from 'react'"]
        H2["import { fetchUser } from './api'"]
        H3["import jwtDecode from 'jwt-decode'"]
    end

    subgraph API["api.ts"]
        A1["import axios from 'axios'"]
    end

    subgraph NM["node_modules — bundled automatically"]
        REACT["react"]
        JWT["jwt-decode"]
        AXIOS["axios"]
    end

    subgraph MOCK["No runtime API — mock with spy()"]
        FETCH["fetch / XMLHttpRequest"]
        NATIVE["RN native modules"]
    end

    E2 --> TEST1
    T1_HOOK --> HOOK
    H1 --> REACT
    H2 --> API
    H3 --> JWT
    A1 --> AXIOS

    AXIOS -. "uses at runtime" .-> FETCH
    HOOK -. "may import" .-> NATIVE

    style REACT fill:#4a9,color:#fff
    style MOCK fill:#c44,color:#fff
    style NM fill:#48a,color:#fff
```
