# TODO — Future Work

## v1: Coverage reporting

### Coverage instrumentation
- **Approach**: Istanbul-style instrumentation at bundle time
- **Options explored**:
  - `esbuild-plugin-istanbul` — requires esbuild JS API (we use CLI)
  - Post-bundle AST instrumentation — needs full parser (OXC or SWC)
  - Rust regex-based — too fragile for statement/branch coverage
  - `c8` / v8 coverage — not available in Hermes
- **Recommended**: Use OXC (already a dependency) to parse the bundle, insert Istanbul-compatible
  counters (`__coverage__[fileId][statementId]++`), then output lcov/cobertura after test execution
- **Output formats**: lcov (for CI), cobertura (for Sonar), html (for local dev)
- **Performance impact**: +0.3-0.5s bundle time (instrument once, cached with bytecode)
- **Config**: `"coverage": true` in hermes-test.config.json, `--coverage` CLI flag
- **Source maps**: esbuild `--sourcemap` to map instrumented positions back to original source

## v1: Component rendering + Jest compat

### 1. `render(<Component />)` API
- Reconciler already builds a real React tree (hooks.ts)
- hostConfig needs to track `{ type, props, children }` for queries
- Query API: `getByText()`, `getByTestId()`, `getByRole()`
- Event simulation: `fireEvent.press()`, `fireEvent.changeText()`
- Snapshot testing: serialize host tree to JSON
- Based on `mdjastrzebski/test-renderer` (universal-test-renderer for React 19)

### 2. Jest compatibility shim
- `globalThis.jest = { fn: spy, mock: mockModule, spyOn, ... }`
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
- Warn when `mockModule` path doesn't match any shadow wrapper
- Suggest fixes for common errors (missing Provider, relative import bypass)
