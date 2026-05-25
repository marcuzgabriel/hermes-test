// In-memory AsyncStorage shim for hermes-test.
// Provides a fresh store per test file. Data persists within a file,
// resets between files via __HT_resetAsyncStorage().

var _store = {};

var AsyncStorage = {
  getItem: function(key) {
    var val = _store[key] !== undefined ? _store[key] : null;
    return Promise.resolve(val);
  },
  setItem: function(key, value) {
    _store[key] = value;
    return Promise.resolve();
  },
  removeItem: function(key) {
    delete _store[key];
    return Promise.resolve();
  },
  multiRemove: function(keys) {
    for (var i = 0; i < keys.length; i++) {
      delete _store[keys[i]];
    }
    return Promise.resolve();
  },
  multiGet: function(keys) {
    var result = [];
    for (var i = 0; i < keys.length; i++) {
      result.push([keys[i], _store[keys[i]] !== undefined ? _store[keys[i]] : null]);
    }
    return Promise.resolve(result);
  },
  multiSet: function(pairs) {
    for (var i = 0; i < pairs.length; i++) {
      _store[pairs[i][0]] = pairs[i][1];
    }
    return Promise.resolve();
  },
  getAllKeys: function() {
    return Promise.resolve(Object.keys(_store));
  },
  clear: function() {
    _store = {};
    return Promise.resolve();
  },
};

// Reset hook — called between test files by the harness
globalThis.__HT_resetAsyncStorage = function() {
  _store = {};
};

module.exports = AsyncStorage;
module.exports.default = AsyncStorage;
