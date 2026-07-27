function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// esbuild's __require THROWS for externalized modules ("Dynamic require of X
// is not supported"). The shim replaces the throw with a Proxy over the
// __HT_mocks registry — this is how natives/externalized modules become
// mockable at all. (In Hermes `typeof require === "undefined"`, so every
// __require call takes the patched branch.)

// Global mock registry lookup.
globalThis.__HT_mocks = { "react-native": { Platform: { OS: "ios" } } };
var rn = __require("react-native");
assert(rn.__esModule === true, "externalized module namespace reports __esModule");
assert(rn.Platform.OS === "ios", "global mock value served through the Proxy");

// Mock-or-real is decided at ACCESS time, per running test file — registering
// a per-file mock AFTER the import must win (ESM hoisting problem).
globalThis.__currentTestFile = "b.test.ts";
globalThis.__HT_file_mocks = { "b.test.ts": { "react-native": { Platform: { OS: "android" } } } };
assert(rn.Platform.OS === "android", "per-file mock overrides live, even after import ran");
globalThis.__HT_file_mocks = undefined;
assert(rn.Platform.OS === "ios", "falls back to global mock when per-file mock is gone");

// Unmocked externalized module: infinite-noop Proxy instead of a crash.
var unknown = __require("some-native-module");
unknown.anything.deeply().chained.calls();
assert(String(unknown.missing) === "", "noop stringifies to empty string");
assert(unknown.missing.length === 0, "noop reports length 0");
assert(unknown.missing.then === undefined, "noop is not thenable (await-safe)");

// default interop: mock without explicit default serves the mock itself.
globalThis.__HT_mocks["cjs-lib"] = { helper: function () { return 7; } };
var cjs = __require("cjs-lib");
assert(cjs.default.helper() === 7, "default falls back to the mock object itself");
"ok";
