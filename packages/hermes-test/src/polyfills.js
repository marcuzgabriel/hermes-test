// Hermes runtime polyfills for hermes-test
// These run before any bundled code (injected via esbuild banner).
// Hermes lacks these APIs since they normally come from the RN native runtime.

// React checks process.env.NODE_ENV at load time
if (typeof globalThis.process === 'undefined') {
  globalThis.process = { env: { NODE_ENV: 'test', JEST_WORKER_ID: '1' } };
} else if (!globalThis.process.env) {
  globalThis.process.env = { NODE_ENV: 'test', JEST_WORKER_ID: '1' };
} else {
  // Always set JEST_WORKER_ID so RTK Query apiBaseQuery uses the mock domain (apiMockDomain)
  // rather than the real AWS domain. Runs before any bundled module-level code.
  if (!globalThis.process.env.JEST_WORKER_ID) {
    globalThis.process.env.JEST_WORKER_ID = '1';
  }
}

// process.nextTick — many Node.js-style tests use this
if (typeof globalThis.process.nextTick === 'undefined') {
  globalThis.process.nextTick = function (fn) {
    Promise.resolve().then(fn);
  };
}

// Object.fromEntries — ES2019, may not exist in older Hermes builds
if (typeof Object.fromEntries === 'undefined') {
  Object.fromEntries = function (iterable) {
    var obj = {};
    if (iterable && typeof iterable[Symbol.iterator] === 'function') {
      var iter = iterable[Symbol.iterator]();
      var next;
      while (!(next = iter.next()).done) {
        obj[next.value[0]] = next.value[1];
      }
    } else if (iterable && typeof iterable.forEach === 'function') {
      iterable.forEach(function (pair) {
        obj[pair[0]] = pair[1];
      });
    }
    return obj;
  };
}

// crypto.getRandomValues — needed by uuid and other crypto-dependent libs
if (typeof globalThis.crypto === 'undefined') {
  globalThis.crypto = {};
}
if (typeof globalThis.crypto.getRandomValues === 'undefined') {
  globalThis.crypto.getRandomValues = function (arr) {
    for (var i = 0; i < arr.length; i++) {
      arr[i] = Math.floor(Math.random() * 256);
    }
    return arr;
  };
}

// MessageChannel polyfill — React 19's scheduler uses it for async work
if (typeof globalThis.MessageChannel === 'undefined') {
  globalThis.MessageChannel = function () {
    var cb = null;
    this.port1 = { onmessage: null };
    this.port2 = {
      postMessage: function () {
        if (cb) {
          var fn = cb;
          cb = null;
          fn({ data: undefined });
        }
      },
    };
    var self = this;
    Object.defineProperty(this.port1, 'onmessage', {
      set: function (fn) {
        cb = fn;
      },
      get: function () {
        return cb;
      },
    });
  };
}

