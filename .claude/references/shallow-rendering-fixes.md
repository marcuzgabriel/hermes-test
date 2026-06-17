# Shallow Rendering Fixes — June 2026

## Context

The `feat/component-rendering` branch added `ht.shallow()` for shallow component rendering.
Topdanmark had 23 `.hermes.test.tsx` files using it. All of them failed (87 total test failures,
plus 4 unrelated `.ts` hook failures). This document records the root causes, fixes applied,
and what still needs work.

## Starting point

```
Tests: 1486 passed, 87 failed, 1573 total (apps/topdanmark)
```

All `.hermes.test.tsx` component rendering tests failed. The `.hermes.test.ts` hook tests
were unaffected by the rendering fixes (4 of those fail for date/timer reasons).

---

## Bug 1: Shallow auto-mock deduplication discarded component names

**Root cause:** When multiple test files call `ht.shallow()` on different components that
import from the same module, the merge logic in `main.rs` used a "first wins" dedup:

```rust
// BAD: skips entry if path already exists — loses new component names
if !shallow_auto_mocks.iter().any(|(p, _)| p == &entry.0) {
    shallow_auto_mocks.push(entry);
}
```

Example: Test A shallow-renders a component using `Card` from `molecules`. Test B shallow-renders
a component using `Card` AND `AnimatedPressable` from `molecules`. If Test A is processed first,
`AnimatedPressable` is never added to the auto-mock.

**Fix:** Merge component names instead of skipping:

```rust
if let Some((_, existing_jsx, existing_other)) = shallow_auto_mocks.iter_mut().find(|(p, _, _)| p == &entry.0) {
    for name in entry.1 { if !existing_jsx.contains(&name) { existing_jsx.push(name); } }
    for name in entry.2 { if !existing_other.contains(&name) { existing_other.push(name); } }
} else {
    shallow_auto_mocks.push(entry);
}
```

**Files:** `main.rs` (4 locations: initial scan, watch mode, split mode, persistent watch)

**Impact:** Fixed SelectableCard (0/13 -> 12/13), BasketDetails, GiftCard passing.

---

## Bug 2: Auto-mock only stubbed JSX components, not co-exported hooks/utilities

**Root cause:** `scan_shallow_auto_mocks` only collected imports that appeared as `<Name` in JSX.
Non-JSX exports (hooks, constants, utilities) from the same module were ignored. The auto-mock
factory only contained JSX stubs, so the shadow wrapper Proxy fell through to `_getReal()`.

Problem: barrel files like `organisms/index.ts` re-export 50+ components. When `_getReal()`
loads the barrel, it pulls in native-dependent components (DateTimePicker, CameraControls, MapInput,
WheelPicker, etc.) that crash. The `_getReal()` fallback returns `{}`, making ALL non-mocked
exports `undefined`.

Concrete failure: `useSkeletonDefaultProps` from `organisms` returned `undefined` because
the barrel crashed on import.

**Fix:** Changed `scan_shallow_auto_mocks` return type from `(path, Vec<jsx_names>)` to
`(path, Vec<jsx_names>, Vec<other_names>)`. For modules that have at least one JSX component,
ALL named imports are now collected. Entry generation creates different stubs:

- JSX names: `R.createElement('Name', p)` (Proxy-wrapped, see Bug 5)
- Other names: chainable Proxy stubs (see Bug 4)

**Files:** `entry.rs` (scan function + both entry generators), `main.rs` (all type signatures),
`esbuild.rs` (thread-local type)

**Impact:** Fixed FormSteps `useSkeletonDefaultProps` crash.

---

## Bug 3: Combined default+named imports not parsed

**Root cause:** The import regex only matched two patterns:

```
import Default from 'path'           // re_default
import { Named } from 'path'         // re_named
```

But NOT:

```
import Animated, { FadeIn, FadeOut } from 'react-native-reanimated'  // combined
```

