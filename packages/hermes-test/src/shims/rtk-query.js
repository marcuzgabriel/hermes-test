// hermes-test shim for @reduxjs/toolkit/query/react
//
// Fixes the dual-instance problem: when split bundles or shadow wrappers
// cause createApi() to be called twice with the same config, only one
// API instance is created. injectEndpoints() mutations always hit the
// canonical instance, so the store and endpoint modules stay in sync.
//
// Usage in hermes-test.config.json:
//   "shims": { "@reduxjs/toolkit/query/react": "hermes-test/shims/rtk-query" }

var real = require('@__ht_real_pkg/@reduxjs/toolkit/query/react');

// Singleton cache keyed by reducerPath — prevents dual-instance from
// split bundles or shadow wrapper re-entrancy
var _apiCache = {};

// Proxy delegates everything to the real module with late binding.
// This avoids eagerly copying ESM live bindings that may not be initialized yet.
var handler = {
  get: function(target, prop) {
    if (prop === 'createApi') {
      return function createApi(opts) {
        var key = opts && opts.reducerPath || 'api';
        if (_apiCache[key]) return _apiCache[key];
        var api = real.createApi(opts);
        _apiCache[key] = api;
        return api;
      };
    }
    return real[prop];
  },
  ownKeys: function() {
    try { return Object.getOwnPropertyNames(real); } catch(e) { return []; }
  },
  getOwnPropertyDescriptor: function(target, prop) {
    try {
      var d = Object.getOwnPropertyDescriptor(real, prop);
      if (d) return { configurable: true, enumerable: d.enumerable, writable: true, value: d.get ? d.get() : d.value };
    } catch(e) {}
    return { configurable: true, enumerable: false, writable: true, value: undefined };
  }
};

module.exports = new Proxy({}, handler);
