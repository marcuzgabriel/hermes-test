# Challenges & Solutions — Full Journey

## Day 1-3: Hermes Embed + esbuild Integration
### Challenge: Getting Hermes to run JS from Rust
- Built C++ bridge linking Hermes static library via cc crate
- JSI (JavaScript Interface) for eval, bytecode compilation
- Hermes stderr suppression for clean test output

### Challenge: esbuild output incompatible with Hermes
- Hermes doesn't support `for (let key of arr)` with arrow closure captures (bug)
- Hermes configurable property descriptors differ from V8
- Hermes `__toESM` early return behavior differs
- **Solution**: `patch_esbuild_for_hermes()` — 4 regex-based post-bundle patches

## Day 4-6: Hooks, Mocks, React Integration
### Challenge: renderHook without React Testing Library
- Built custom renderHook using React's internal act() mechanism
- Synchronous hook rendering with state history tracking
- waitFor/flushAsync for async hook patterns

### Challenge: mockModule timing
- esbuild hoists imports above mockModule calls
- `__require` Proxy with lazy property access solves this for externalized modules
- Named import destructuring captured at init time → Proxy intercepts at access time

### Challenge: __HT_noop for unknown externals
- Externalized modules return undefined → crashes on property access
- Proxy-based noop: infinite chaining (`noop.foo().bar.baz()`)
- Special handling: Symbol.toPrimitive, toJSON, ownKeys, constructor, length

## Day 7-8: Real-World Monorepo (Topdanmark)
### Challenge: tsconfig path aliases in monorepos
- `@topdanmark/mobile-insurance-app` → `./src` via tsconfig paths
- Followed "extends" chain to resolve parent tsconfig paths
- Alias-vs-external conflict: mocked paths must stay external

### Challenge: Zod v4 class-extends crash
- Hermes TDZ bug: `class X extends Variable` crashes when Variable is local
- esbuild `--target=hermes0.12.0` can't downlevel classes yet
- SWC Rust crates evaluated but rejected (thread-locals, require('@swc/helpers'))
- **Solution**: `fix_all_class_extends()` — regex-based Reflect.construct transform
  Handles 3 patterns, $-prefixed identifiers, paren-matching for super() args

### Challenge: Scoped lifecycle hooks
- beforeEach from nested group contaminating sibling groups
- **Solution**: hooks store their group scope, only ancestor hooks execute

## Day 9: Mock Conflicts Between Test Files
### Challenge: Multiple files mocking same module differently
- useChooseProfile and useServicePills both mock `@topdanmark/.../hooks`
- Single bundle: last mockModule wins, other files get wrong mock
- **Interim solution**: per-file isolation (separate esbuild + Hermes per file)
- Cost: ~120ms overhead per isolated file

### Challenge: Identifying which files need isolation
- `has_alias_mock_conflict()`: checks if file's mockModule paths match any alias
- Partition into batch (fast, shared bundle) and isolated (separate bundles)

## Day 10: The Single-Bundle Quest
### Challenge: Parallel esbuild for isolated files
- Sequential: 5 files x 80ms = 400ms bundling
- **Solution**: `std::thread::spawn` all esbuild processes, unique entry paths
- Saved ~120ms (0.73s → 0.60s for 5 isolated files)

### Challenge: Single-bundle for ALL files (the big goal)
#### Attempt 1: Post-bundle mock hoisting
- Move mockModule() before init_*() in bundled output
- Rust regex doesn't support lookahead
- Unnecessary anyway: __require Proxy already handles late-binding
- **Result**: dead end for externalized mocks

#### Attempt 2: OXC AST pre-transform (Vitest-style)
- Added OXC 0.132 as Rust dependency
- Parser + semantic analysis to find all import references
- Rewrote test file imports to Proxy wrappers
- **Problem**: only transforms test file, not transitive dependencies
- Hook implementations still get real module → "react-redux context" error
- **Result**: correct for test's own imports, useless for transitive deps

#### Attempt 3: Externalize alias-resolved paths
- Keep alias active, externalize the resolved path (e.g., /abs/src/hooks)
- esbuild: alias runs before external check → resolved path not caught
- Alternative: externalize everything under alias → breaks non-mock consumers
- **Result**: fundamental esbuild limitation

