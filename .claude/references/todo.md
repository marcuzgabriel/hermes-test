# TODO — Future Work

## v1: Coverage reporting

### Coverage instrumentation
- **Approach**: Istanbul-style instrumentation at bundle time
- **Options explored**:
  - `esbuild-plugin-istanbul` — requires esbuild JS API (we use CLI)
  - Post-bundle AST instrumentation — needs full parser (OXC or SWC)
  - Rust regex-based — too fragile for statement/branch coverage
  - `c8` / v8 coverage — not available in Hermes
- **Recommended**: `swc-coverage-instrument` Rust crate — pure Rust, Istanbul-compatible,
  no Node dependency. Uses SWC's parser + a coverage visitor to instrument each source file.
  Generates `__coverage__` global matching Istanbul's FileCoverage format.
- **Alternative**: OXC (already a dep) but no existing coverage visitor — would need to write from scratch
- **Output formats**: lcov (for CI), cobertura (for Sonar), html (for local dev)
- **Reporting**: `istanbul-lib-report` (Node) or implement lcov output in Rust (simple text format)
- **Performance impact**: +0.3-0.5s bundle time (instrument once, cached with bytecode)
- **Config**: `--coverage` CLI flag, `"coverage": true` in hermes-test.config.json
- **Source maps**: esbuild `--sourcemap` to map instrumented positions back to original source
- **Important**: until coverage is implemented, benchmark claims must compare against Jest `--no-coverage` (29x, not 147x)
- **Current state**: prototype works on small bundles, 537/1054 on large (bare-body insertion issues)
- **Next approach**: OXC VisitMut + codegen (modify AST directly, re-emit clean JS) OR per-file instrumentation before esbuild (like Vitest's onFileTransform with istanbul-lib-instrument)
- **Key insight from Vitest**: they instrument per-file during transform phase, NOT post-bundle

## v1: Component rendering + Jest compat

### 1. `render(<Component />)` API
- Reconciler already builds a real React tree (hooks.ts)
- hostConfig needs to track `{ type, props, children }` for queries
- Query API: `getByText()`, `getByTestId()`, `getByRole()`
- Event simulation: `fireEvent.press()`, `fireEvent.changeText()`
- Snapshot testing: serialize host tree to JSON
- Based on `mdjastrzebski/test-renderer` (universal-test-renderer for React 19)

### 2. Jest compatibility shim
- `globalThis.jest = { fn: spy, mock: mock, spyOn, ... }`
- `jest.requireActual(path)` — return real module
- `jest.useFakeTimers()` / `jest.useRealTimers()`
- `jest.clearAllMocks()` → `clearAllMocks()`
- Enables reuse of library-shipped `__mocks__/` files without rewriting

### 3. Library mock support
- expo-router: `testing-library/mocks.js` uses `jest.mock()` + `jest.requireActual()`
- expo-font: `jest/expo-font.js`
- react-native-reanimated: `mock.js`
- zustand: `__mocks__/zustand.ts`
- Most only need `jest.fn()` → `spy()` mapping
- `expect.extend()` for custom matchers (expo-router pathname matchers)

### 4. `setupFiles` config option
- Load user-specified files before tests (like Jest's `setupFilesAfterFramework`)
- Would enable: `"setupFiles": ["expo-router/build/testing-library/mocks"]`

## v1.1: DX improvements

### 5. Terminal beautification
- Progress spinner during eval: `⠋ Running tests... 847/1472`
- Better summary: `✓ 1472 tests passed (259 files) · 0.79s · 1,862 tests/sec`
- Watch mode banner with keybindings

### 6. Extract shared Proxy generation
- Shadow wrappers, package shims, wrapper shims all generate near-identical Proxy code
- ~150 lines Rust duplication → single `fn generate_proxy_wrapper()` function

### 7. Error diagnostics
- Warn when `mock()` path doesn't match any shadow wrapper
- Suggest fixes for common errors (missing Provider, relative import bypass)
