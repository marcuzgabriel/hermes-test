function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// A regex literal containing a quote (`/[<>:"\/\\|?*]/`) used to open a phantom string in
// find_matching_brace. The scan then ran past the first test body into the next one (or to
// the end of the bundle → `len + 1` → slice panic at patches.rs:76). With regex literals
// understood, each body is hoisted independently and `10 / 2` is still read as division.

globalThis.__fixtureOrder = [];
var order = globalThis.__fixtureOrder;
function init_hermes_test() { order.push("init_hermes_test"); }
function init_snake() { order.push("init_snake"); }
function init_other() { order.push("init_other"); }
var ht = { mock: function (path) { order.push("mock:" + path); } };

require_snake_test();
require_other_test();

assert(order.join("|") ===
  "init_hermes_test|mock:../fs|init_snake|regex-ok|div-ok|snake-end|init_hermes_test|mock:../api|init_other|other-end",
  "both bodies hoisted independently despite the quote inside a regex literal (got: " + order.join("|") + ")");
"ok";
