function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// ENGINE CONFORMANCE (V1). On the legacy engine these were all broken —
// Error subclasses silently lost message/props, Map/Set threw "requires
// 'new'" — and a ~400-line downleveling transform (old patch 4) papered
// over it. The V1 engine handles them natively; this fixture is the tripwire
// that any future engine bump keeps it that way. If it fails, the engine
// regressed and the transform graveyard in git history is the map back.
var e = new ChildError("boom");
assert(e.message === "boom", "Error subclass keeps message through 2-level chain");
assert(e.base === "B" && e.child === "C", "instance props from every level");
assert(e instanceof ChildError && e instanceof BaseError && e instanceof Error, "prototype chain");
assert(Object.getPrototypeOf(e) === ChildError.prototype, "most-derived prototype");

var r = new Registry();
r.set("k", 1);
assert(r.get("k") === 1 && r instanceof Registry && r instanceof Map, "Map subclass works natively");
"ok";
