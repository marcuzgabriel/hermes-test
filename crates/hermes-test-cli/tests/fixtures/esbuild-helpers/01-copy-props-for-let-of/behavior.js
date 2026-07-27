function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// Patch 1: Hermes's for-let-of closure bug makes every getter capture the
// LAST key — unpatched, to.a and to.b would all return c's value. The patch
// rewrites the loop to bind each key by value.
var src = { a: 1, b: 2, c: 3 };
var to = __copyProps({}, src);
assert(to.a === 1, "getter for 'a' returns a's value, not the last key's (for-let-of bug)");
assert(to.b === 2, "getter for 'b' returns b's value");
assert(to.c === 3, "getter for 'c' returns c's value");

var d = Object.getOwnPropertyDescriptor(to, "a");
assert(typeof d.get === "function", "props are live getters delegating to the source module");
assert(d.configurable === true, "patch adds configurable:true so mock() can redefine exports");

src.a = 99;
assert(to.a === 99, "getters stay live — reads always hit the current source value");
"ok";
