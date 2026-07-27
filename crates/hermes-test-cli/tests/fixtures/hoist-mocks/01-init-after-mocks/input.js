  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __commonJS = (cb, mod2) => function __require2() {
    return mod2 || (0, cb[__getOwnPropNames(cb)[0]])((mod2 = { exports: {} }).exports, mod2), mod2.exports;
  };
  var require_counter_test = __commonJS({
    "src/counter.test.ts"(exports) {
      init_hermes_test();
      init_store();
      var mockDispatch = function() {};
      ht.mock("../store", () => ({ dispatch: mockDispatch }));
      init_counter();
      ht.mock("../api", () => ({ load: () => 1 }));
      globalThis.__fixtureOrder.push("body-end");
    }
  });
