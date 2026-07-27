function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// Real bundles contain a SECOND set of esbuild helpers: dependencies shipped
// as pre-bundled CJS (react-redux et al.) inline their own copy, renamed with
// a `2` suffix and indented deeper. The patches must fix EVERY copy, not just
// the first — this input is the react-redux shape from the Topdanmark bundle.
var dep = require_fake_dep();
assert(dep.value === 42, "nested module's own export pipeline works");

var h = dep.__fixtureHelpers;

// Nested __copyProps2 must be patched like the outer one.
var src = { a: 1, b: 2, c: 3 };
var to = h.copyProps({}, src);
assert(to.a === 1 && to.b === 2 && to.c === 3, "nested __copyProps2: each getter returns its own key (for-let-of bug)");
var d = Object.getOwnPropertyDescriptor(to, "a");
assert(d.configurable === true, "nested __copyProps2: configurable:true applied");

// Nested __export2 must be patched like the outer one.
var ex = {};
h.exportFn(ex, { foo: () => "real" });
var de = Object.getOwnPropertyDescriptor(ex, "foo");
assert(de.configurable === true, "nested __export2: getters configurable");

// Nested __toESM2 must pass __esModule objects through identically.
var esm = { __esModule: true, x: 1 };
assert(h.toESM(esm) === esm, "nested __toESM2: __esModule module passes through as SAME object");

// And the OUTER helpers must still be patched (no regression from the fix).
var outer = __copyProps({}, { p: 7 });
assert(Object.getOwnPropertyDescriptor(outer, "p").configurable === true, "outer __copyProps still patched");
assert(__toESM(esm) === esm, "outer __toESM still patched");
"ok";
