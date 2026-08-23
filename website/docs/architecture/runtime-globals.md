---
title: Runtime globals
---

# Runtime globals

Hermes is only a JavaScript engine. On a device, React Native's `InitializeCore`
(`Libraries/Core/setUpXHR.js` and friends) installs the web-platform globals apps rely on.
hermes-test installs the same set — from the same sources where that is possible — so tests see
what the phone sees. Users install and configure nothing.

| Global | Source in hermes-test | Source in React Native |
|---|---|---|
| `Headers`, `Request`, `Response` | `whatwg-fetch` (bundled into the harness) | `whatwg-fetch` (`Libraries/Network/fetch.js`) |
| `AbortController`, `AbortSignal` | `abort-controller` (bundled into the harness) | `abort-controller` |
| `fetch` | hermes-test's handler-based mock — see [mock.fetch](../api/mock) | `whatwg-fetch` over XMLHttpRequest |
| `FormData` | mirror of RN's `Libraries/Network/FormData.js` (`append`, `getAll`, `getParts`) | Flow file, cannot be bundled |
| `Blob`, `File` | RN-shaped (`Libraries/Blob/Blob.js`): string/Blob parts, `size`, `type`, `slice`, `close`; typed-array parts rejected like RN on iOS | native blob store |
| `URL`, `URLSearchParams` | mirror of RN's `Libraries/Blob/URL.js` — userinfo stripped from `host`, `protocol` for any scheme, **no host for non-http schemes** (same as RN) | Flow file importing a native module |
| `console` | full RN surface (`log/info/warn/error`, `assert`, `group*`, `table`, `time*`, `count*`) | `@react-native/js-polyfills/console.js` |
| `setTimeout`, `setImmediate`, `MessageChannel`, `queueMicrotask` | harness polyfills (drained deterministically by the test loop) | native |

## Expo projects

When the project depends on `expo`, hermes-test additionally runs the project's **own**
`expo/src/winter/runtime.native.ts` before any test — exactly what `expo/src/Expo.fx` does at
app start. Nothing is pinned: whatever Expo SDK is installed is what runs. That installs Expo's
WinterCG globals on top of React Native's:

| Global | Notes |
|---|---|
| `TextDecoder`, `TextDecoderStream`, `TextEncoderStream` | present on Expo, absent on bare RN |
| `URL`, `URLSearchParams` | full WHATWG (`whatwg-url-minimum`) — replaces RN's minimal URL, so e.g. `new URL('s3://bucket/key').host === 'bucket'` |
| `structuredClone` | `@ungap/structured-clone` |
| `FormData` | spec methods patched in (`get`, `has`, `entries`, `delete`, …) |
| `fetch` | **stays hermes-test's mock** — Expo's native `expo/fetch` is skipped via Expo's own `EXPO_PUBLIC_USE_RN_FETCH=1` opt-out |

Bare React Native projects (RN CLI, no `expo` package) get React Native's set only. Disable the
Expo runtime with `"expoRuntime": false` in `hermes-test.config.json`.
`examples/expo-app/src/examples/expo-runtime-globals.test.ts` pins this; `rn-globals.test.ts`
documents where the two differ.

## What is deliberately *not* installed

Anything React Native does not ship — for example `TextDecoder` (Hermes has `TextEncoder` only)
or `crypto.subtle`. Code that uses them fails in hermes-test exactly where it would fail on a
device. If your app installs its own polyfills at startup (`react-native-url-polyfill`,
`text-encoding`, …), run that setup in your tests too; the harness only fills globals that are
absent, so yours win.

## Verifying

`examples/expo-app/src/examples/rn-globals.test.ts` pins the behaviour of every row above.
