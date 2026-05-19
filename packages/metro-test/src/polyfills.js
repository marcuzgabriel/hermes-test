// Hermes runtime polyfills for metro-test
// These run before any bundled code (injected via esbuild banner).
// Hermes lacks these APIs since they normally come from the RN native runtime.

// React checks process.env.NODE_ENV at load time
if (typeof globalThis.process === 'undefined') {
  globalThis.process = { env: { NODE_ENV: 'test' } };
} else if (!globalThis.process.env) {
  globalThis.process.env = { NODE_ENV: 'test' };
}

// Timer polyfills — React scheduler and react-test-renderer need these
(function() {
  var queue = [];
  var timerIdCounter = 1;
  var timers = {};

  if (typeof globalThis.setImmediate === 'undefined') {
    globalThis.setImmediate = function(fn) { queue.push(fn); };
  }

  // Used by act() and waitFor() to synchronously flush async work
  globalThis.__drainMicrotasks = function() {
    var limit = 1000;
    while (queue.length > 0 && limit-- > 0) { queue.shift()(); }
    var ids = Object.keys(timers);
    for (var i = 0; i < ids.length; i++) {
      var t = timers[ids[i]];
      if (t) { delete timers[ids[i]]; t(); }
    }
  };

  if (typeof globalThis.setTimeout === 'undefined') {
    globalThis.setTimeout = function(fn, delay) {
      var id = timerIdCounter++;
      if (!delay || delay <= 0) {
        queue.push(fn);
      } else {
        timers[id] = fn;
      }
      return id;
    };
  }

  if (typeof globalThis.clearTimeout === 'undefined') {
    globalThis.clearTimeout = function(id) { delete timers[id]; };
  }

  if (typeof globalThis.console === 'undefined') {
    globalThis.console = {
      log: function() {},
      warn: function() {},
      error: function() {},
      info: function() {},
      debug: function() {},
    };
  }
})();