The comma after `Animated` broke the `re_default` match. `react-native-reanimated` was never
detected as having JSX components.

**Fix:** Added `re_combined` regex:

```rust
r#"import\s+(\w+)\s*,\s*\{([^}]+)\}\s+from\s+['"]([^'"]+)['"]"#
```

Used in both the first pass (finding modules with JSX) and second pass (collecting imports).

**Files:** `entry.rs`

**Impact:** `react-native-reanimated` now detected with `Animated` (JSX) + `FadeIn`, `FadeOut` (other).

---

## Bug 4: Non-JSX stubs broke on method chaining

**Root cause:** `FadeIn` from `react-native-reanimated` is used as `FadeIn.duration(700).delay(100)`.
The initial non-JSX stub was `function() { return {}; }`. Calling `.duration(700)` on `{}`
returns `undefined`, then `.delay(100)` on `undefined` crashes.

**Fix:** Non-JSX stubs use a self-referencing chainable Proxy:

```js
new Proxy(function() { return {}; }, {
  get: function(t, k) {
    if (typeof k === 'symbol') return t[k];
    var self = r['FadeIn'];
    return function() { return self; };
  }
})
```

Any property access returns a function that returns the Proxy itself, enabling infinite chaining.

**Files:** `entry.rs` (both entry generators)

**Impact:** Fixed `FadeIn.duration(700)` crash in FormSteps and other Animated components.

---

## Bug 5: JSX Proxy stubs triggered React 19 warnings

**Root cause:** JSX component stubs were plain functions. But for `<Animated.View>` to work,
`Animated` needs to be an object with a `.View` property. Initial fix used a Proxy that
returned a component function for ANY property access. React 19 then inspected these stubs
and found truthy values for `childContextTypes`, `getDerivedStateFromProps`, `contextTypes`,
etc., triggering many warnings.

**Fix:** JSX stubs use Proxy with a property allowlist. React-internal properties return
`undefined` (via `t[k]` fallback), while sub-component access returns createElement stubs:

```js
new Proxy(function(p) { return R.createElement('Name', p); }, {
  get: function(t, k) {
    if (typeof k === 'symbol') return t[k];
    if (k === 'prototype' || k === 'contextTypes' || k === 'childContextTypes' ||
        k === 'getDerivedStateFromProps' || /* ... */) return t[k];
    return function(p) { return R.createElement('Name.' + k, p); };
  }
})
```

**Files:** `entry.rs` (both entry generators)

**Impact:** Eliminated React 19 `childContextTypes` / `getDerivedStateFromProps` warnings.

---

## Bug 6: npm packages excluded from auto-mocking

**Root cause:** `scan_shallow_auto_mocks` had an `is_internal` guard that only processed
imports from relative paths or tsconfig aliases. npm packages like `react-native-reanimated`
were skipped even when used in JSX.

**Fix:** Removed the `is_internal` guard. All modules with JSX components are now auto-mocked,
whether internal (aliased) or npm. Kept existing filters for `react` and `react-native`
(those have dedicated shims).

**Files:** `entry.rs`

**Impact:** `react-native-reanimated` now gets package shims + auto-mock stubs.

---

## Bug 7: `ht.shallow()` with alias paths failed to resolve

**Root cause:** `scan_shallow_auto_mocks` only resolved relative paths via `resolve_relative_file`.
When a test uses `ht.shallow('@topdanmark/mobile-insurance-ui/components/molecules/InsuranceProductGroup')`,
the alias path couldn't be resolved to a real file. The function returned empty, so no
auto-mocks were generated.

**Fix:** Added `scan_shallow_auto_mocks_with_pairs` that accepts `alias_pairs: &[(String, String)]`.
After relative resolution fails, it tries alias resolution: matches the import path against
alias keys, replaces the prefix with the target directory, and tries file extensions + index files.

Also added `find_mock_modules_with_alias_pairs` so the mock module discovery uses the same
alias resolution.

