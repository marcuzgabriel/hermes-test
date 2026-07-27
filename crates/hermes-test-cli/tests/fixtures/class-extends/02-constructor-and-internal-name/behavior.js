function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

var t = new TaggedLogger("svc", "auth");
assert(t instanceof TaggedLogger, "instanceof TaggedLogger");
assert(t instanceof Logger, "instanceof Logger");
assert(t.prefix === "svc", "super(prefix) reached parent constructor");
assert(t.tag === "auth", "this.x assignment after super() lands on the instance");
assert(t.log("y") === "svc: y", "parent method inherited");
assert(t.tagged("hi") === "[auth] svc: hi", "own method sees both own and parent state");

// esbuild's internal-name pattern: static factory referencing `_TaggedLogger`
var made = TaggedLogger.create("push");
assert(made instanceof TaggedLogger, "internal class name rewritten to outer name in static method");
assert(made.tagged("go") === "[push] app: go", "factory-built instance fully functional");
"ok";
