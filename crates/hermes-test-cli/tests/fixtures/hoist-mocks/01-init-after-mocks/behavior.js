function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// esbuild initializes modules (init_*()) where the import statements were —
// BEFORE the ht.mock() calls below them ever run. A module that captures
// values at init time (`const { dispatch } = store`) would grab the real
// thing. The transform pushes init_*() calls below the last ht.mock() so
// mocks are registered before any module initializes. init_hermes* must
// stay put (the harness itself must init first).

globalThis.__fixtureOrder = [];
var order = globalThis.__fixtureOrder;
function init_hermes_test() { order.push("init_hermes_test"); }
function init_store() { order.push("init_store"); }
function init_counter() { order.push("init_counter"); }
var ht = { mock: function (path) { order.push("mock:" + path); } };

require_counter_test();

assert(order.join("|") ===
  "init_hermes_test|mock:../store|mock:../api|init_store|init_counter|body-end",
  "init_*() calls run AFTER all ht.mock() registrations (got: " + order.join("|") + ")");
"ok";