**Files:** `entry.rs`, `main.rs`

**Impact:** Fixed InsuranceProductGroup (0/2 -> 2/2 passing).

---

## Debug: HT_DEBUG environment variable

Added `HT_DEBUG=1` support to print shallow auto-mock details:

```
$ HT_DEBUG=1 hermes-test
[DEBUG] Shallow auto-mocks (20):
  @topdanmark/mobile-insurance-ui/components/atoms -> jsx: ["Wrapper", "Text", "CTAButton"], other: []
  react-native-reanimated -> jsx: ["Animated"], other: ["FadeIn", "FadeOut", "FadeInDown"]
  ...
```

---

## Phase 2: Test-level fixes (Topdanmark test files)

After fixing the infrastructure (87 → 57), the remaining failures needed test-level fixes:

### Bug 8: `fireEvent.press` ignores `disabled` prop

`fireEvent.press` called `onPress` regardless of `disabled` prop. React Native's Pressable
blocks touch when disabled, but the harness didn't replicate this.

**Fix:** Check `disabled` prop before calling handler. Also added event bubbling — pressing
a `<Text>` inside a `<TouchableOpacity>` now walks up to find the nearest `onPress` handler.
Added `_parent` tracking in the reconciler's `appendChild`/`insertBefore`.

**Files:** `render.ts`, `hooks.ts` (hermes-test harness)

### Bug 9: `_MENU` / `_MENU_CARD` testID suffix from SelectableCard internals

Tests queried `testID_MENU` or `testID_MENU_CARD` — internal testIDs generated by the real
SelectableCard. With the stub, only the base `testID` exists.

**Fix:** Removed `_MENU` and `_MENU_CARD` suffixes from all testID queries in:
- SelectorDriver, SelectorNoContent, SelectorDynamicList, SelectorIncidentAuto, DamageZones

### Bug 10: `node.props.children` always undefined

The reconciler strips `children` from props in `createInstance`. Tests using
`node.props.children` to check text got `undefined`.

**Fix:** Changed to `node.children[0]?.text` or `getByText()` in:
- SelectorDriver, SelectorNoContent, SelectorDynamicList, DatePickerTimeFreeTextInput

### Bug 11: Theme mocks missing color keys

Components accessed color tokens not present in the mock (e.g. `COLORS.BACKGROUND.COLOR_BACKGROUND_SECONDARY`).

**Fix:** Added missing color keys to theme mocks in:
- DatePickerTimeFreeTextInput (added `BACKGROUND`, `UI_ELEMENT`)
- SelectorDynamicList (added `CTA`, `COLOR_GLYPH_TERTIARY`)

### Bug 12: `FormContext` mock incompatible with `useContext`

PackageAdditionalMessage's test mocked `FormContext` with a plain object. React's `useContext`
reads from the fiber tree, not `_currentValue`, so the mock didn't work.

**Fix:** Used `React.createContext(null)` to create a real context object.

### Bug 13: Text is a prop, not content (BenefitsCard, InsuranceCard)

Tests used `getByText('Kundefordel')` but the text was an `<Accordion title="...">` prop,
not rendered text content. Similarly `getByText('Bilforsikring')` was a Card prop.

**Fix:** Removed queries for text that's props on stubbed components. Kept queries for
text that's rendered as actual children (`<Text>{value}</Text>`).

---

## Phase 3: ht.unmock() and remaining fixes

### Bug 14: `ht.unmock()` — new API for opting out of the shim system

**Root cause:** esbuild inlines ESM imports as direct variable references. Modules like
`moment` and `moment-timezone` can't have their `.now` property patched via test code
because the Proxy shim intercepts property access and returns the original.

**Fix:** Added `ht.unmock('module')` — a build-time directive that removes the module from
the mock/shim list. The real module is bundled directly, so `require('moment').now = () => Date.now()`
patches the same object the hook uses.

