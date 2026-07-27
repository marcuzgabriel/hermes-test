function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// Patch 3: hermes-test's __require returns mock Proxies with __esModule=true.
// Unpatched __toESM COPIES properties into a fresh object, destroying Proxy
// behavior (mocks registered later become invisible). The patch inserts an
// early return: __esModule modules pass through IDENTICALLY.
var esm = { __esModule: true, x: 1 };
var out = __toESM(esm);
assert(out === esm, "__esModule module passes through as the SAME object (Proxy survives)");

// Non-ESM (CJS) modules must still get the normal wrapping.
var cjs = { y: 2 };
var wrapped = __toESM(cjs);
assert(wrapped !== cjs, "CJS module still gets wrapped in a namespace object");
assert(wrapped.y === 2, "named props copied onto the namespace");
assert(wrapped.default === cjs, "default points at module.exports (node interop)");
"ok";
