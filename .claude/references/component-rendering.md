# Component Rendering — Status & Next Steps

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
- Walks up from test file directory, checks hermes-test/node_modules, workspace paths
- react-reconciler moved to hermes-test dependencies (installed with package)

## Current Blocker: Deep Component Imports

When importing real components (e.g. `import BasketDetails from '../index'`), esbuild follows
the entire import chain: UI library → react-native internals → native modules → crash.

This is the same problem as Jest's `transformIgnorePatterns` but in esbuild's context.
Pure `render(<div>Hello</div>)` works fine. Real component imports crash.

## Next Step: Shallow Rendering

**The key insight:** The existing Jest tests already mock all child components manually.
Shallow rendering automates this — same result, zero boilerplate.

### How it works
```ts
render(<BasketDetails {...props} />, { shallow: true })
```

1. React renders BasketDetails — runs its hooks, returns JSX
2. JSX contains `<InsuranceCard>`, `<GiftCard>`, etc.
3. Reconciler sees `InsuranceCard` but does NOT call its function body
4. Records it as `{ type: 'InsuranceCard', props: { ... }, children: [] }`
5. No child imports are triggered — no crash

### Implementation plan
1. **Host config change**: In `createInstance`, check if the component type is a user
   component (function/class) vs a host component (string like 'View', 'Text').
   In shallow mode, don't recurse into user components — just record type + props.
2. **Reconciler wrapper**: Create a shallow reconciler that wraps component types
   with a stub before rendering. When the reconciler encounters a non-host component
   in shallow mode, replace it with a pass-through that just records its props.
3. **API**: `render(element, { shallow: true })` — add option to render.ts

### What shallow gives you
- `getByType('InsuranceCard')` — find by component name
- `toHaveProp('insurance', ...)` — assert props passed to children
- `toMatchSnapshot()` — captures structure without child internals
- No `mockModule` needed for child components
- No UI library shims needed

### What still needs full render
- Integration tests between parent and child
- Tests that assert on deeply nested text content
- Tests that need `fireEvent` on child component elements

### Both modes should coexist
- Default `render()` = full rendering (needs shims/mocks for native deps)
- `render(el, { shallow: true })` = stop at first level (works without dependency setup)
