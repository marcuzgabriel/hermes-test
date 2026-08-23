  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __commonJS = (cb, mod2) => function __require2() {
    return mod2 || (0, cb[__getOwnPropNames(cb)[0]])((mod2 = { exports: {} }).exports, mod2), mod2.exports;
  };
  var require_snake_test = __commonJS({
    "src/snake.test.ts"(exports) {
      init_hermes_test();
      ht.mock("../fs", () => ({ read: () => "x" }));
            init_snake();
globalThis.__fixtureOrder.push(/[<>:"\/\\|?*]/.test("a:b") ? "regex-ok" : "regex-broken");
      globalThis.__fixtureOrder.push(10 / 2 === 5 ? "div-ok" : "div-broken");
      globalThis.__fixtureOrder.push("snake-end");
    }
  });
  var require_other_test = __commonJS({
    "src/other.test.ts"(exports) {
      init_hermes_test();
      ht.mock("../api", () => ({ load: () => 1 }));
            init_other();
globalThis.__fixtureOrder.push("other-end");
    }
  });