#### Attempt 4: Shadow wrappers (SUCCESS)
- User idea: "create a mock copy per file, use filename as identifier"
- Implementation: shadow directory tree with symlinks + Proxy wrapper files
- First try: `require(absolute_path)` → duplicate modules, circular crash
- Second try: relative paths → still duplicated by esbuild
- Third try: `@__ht_real/` alias for wrappers → same bundled instance
- Fourth try: lazy `_getReal()` → avoids circular dependency crashes
- **Result**: 91/94 at 0.49s (was 0.89s). 45% faster. Single bundle.

### Challenge: __DEV__ global missing
- createLogger accesses `__DEV__` which doesn't exist in Hermes test env
- **Solution**: `globalThis.__DEV__ = false` in entry generator

### Challenge: Index file matching
- Mock path `@scope/hooks` must match file `hooks/index.ts`
- Code generates `@scope/hooks/index` which doesn't match
- **Solution**: check both direct path AND parent-directory-for-index-files

## Day 19: Split Mode + Remaining 143 Failures

### Challenge: Split mode broken (233/1115)
- `bundle_split` creates vendor + group bundles
- Group bundles skip aliases when mock sub-paths exist (line 326-328 in bundler.rs)
  → all aliased imports become `__require` calls hitting empty `{}` placeholders
- Vendor bundle ALSO skipped aliases (same logic) → `require('@topdanmark/...')` in
  vendor hit local `__require` → circular call → stack overflow (87 crashes)
- **Solution**: vendor passes empty `mock_modules` so aliases resolve and real source is
  bundled. Group bundles use shadow wrappers (same as single-bundle) with only non-aliased
  mocks externalized. Shadow dirs cleaned up after split bundling.
- **Result**: 233 → 1315 passed. Stack overflows eliminated.

### Challenge: deepEqual treats all Dates as equal
- `Object.keys(new Date())` returns `[]` — zero own properties
- `deepEqual` compared objects by iterating `Object.keys`, so any two Dates were "equal"
- **Solution**: `if (a instanceof Date && b instanceof Date) return a.getTime() === b.getTime()`

### Challenge: 143 remaining failures (two categories)
**39 tests fail even alone (standalone bugs):**
- Relative imports (`../redux/useRedux`) bypass shadow wrappers entirely — esbuild resolves
  them to the real source via shared top-level vars, no Proxy interception possible
- Complex async (apiBaseQuery RTK middleware), UI dep crashes (useFormCoordinator)
- Known fix for relative imports: convert test files to `withStore` pattern (proven for 21 tests)

**~104 tests pass alone, fail in suite ("undefined is not a function"):**
- `find_mock_modules` scans ALL test files → `mockModule('X')` in ANY file causes `X` to be
  externalized globally → files that don't mock `X` get `__HT_noop` instead of real module
- `__HT_noop` Proxy swallows errors: returns `0` for valueOf, truthy for property access,
  functions return noop — tests get garbage values instead of crashes
- Split mode vendor fixes this for npm packages (bundles real implementations)
- Still affects non-aliased packages that can't be resolved by the vendor

### Strategies tried and ruled out (Day 19):
1. **FileContext** (per-file harness state) — zero impact, contamination isn't from hooks
2. **Mock vendor** (pre-bundle non-aliased packages) — fixed 1 test, redundant with split vendor
3. **Auto-split mode** — worse (368/1458), split has own issues
4. **Package ESM wrappers** — esbuild requires static named exports
5. **--preserve-symlinks** — breaks monorepo node_modules resolution
6. **Shadow tree full copy** — zero impact, symlinks weren't the cause

## Patterns & Principles Learned

1. **esbuild aliases run before externals** — can't selectively externalize aliased paths
2. **ESM named exports are static bindings** — can't intercept with Proxy at import time
3. **Proxy + lazy loading = mock isolation** — one module appears different to different consumers
4. **Filesystem-level interception > AST transforms** — simpler, no codegen, no edge cases
5. **Circular deps need lazy wrappers** — eager require in wrapper deadlocks module init
6. **Hermes compat needs post-bundle patches** — can't rely on esbuild/SWC class transforms
7. **Regex transforms are fragile but fast** — OXC for correctness, regex for performance
8. **Per-file isolation is the fallback** — parallel esbuild makes it acceptable (~60ms/file)
