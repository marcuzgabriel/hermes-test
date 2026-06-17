# Component Rendering — Status

## What's Built (feat/component-rendering branch)

### render() + queries
- `render(<Component />)` via react-reconciler, version-agnostic (works with any React 19.x)
- Queries: `getByText`, `getByTestId`, `getByProps`, `getByType` (get/getAll/query/queryAll variants)
- `fireEvent.press()`, `fireEvent.changeText()`, `fireEvent.scroll()`, generic `fireEvent()`
- `toJSON()` / `toTree()` for serialization
- `rerender()` / `unmount()`

### Element matchers
- `toBeRendered()`, `toHaveTextContent()`, `toContainElement()`, `toBeEmpty()`
- `toHaveDisplayValue()`, `toHaveProp()`, `toHaveStyle()`
- `toBeEnabled()` / `toBeDisabled()`, `toBeVisible()`

### Snapshot testing
- `toMatchSnapshot()` — writes/compares `__snapshots__/<file>.snap` (JSON format)
- `--update-snapshots` CLI flag
- `__HT_readFile` / `__HT_writeFile` host functions in Hermes C++ bridge

### Version-agnostic react-reconciler
- Reconciler removed from harness bundle (400K → 66K)
- CLI auto-resolves from user's node_modules via `find_react_reconciler()` + esbuild `--alias`
- react-reconciler moved to hermes-test dependencies (installed with package)

## ht.shallow() — Auto-Mock Child Components

### How it works
`ht.shallow('../index')` scans the component file for `<ComponentName>` in JSX, matches
against imports, and auto-generates `ht.mock()` calls. Child components become stubs:
`(props) => React.createElement('Name', props)`.

```tsx
ht.shallow('../index');  // auto-mocks all JSX child components

// Only override what needs specific values:
ht.mock('@scope/hooks', () => ({ useTheme: () => ({...}) }));
ht.mock('@scope/helpers/popup', () => ({ default: { presentInfoPopup: spy() } }));
```

### What it gives you
- `getByType('InsuranceCard')` — find by component name
- `toHaveProp('insurance', ...)` — assert props passed to children
- `fireEvent.press(getByType('CTAButton'))` — fire events on child nodes
- No manual child component mocking needed (4 mocks vs 13 in Jest)

### Implementation
- `scan_shallow_auto_mocks()` in entry.rs — scans `<Name>` in JSX, matches against imports
- Auto-generates `ht.mock()` with Proxy stubs in entry, before test file loads
- Mock hoisting moves them before `require()` → shadow wrappers return stubs
- Works in single-bundle, split, and watch mode

## act() — RTL Pattern for IS_REACT_ACT_ENVIRONMENT

### The problem
React warns "not wrapped in act()" when setState is called while
`IS_REACT_ACT_ENVIRONMENT === true` but outside an active `act()` scope.
Hooks with `useEffect → dispatch → promise → setState` trigger this because
the promise resolves via `drain()` after `reactAct()` returns.

### The solution (from React Testing Library's act-compat.js)
```
Default:  IS_REACT_ACT_ENVIRONMENT = false

act(() => {
  IS_REACT_ACT_ENVIRONMENT = true     // set before reactAct()
  reactAct(() => { callback() })      // React processes render + effects
  IS_REACT_ACT_ENVIRONMENT = false    // restored BEFORE flush()
  flush()                              // drains microtasks (no warning)
})

After:    IS_REACT_ACT_ENVIRONMENT = false
```

Key: restore to `false` BEFORE `flush()`. Async effects resolved by flush()
see `false` → React doesn't warn. Result: 82 act() warnings → 0.

### Files
- `hooks.ts` — act() with RTL-pattern IS_REACT_ACT_ENVIRONMENT toggle
- `entry.rs` — removed IS_REACT_ACT_ENVIRONMENT=true (harness manages it)
- `esbuild.rs` — removed from vendor entry too

## Console.log Support

### How it works
- Harness replaces `console` methods with `print()`-based output (Hermes native stdout)
- Output appears immediately in all modes (single, split, watch)
- `__HT_mocks['console'] = globalThis.console` in vendor entry for split mode
- Pretty-printed JSON: `JSON.stringify(obj, null, 2)`
- Filters "Expected host context" noise from react-reconciler

### Why print() not console
- Hermes's native `console.log` writes to stderr (suppressed during test execution)
- `--external:console` caused esbuild to treat console as a module (`__require("console")`)
- `print()` writes directly to stdout, bypasses all bundler/runtime issues

## Error DX

### Jest-style assertion messages
```
expect(received).toBe(expected)

    Expected: 1
    Received: undefined
    Hint: Received is undefined. This usually means the module needs to be mocked with ht.mock().
```

### Stack traces
- Full Hermes stack traces via `e.stack`
- `__HT_shallow_imports` global maps function names → module paths for error hints

## Hermes Runtime Config
- `withMicrotaskQueue(true)` — enables queueMicrotask/drainJobs support
- `__HT_drain` calls `rt.drainMicrotasks()` — flushes Hermes job queue
- No event loop — drain() is called explicitly by act()/flush()

## TypeScript
- `globals.d.ts` uses vitest pattern: `declare global { const ht }` + `export {}`
- Users add `"hermes-test/globals"` to tsconfig `types` array
- `ht.shallow` typed with JSDoc: "Shallow render: auto-mock child components"