// Timer polyfills — React scheduler needs these
(function () {
  var queue = [];
  var timerIdCounter = 1;
  var timers = {};

  if (typeof globalThis.setImmediate === 'undefined') {
    globalThis.setImmediate = function (fn) {
      queue.push(fn);
    };
  }

  // Flush all async work: Hermes microtask queue (promises) + our polyfill queues (timers).
  // The C++ bridge installs a native __HT_drain that calls Hermes's drainMicrotasks().
  // We wrap it to also flush our setImmediate/setTimeout polyfill queues.
  var nativeDrain = globalThis.__HT_drain || function () {};
  globalThis.__HT_drain = function () {
    // 1. Drain Hermes's internal promise/microtask queue
    nativeDrain();
    // 2. Flush our setImmediate queue
    var limit = 1000;
    while (queue.length > 0 && limit-- > 0) {
      queue.shift()();
    }
    // 3. Flush pending timers
    var ids = Object.keys(timers);
    for (var i = 0; i < ids.length; i++) {
      var t = timers[ids[i]];
      if (t) {
        delete timers[ids[i]];
        t();
      }
    }
    // 4. Drain again (timer callbacks may have queued more microtasks)
    nativeDrain();
  };

  if (typeof globalThis.setTimeout === 'undefined') {
    globalThis.setTimeout = function (fn, delay) {
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
    globalThis.clearTimeout = function (id) {
      delete timers[id];
    };
  }

  if (typeof globalThis.console === 'undefined') {
    globalThis.console = {
      log: function () {},
      warn: function () {},
      error: function () {},
      info: function () {},
      debug: function () {},
    };
  }
})();

// Web API polyfills — needed for RTK Query's fetchBaseQuery
(function () {
  // AbortController / AbortSignal
  if (typeof globalThis.AbortController === 'undefined') {
    function AbortSignal() {
      this.aborted = false;
      this._listeners = [];
    }
    AbortSignal.prototype.addEventListener = function (type, fn) {
      this._listeners.push(fn);
    };
    AbortSignal.prototype.removeEventListener = function (type, fn) {
      this._listeners = this._listeners.filter(function (f) {
        return f !== fn;
      });
    };

    function AbortController() {
      this.signal = new AbortSignal();
    }
    AbortController.prototype.abort = function () {
      this.signal.aborted = true;
      for (var i = 0; i < this.signal._listeners.length; i++) {
        try {
          this.signal._listeners[i]();
        } catch (e) {}
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
          init.forEach(
            function (v, k) {
              this._map[k.toLowerCase()] = v;
            }.bind(this),
          );
        } else {
          var keys = Object.keys(init);
          for (var i = 0; i < keys.length; i++) {
            this._map[keys[i].toLowerCase()] = init[keys[i]];
          }
        }
      }
    }
    Headers.prototype.get = function (k) {
      return this._map[k.toLowerCase()] || null;
    };
    Headers.prototype.has = function (k) {
      return k.toLowerCase() in this._map;
    };
    Headers.prototype.set = function (k, v) {
      this._map[k.toLowerCase()] = v;
    };
    Headers.prototype.append = function (k, v) {
      k = k.toLowerCase();
      this._map[k] = this._map[k] ? this._map[k] + ', ' + v : v;
    };
    Headers.prototype.delete = function (k) {
      delete this._map[k.toLowerCase()];
    };
    Headers.prototype.forEach = function (fn) {
      var keys = Object.keys(this._map);
      for (var i = 0; i < keys.length; i++) fn(this._map[keys[i]], keys[i], this);
    };
    Headers.prototype.entries = function () {
      return Object.entries(this._map);
    };
    globalThis.Headers = Headers;
  }

  // URLSearchParams — always install BEFORE URL: native Hermes version may not parse correctly
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
      } else if (init && typeof init === 'object') {
        if (Array.isArray(init)) {
          // Array of [key, value] pairs
          for (var i = 0; i < init.length; i++) {
            this._params.push([String(init[i][0]), String(init[i][1])]);
          }
        } else if (typeof init[Symbol.iterator] === 'function') {
          // Iterable (e.g. another URLSearchParams instance)
          var iter = init[Symbol.iterator]();
          var next;
          while (!(next = iter.next()).done) {
            this._params.push([String(next.value[0]), String(next.value[1])]);
          }
        } else if (typeof init.forEach === 'function') {
          // URLSearchParams-like with forEach(value, key)
          init.forEach(
            function (v, k) {
              this._params.push([String(k), String(v)]);
            }.bind(this),
          );
        } else {
          // Plain object: { key: value }
          var keys = Object.keys(init);
          for (var i = 0; i < keys.length; i++) {
            this._params.push([keys[i], String(init[keys[i]])]);
          }
        }
      }
    }
    URLSearchParams.prototype.get = function (k) {
      for (var i = 0; i < this._params.length; i++) {
        if (this._params[i][0] === k) return this._params[i][1];
      }
      return null;
    };
    URLSearchParams.prototype.has = function (k) {
      for (var i = 0; i < this._params.length; i++) {
        if (this._params[i][0] === k) return true;
      }
      return false;
    };
    URLSearchParams.prototype.set = function (k, v) {
      for (var i = 0; i < this._params.length; i++) {
        if (this._params[i][0] === k) {
          this._params[i][1] = String(v);
          return;
        }
      }
      this._params.push([k, String(v)]);
    };
    URLSearchParams.prototype.append = function (k, v) {
      this._params.push([k, String(v)]);
    };
    URLSearchParams.prototype['delete'] = function (k) {
      this._params = this._params.filter(function (p) {
        return p[0] !== k;
      });
    };
    URLSearchParams.prototype.entries = function () {
      var params = this._params;
      var i = 0;
      return {
        next: function () {
          if (i < params.length) return { value: [params[i][0], params[i++][1]], done: false };
          return { value: undefined, done: true };
        },
      };
    };
    URLSearchParams.prototype.keys = function () {
      var params = this._params;
      var i = 0;
      return {
        next: function () {
          if (i < params.length) return { value: params[i++][0], done: false };
          return { value: undefined, done: true };
        },
      };
    };
    URLSearchParams.prototype.values = function () {
      var params = this._params;
      var i = 0;
      return {
        next: function () {
          if (i < params.length) return { value: params[i++][1], done: false };
          return { value: undefined, done: true };
        },
      };
    };
    URLSearchParams.prototype.forEach = function (fn, thisArg) {
      for (var i = 0; i < this._params.length; i++) {
        fn.call(thisArg, this._params[i][1], this._params[i][0], this);
      }
    };
    URLSearchParams.prototype.toString = function () {
      return this._params
        .map(function (p) {
          return encodeURIComponent(p[0]) + '=' + encodeURIComponent(p[1]);
        })
        .join('&');
    };
    URLSearchParams.prototype[Symbol.iterator] = URLSearchParams.prototype.entries;
    globalThis.URLSearchParams = URLSearchParams;
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
        this.protocol = '';
        this.hostname = '';
        this.port = '';
        this.pathname = url;
        this.search = '';
        this.hash = '';
        this.host = '';
        this.origin = '';
      }
      this.searchParams = new globalThis.URLSearchParams(this.search);
    }
    URL.prototype.toString = function () {
      return this.href;
    };
    globalThis.URL = URL;
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
    globalThis.fetch = function () {
      return Promise.reject(
        new Error('fetch not configured — use mockFetch() to register handlers'),
      );
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
    globalThis.Response.prototype.json = function () {
      return Promise.resolve(JSON.parse(this.body));
    };
    globalThis.Response.prototype.text = function () {
      return Promise.resolve(String(this.body));
    };
  }
})();
