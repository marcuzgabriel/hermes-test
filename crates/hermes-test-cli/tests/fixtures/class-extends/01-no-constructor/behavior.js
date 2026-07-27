function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

var c = new Child("hermes");
assert(c instanceof Child, "instanceof Child");
assert(c instanceof Base, "instanceof Base");
assert(c.name === "hermes", "default constructor forwards args to parent");
assert(c.shout() === "HELLO HERMES", "own method calls inherited method through the chain");
assert(c.constructor === Child, "constructor identity preserved");
"ok";