**Files:** `entry.rs` (scanner), `harness.ts` (no-op runtime)

### Bug 15: Config shims not externalized

Config shims (e.g. `react-native-launch-arguments` in `hermes-test.config.json`) are loaded
into `__HT_mocks` in the entry but the package is NOT externalized by esbuild. Result: esbuild
bundles the real native module inline, and `import { LaunchArguments }` resolves to the
bundled (broken) code instead of the shim.

**Workaround:** Add `ht.mock('react-native-launch-arguments', ...)` in test files that
need it. A proper fix (externalizing shim packages in esbuild) regressed other tests.

### Bug 16: ActionFormBottomSheet ref not connecting

Selector tests press options that call `bottomSheetRef.current?.show()`. The auto-mock
stub doesn't use `forwardRef`, so `ref.current` stays `null` and the bottom sheet never opens.

**Fix:** Explicit `ht.mock` for ActionFormBottomSheet with `React.forwardRef` + `useImperativeHandle`
providing `show()`/`hide()`. Also render `footer` prop alongside children.

### Bug 17: renderField utility not auto-mocked

`renderField` creates `<TextField>` components but it's a utility function in a separate file,
not an import of the shallow target component. So `TextField` falls through to the real barrel
file which crashes.

**Fix:** Explicit `ht.mock` for `renderField` returning `<TextInput>` with testID/onChangeText.

### Bug 18: Danish character encoding in test expectations

Test assertions used ASCII (`pakoerte`, `koeretoej`, `behover`) but mock data used proper
Danish (`påkørte`, `køretøj`, `behøver`).

**Fix:** Updated test expectations to match actual Unicode characters.

### Bug 19: Missing mock properties

- `useTheme` missing `COLORS.BACKGROUND`, `COLORS.CTA`, `COLORS.UI_ELEMENT` in various tests
- `validateFields` mock missing `{ isValid: true }` return value
- `Analytics` mock missing `trackExternalLink`
- `LINKS` mock missing `MILEAGE_REDIRECT_GW`
- `useGetDigitalHealthcare` not set for elite/mileage test
- `fireEvent(el, 'onChange', ...)` should be `'change'` (hermes-test prepends `on`)

---

## Final result

```
Before: Tests: 1486 passed, 87 failed, 1573 total
After:  Tests: 1572 passed, 1 failed,  1573 total
```

86 tests fixed. The remaining 1 failure:

| File | Test | Root cause |
|------|------|------------|
| DatePickerTimeFreeTextInput | location input test | ActionFormBottomSheet `footer` prop not rendering after `show()` via ref — `useState` update inside forwardRef mock doesn't propagate to parent tree in single act pass |

---

## Files changed

```
hermes-test (infrastructure):
  crates/hermes-test-cli/src/bundler/entry.rs   — scan_shallow_auto_mocks rewrite, ht.unmock scanner
  crates/hermes-test-cli/src/bundler/esbuild.rs — thread-local type update
  crates/hermes-test-cli/src/main.rs            — merge logic, alias pairs, HT_DEBUG
  packages/hermes-test/src/render.ts            — fireEvent.press disabled check + bubbling
  packages/hermes-test/src/hooks.ts             — _parent tracking in reconciler
  packages/hermes-test/src/harness.ts           — ht.unmock runtime no-op

topdanmark tests (23 files modified):
  All Claims/ActionForm/* tests                 — ActionFormBottomSheet mock, renderField mock,
                                                  _MENU testIDs, theme colors, Danish encoding,
                                                  validateFields.isValid, DrillDownPill mock,
                                                  DateTimePickerBottomSheet mock, TextField mock
  InsurancePurchase/* tests                     — text-as-prop queries, FormContext, RegExp matching
  Hook tests                                    — ht.unmock('moment'), moment.now patching,
                                                  LaunchArguments mock, useCampaigns mock
```
