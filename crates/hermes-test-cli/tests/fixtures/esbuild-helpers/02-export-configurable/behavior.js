function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// Patch 2: esbuild's __export defines module exports as NON-configurable
// getters, which makes replacing an export (mocking) throw. The patch adds
// configurable:true.
var exportsObj = {};
__export(exportsObj, { foo: () => 42, bar: () => "real" });
assert(exportsObj.foo === 42, "__export getters still work");
assert(exportsObj.bar === "real", "all names exported");

var d = Object.getOwnPropertyDescriptor(exportsObj, "bar");
assert(d.configurable === true, "export getters configurable (unpatched esbuild: false)");

// The actual capability this buys: an export can be redefined, like a mock would.
Object.defineProperty(exportsObj, "bar", { value: "mocked", configurable: true });
assert(exportsObj.bar === "mocked", "export can be replaced after definition");
"ok";
