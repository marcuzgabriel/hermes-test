function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// Narrowed patch 4: only the subtree rooted at a NATIVE builtin gets
// downleveled. BaseError (extends Error) must be downleveled; ChildError
// must be too — probed July 2026: native super() into a downleveled
// function DISCARDS its return, so the whole chain converts or none of it.
// UserChild (pure user chain) stays a native class.
var e = new ChildError("boom");
assert(e.message === "boom", "Error message survives the downleveled chain");
assert(e.base === "B", "base class instance prop set");
assert(e.child === "C", "child class instance prop set");
assert(e instanceof ChildError && e instanceof BaseError && e instanceof Error, "full prototype chain");
assert(Object.getPrototypeOf(e) === ChildError.prototype, "most-derived prototype");

var u = new UserChild();
assert(u.kind === "plain" && u.extra === 1, "user chain constructs correctly (natively)");
assert(u instanceof UserChild && u instanceof Plain, "user chain instanceof");
"ok";
