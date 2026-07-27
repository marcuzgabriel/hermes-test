function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// The Day 23 shape: new.target must survive the whole super chain.
// The discriminating case is a NO-CONSTRUCTOR class in the MIDDLE of the
// chain (B): its synthesized constructor must forward new.target, or every
// instance constructed through it silently gets B.prototype instead of the
// most-derived prototype. (A no-constructor class at the top of a chain
// cannot catch this — nothing extends it. Gap found by mutation-testing
// this very corpus, 2026-07.)
var d = new D();
assert(d instanceof D, "instanceof D");
assert(d instanceof C, "instanceof C");
assert(d instanceof B, "instanceof B");
assert(d instanceof A, "instanceof A");
assert(Object.getPrototypeOf(d) === D.prototype, "new.target forwarded through no-ctor middle AND leaf (Day 23 bug)");
assert(d.trail.join(",") === "A,C", "every explicit constructor in the chain ran exactly once");

var c = new C();
assert(Object.getPrototypeOf(c) === C.prototype, "new.target forwarded through no-ctor middle class");

var b = new B();
assert(Object.getPrototypeOf(b) === B.prototype, "direct construction of no-ctor class still correct");
assert(b.trail.join(",") === "A", "chain above the no-ctor class runs");
"ok";
