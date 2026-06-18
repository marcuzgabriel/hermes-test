// hermes-test shim for @reduxjs/toolkit/query (without /react)
//
// Same singleton-cache pattern as rtk-query.js but loads the core module
// instead of the react variant, avoiding circular dependency when
// @reduxjs/toolkit/query/react internally imports @reduxjs/toolkit/query.
//
// Usage in hermes-test.config.json:
//   "shims": { "@reduxjs/toolkit/query": "hermes-test/shims/rtk-query-core" }

var real = require('@__ht_real_pkg/@reduxjs/toolkit/query');

var _apiCache = {};

var handler = {
  get: function (target, prop) {
    if (prop === 'createApi') {
      return function createApi(opts) {
        var key = (opts && opts.reducerPath) || 'api';
        if (_apiCache[key]) return _apiCache[key];
        var api = real.createApi(opts);
        _apiCache[key] = api;
        return api;
      };
    }
    return real[prop];
  },
  ownKeys: function () {
    try {
      return Object.getOwnPropertyNames(real);
    } catch (e) {
      return [];
    }
  },
  getOwnPropertyDescriptor: function (target, prop) {
    try {
      var d = Object.getOwnPropertyDescriptor(real, prop);
      if (d)
        return {
          configurable: true,
          enumerable: d.enumerable,
          writable: true,
          value: d.get ? d.get() : d.value,
        };
    } catch (e) {}
    return { configurable: true, enumerable: false, writable: true, value: undefined };
  },
};

module.exports = new Proxy({}, handler);
