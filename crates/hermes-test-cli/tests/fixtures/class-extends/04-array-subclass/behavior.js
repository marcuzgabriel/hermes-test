function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// Hermes native-super bug: super() in `class X extends Array` discards the
// return value — the Reflect.construct downlevel is what makes this work.
var s = new Stack();
s.push(1, 2, 3);
assert(s instanceof Stack, "instanceof Stack");
assert(s instanceof Array, "instanceof Array");
assert(Array.isArray(s), "Array.isArray sees a real array");
assert(s.length === 3, "push updates length (exotic array behavior preserved)");
assert(s.peek() === 3, "own method reads indexed elements");

// Transform contract: Symbol.species is defined on the subclass.
assert(Stack[Symbol.species] === Stack, "transform installs Symbol.species on the subclass");

// Engine-faithful assertion (discovered by this fixture, first run, 2026-07):
// Hermes does NOT honor Symbol.species in Array methods — map() on a subclass
// returns a plain Array, on device and here alike. A Node-based runner would
// assert the opposite and pass; that divergence is the point of this runner.
var mapped = s.map(function (x) { return x * 2; });
assert(!(mapped instanceof Stack), "Hermes ignores species: map returns plain Array (engine fidelity)");
assert(Array.isArray(mapped), "map result is a real Array");
assert(mapped[2] === 6, "map produced correct values");
"ok";
