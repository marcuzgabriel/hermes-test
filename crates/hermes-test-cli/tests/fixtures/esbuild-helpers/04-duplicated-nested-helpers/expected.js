  var __create = Object.create;
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __getProtoOf = Object.getPrototypeOf;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __commonJS = (cb, mod2) => function __require2() {
    return mod2 || (0, cb[__getOwnPropNames(cb)[0]])((mod2 = { exports: {} }).exports, mod2), mod2.exports;
  };
  var __export = (target, all) => {
    for (var name in all)
      __defProp(target, name, { get: all[name], enumerable: true, configurable: true });
  };
  var __copyProps = (to, from, except, desc) => {
    if (from && typeof from === "object" || typeof from === "function") {
      var keys = __getOwnPropNames(from);
      for (var i = 0; i < keys.length; i++) {
        var key = keys[i];
        if (!__hasOwnProp.call(to, key) && key !== except)
          __defProp(to, key, { get: ((k) => from[k]).bind(null, key), enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable, configurable: true });
      }
    }
    return to;
  };
  var __toESM = (mod2, isNodeMode, target) => (mod2 && mod2.__esModule ? mod2 : (target = mod2 != null ? __create(__getProtoOf(mod2)) : {}, __copyProps(
    // If the importer is in node compatibility mode or this is not an ESM
    // file that has been converted to a CommonJS file using a Babel-
    // compatible transform (i.e. "__esModule" has not been set), then set
    // "default" to the CommonJS "module.exports" for node compatibility.
    isNodeMode || !mod2 || !mod2.__esModule ? __defProp(target, "default", { value: mod2, enumerable: true }) : target,
    mod2
  )));
  var require_fake_dep = __commonJS({
    "node_modules/fake-dep/dist/cjs/fake-dep.development.cjs"(exports, module) {
      "use strict";
      var __create2 = Object.create;
      var __defProp2 = Object.defineProperty;
      var __getOwnPropDesc2 = Object.getOwnPropertyDescriptor;
      var __getOwnPropNames2 = Object.getOwnPropertyNames;
      var __getProtoOf2 = Object.getPrototypeOf;
      var __hasOwnProp2 = Object.prototype.hasOwnProperty;
      var __export2 = (target, all) => {
        for (var name in all)
          __defProp2(target, name, { get: all[name], enumerable: true, configurable: true });
      };
      var __copyProps2 = (to, from, except, desc) => {
        if (from && typeof from === "object" || typeof from === "function") {
          var keys = __getOwnPropNames2(from);
          for (var i = 0; i < keys.length; i++) {
            var key = keys[i];
            if (!__hasOwnProp2.call(to, key) && key !== except)
              __defProp2(to, key, { get: ((k) => from[k]).bind(null, key), enumerable: !(desc = __getOwnPropDesc2(from, key)) || desc.enumerable, configurable: true });
          }
        }
        return to;
      };
      var __toESM2 = (mod2, isNodeMode, target) => (mod2 && mod2.__esModule ? mod2 : (target = mod2 != null ? __create2(__getProtoOf2(mod2)) : {}, __copyProps2(
        // If the importer is in node compatibility mode or this is not an ESM
        // file that has been converted to a CommonJS file using a Babel-
        // compatible transform (i.e. "__esModule" has not been set), then set
        // "default" to the CommonJS "module.exports" for node compatibility.
        isNodeMode || !mod2 || !mod2.__esModule ? __defProp2(target, "default", { value: mod2, enumerable: true }) : target,
        mod2
      )));
      var __toCommonJS2 = (mod2) => __copyProps2(__defProp2({}, "__esModule", { value: true }), mod2);
      var src_exports = {};
      __export2(src_exports, {
        value: () => innerValue
      });
      module.exports = __toCommonJS2(src_exports);
      var innerValue = 42;
      module.exports.__fixtureHelpers = { copyProps: __copyProps2, toESM: __toESM2, exportFn: __export2 };
    }
  });
