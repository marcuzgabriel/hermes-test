// Hermes runtime polyfills for hermes-test
// These run before any bundled code (injected via esbuild banner).
// Hermes lacks these APIs since they normally come from the RN native runtime.

// React checks process.env.NODE_ENV at load time
if (typeof globalThis.process === 'undefined') {
  globalThis.process = { env: { NODE_ENV: 'test' } };
} else if (!globalThis.process.env) {
  globalThis.process.env = { NODE_ENV: 'test' };
}

// Fix Array.isArray for class-extends-Array (Hermes doesn't set [[ArrayExoticObject]])
// RTK's Tuple extends Array but Array.isArray returns false for it.
(function() {
  var origIsArray = Array.isArray;
  Array.isArray = function(arg) {
    if (origIsArray(arg)) return true;
    // Check prototype chain for Array subclasses
    if (arg && typeof arg === 'object' && typeof arg.length === 'number') {
      var proto = Object.getPrototypeOf(arg);
      while (proto && proto !== Object.prototype) {
        if (proto === Array.prototype) return true;
        proto = Object.getPrototypeOf(proto);
      }
    }
    return false;
  };
})();

// Timer polyfills — React scheduler and react-test-renderer need these
(function() {
  var queue = [];
  var timerIdCounter = 1;
  var timers = {};

  if (typeof globalThis.setImmediate === 'undefined') {
    globalThis.setImmediate = function(fn) { queue.push(fn); };
  }

  // Flush all async work: Hermes microtask queue (promises) + our polyfill queues (timers).
  // The C++ bridge installs a native __drainMicrotasks that calls Hermes's drainMicrotasks().
  // We wrap it to also flush our setImmediate/setTimeout polyfill queues.
  var nativeDrain = globalThis.__drainMicrotasks || function() {};
  globalThis.__drainMicrotasks = function() {
    // 1. Drain Hermes's internal promise/microtask queue
    nativeDrain();
    // 2. Flush our setImmediate queue
    var limit = 1000;
    while (queue.length > 0 && limit-- > 0) { queue.shift()(); }
    // 3. Flush pending timers
    var ids = Object.keys(timers);
    for (var i = 0; i < ids.length; i++) {
      var t = timers[ids[i]];
      if (t) { delete timers[ids[i]]; t(); }
    }
    // 4. Drain again (timer callbacks may have queued more microtasks)
    nativeDrain();
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

// Web API polyfills — needed for RTK Query's fetchBaseQuery
(function() {
  // AbortController / AbortSignal
  if (typeof globalThis.AbortController === 'undefined') {
    function AbortSignal() { this.aborted = false; this._listeners = []; }
    AbortSignal.prototype.addEventListener = function(type, fn) { this._listeners.push(fn); };
    AbortSignal.prototype.removeEventListener = function(type, fn) {
      this._listeners = this._listeners.filter(function(f) { return f !== fn; });
    };

    function AbortController() { this.signal = new AbortSignal(); }
    AbortController.prototype.abort = function() {
      this.signal.aborted = true;
      for (var i = 0; i < this.signal._listeners.length; i++) {
        try { this.signal._listeners[i](); } catch(e) {}
      }
    };

    globalThis.AbortController = AbortController;
    globalThis.AbortSignal = AbortSignal;
  }

  // Headers
  if (typeof globalThis.Headers === 'undefined') {
    function Headers(init) {
      this._map = {};
      if (init) {
        if (typeof init.forEach === 'function') {
          init.forEach(function(v, k) { this._map[k.toLowerCase()] = v; }.bind(this));
        } else {
          var keys = Object.keys(init);
          for (var i = 0; i < keys.length; i++) {
            this._map[keys[i].toLowerCase()] = init[keys[i]];
          }
        }
      }
    }
    Headers.prototype.get = function(k) { return this._map[k.toLowerCase()] || null; };
    Headers.prototype.has = function(k) { return k.toLowerCase() in this._map; };
    Headers.prototype.set = function(k, v) { this._map[k.toLowerCase()] = v; };
    Headers.prototype.append = function(k, v) {
      k = k.toLowerCase();
      this._map[k] = this._map[k] ? this._map[k] + ', ' + v : v;
    };
    Headers.prototype.delete = function(k) { delete this._map[k.toLowerCase()]; };
    Headers.prototype.forEach = function(fn) {
      var keys = Object.keys(this._map);
      for (var i = 0; i < keys.length; i++) fn(this._map[keys[i]], keys[i], this);
    };
    Headers.prototype.entries = function() { return Object.entries(this._map); };
    globalThis.Headers = Headers;
  }

  // URL — always install: Hermes has a built-in URL that doesn't parse searchParams correctly
  {
    function URL(url, base) {
      if (base && url.indexOf('://') === -1) {
        url = base.replace(/\/$/, '') + '/' + url.replace(/^\//, '');
      }
      this.href = url;
      var match = url.match(/^(https?:)\/\/([^/:?#]+)(:\d+)?(\/[^?#]*)?(\?[^#]*)?(#.*)?$/);
      if (match) {
        this.protocol = match[1];
        this.hostname = match[2];
        this.port = match[3] ? match[3].slice(1) : '';
        this.pathname = match[4] || '/';
        this.search = match[5] || '';
        this.hash = match[6] || '';
        this.host = this.hostname + (this.port ? ':' + this.port : '');
        this.origin = this.protocol + '//' + this.host;
      } else {
        this.protocol = ''; this.hostname = ''; this.port = '';
        this.pathname = url; this.search = ''; this.hash = '';
        this.host = ''; this.origin = '';
      }
      this.searchParams = new URLSearchParams(this.search);
    }
    URL.prototype.toString = function() { return this.href; };
    globalThis.URL = URL;
  }

  // URLSearchParams — always install: native Hermes version may not parse correctly
  {
    function URLSearchParams(init) {
      this._params = [];
      if (typeof init === 'string') {
        init = init.replace(/^\?/, '');
        var pairs = init.split('&');
        for (var i = 0; i < pairs.length; i++) {
          if (!pairs[i]) continue;
          var kv = pairs[i].split('=');
          this._params.push([decodeURIComponent(kv[0]), decodeURIComponent(kv.slice(1).join('='))]);
        }
      }
    }
    URLSearchParams.prototype.get = function(k) {
      for (var i = 0; i < this._params.length; i++) {
        if (this._params[i][0] === k) return this._params[i][1];
      }
      return null;
    };
    URLSearchParams.prototype.has = function(k) {
      for (var i = 0; i < this._params.length; i++) {
        if (this._params[i][0] === k) return true;
      }
      return false;
    };
    URLSearchParams.prototype.set = function(k, v) {
      for (var i = 0; i < this._params.length; i++) {
        if (this._params[i][0] === k) { this._params[i][1] = v; return; }
      }
      this._params.push([k, v]);
    };
    URLSearchParams.prototype.append = function(k, v) { this._params.push([k, v]); };
    URLSearchParams.prototype.toString = function() {
      return this._params.map(function(p) { return encodeURIComponent(p[0]) + '=' + encodeURIComponent(p[1]); }).join('&');
    };
    globalThis.URLSearchParams = URLSearchParams;
  }

  // Request (minimal — RTK Query checks typeof Request)
  if (typeof globalThis.Request === 'undefined') {
    globalThis.Request = function Request(url, init) {
      this.url = typeof url === 'string' ? url : url.href;
      this.method = (init && init.method) || 'GET';
      this.headers = new globalThis.Headers(init && init.headers);
      this.body = init && init.body;
    };
  }

  // Stub fetch (mockFetch will override with handler-based implementation)
  if (typeof globalThis.fetch === 'undefined') {
    globalThis.fetch = function() {
      return Promise.reject(new Error('fetch not configured — use mockFetch() to register handlers'));
    };
  }

  // Response (minimal)
  if (typeof globalThis.Response === 'undefined') {
    globalThis.Response = function Response(body, init) {
      this.body = body;
      this.status = (init && init.status) || 200;
      this.ok = this.status >= 200 && this.status < 300;
      this.headers = new globalThis.Headers(init && init.headers);
    };
    globalThis.Response.prototype.json = function() { return Promise.resolve(JSON.parse(this.body)); };
    globalThis.Response.prototype.text = function() { return Promise.resolve(String(this.body)); };
  }
})();
