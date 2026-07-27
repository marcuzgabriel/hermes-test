function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// super.method() must keep `this` bound to the INSTANCE. The naive rewrite
// `super.describe()` → `Base.prototype.describe()` silently rebinds `this`
// to Base.prototype, so parent methods read undefined instance state.
var s = new Sub("x");
assert(s.describe() === "sub(base:x)", "super.method() in a method sees instance state (this bound)");
assert(s.initial === "base:x", "super.method() in a constructor sees instance state");
"ok";
