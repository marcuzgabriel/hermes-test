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
  globalThis.process.nextTick = function(fn) {
    Promise.resolve().then(fn);
  };
}

// Object.fromEntries — ES2019, may not exist in older Hermes builds
if (typeof Object.fromEntries === 'undefined') {
  Object.fromEntries = function(iterable) {
    var obj = {};
    if (iterable && typeof iterable[Symbol.iterator] === 'function') {
      var iter = iterable[Symbol.iterator]();
      var next;
      while (!(next = iter.next()).done) {
        obj[next.value[0]] = next.value[1];
      }
    } else if (iterable && typeof iterable.forEach === 'function') {
      iterable.forEach(function(pair) { obj[pair[0]] = pair[1]; });
    }
    return obj;
  };
}

// crypto.getRandomValues — needed by uuid and other crypto-dependent libs
if (typeof globalThis.crypto === 'undefined') {
  globalThis.crypto = {};
}
if (typeof globalThis.crypto.getRandomValues === 'undefined') {
  globalThis.crypto.getRandomValues = function(arr) {
    for (var i = 0; i < arr.length; i++) {
      arr[i] = Math.floor(Math.random() * 256);
    }
    return arr;
  };
}

// MessageChannel polyfill — React 19's scheduler uses it for async work
if (typeof globalThis.MessageChannel === 'undefined') {
  globalThis.MessageChannel = function() {
    var cb = null;
    this.port1 = { onmessage: null };
    this.port2 = {
      postMessage: function() {
        if (cb) { var fn = cb; cb = null; fn({ data: undefined }); }
      }
    };
    var self = this;
    Object.defineProperty(this.port1, 'onmessage', {
      set: function(fn) { cb = fn; },
      get: function() { return cb; }
    });
  };
}

// Timer polyfills — React scheduler needs these
(function() {
  var queue = [];
  var timerIdCounter = 1;
  var timers = {};

  if (typeof globalThis.setImmediate === 'undefined') {
    globalThis.setImmediate = function(fn) { queue.push(fn); };
  }

  // Flush all async work: Hermes microtask queue (promises) + our polyfill queues (timers).
  // The C++ bridge installs a native __HT_drain that calls Hermes's drainMicrotasks().
  // We wrap it to also flush our setImmediate/setTimeout polyfill queues.
  var nativeDrain = globalThis.__HT_drain || function() {};
  globalThis.__HT_drain = function() {
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
          init.forEach(function(v, k) { this._params.push([String(k), String(v)]); }.bind(this));
        } else {
          // Plain object: { key: value }
          var keys = Object.keys(init);
          for (var i = 0; i < keys.length; i++) {
            this._params.push([keys[i], String(init[keys[i]])]);
          }
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
        if (this._params[i][0] === k) { this._params[i][1] = String(v); return; }
      }
      this._params.push([k, String(v)]);
    };
    URLSearchParams.prototype.append = function(k, v) { this._params.push([k, String(v)]); };
    URLSearchParams.prototype['delete'] = function(k) {
      this._params = this._params.filter(function(p) { return p[0] !== k; });
    };
    URLSearchParams.prototype.entries = function() {
      var params = this._params;
      var i = 0;
      return { next: function() {
        if (i < params.length) return { value: [params[i][0], params[i++][1]], done: false };
        return { value: undefined, done: true };
      }};
    };
    URLSearchParams.prototype.keys = function() {
      var params = this._params;
      var i = 0;
      return { next: function() {
        if (i < params.length) return { value: params[i++][0], done: false };
        return { value: undefined, done: true };
      }};
    };
    URLSearchParams.prototype.values = function() {
      var params = this._params;
      var i = 0;
      return { next: function() {
        if (i < params.length) return { value: params[i++][1], done: false };
        return { value: undefined, done: true };
      }};
    };
    URLSearchParams.prototype.forEach = function(fn, thisArg) {
      for (var i = 0; i < this._params.length; i++) {
        fn.call(thisArg, this._params[i][1], this._params[i][0], this);
      }
    };
    URLSearchParams.prototype.toString = function() {
      return this._params.map(function(p) { return encodeURIComponent(p[0]) + '=' + encodeURIComponent(p[1]); }).join('&');
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
        this.protocol = ''; this.hostname = ''; this.port = '';
        this.pathname = url; this.search = ''; this.hash = '';
        this.host = ''; this.origin = '';
      }
      this.searchParams = new globalThis.URLSearchParams(this.search);
    }
    URL.prototype.toString = function() { return this.href; };
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

"use strict";
var __metroTestHarness = (() => {
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
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
  var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

  // src/harness.ts
  var harness_exports = {};
  __export(harness_exports, {
    HttpResponse: () => HttpResponse,
    act: () => act,
    advanceTimersByTime: () => advanceTimersByTime,
    advanceTimersToNextTimer: () => advanceTimersToNextTimer,
    afterAll: () => afterAll,
    afterEach: () => afterEach,
    beforeAll: () => beforeAll,
    beforeEach: () => beforeEach,
    clearAllMocks: () => clearAllMocks,
    expect: () => expect,
    fireEvent: () => fireEvent,
    flushAsync: () => flushAsync,
    getTimerCount: () => getTimerCount,
    group: () => group,
    http: () => http,
    render: () => render,
    renderHook: () => renderHook,
    runAllTimers: () => runAllTimers,
    spy: () => spy,
    spyOn: () => spyOn,
    test: () => test,
    useFakeTimers: () => useFakeTimers,
    useMock: () => useMock,
    useRealTimers: () => useRealTimers,
    waitFor: () => waitFor
  });

  // src/expect.ts
  var _readFile = globalThis.__HT_readFile || (() => null);
  var _writeFile = globalThis.__HT_writeFile || (() => false);
  var _snapshotFile = "";
  var _snapshotTestName = "";
  var _snapshotCounter = 0;
  var _updateSnapshots = false;
  var _snapshotCache = {};
  function _setSnapshotContext(file, testName, update) {
    _snapshotFile = file;
    _snapshotTestName = testName;
    _snapshotCounter = 0;
    _updateSnapshots = update;
  }
  function _serializeSnapshot(value) {
    return JSON.stringify(value, (_key, val) => {
      if (typeof val === "function") return "[Function]";
      return val;
    }, 2);
  }
  function _loadSnapshots(path) {
    if (_snapshotCache[path]) return _snapshotCache[path];
    const content = _readFile(path);
    if (content) {
      try {
        _snapshotCache[path] = JSON.parse(content);
      } catch {
        _snapshotCache[path] = {};
      }
    } else {
      _snapshotCache[path] = {};
    }
    return _snapshotCache[path];
  }
  function _saveSnapshots(path, data) {
    _snapshotCache[path] = data;
    _writeFile(path, JSON.stringify(data, null, 2) + "\n");
  }
  function _matchSnapshot(actual) {
    _snapshotCounter++;
    const key = _snapshotTestName + (_snapshotCounter > 1 ? ` ${_snapshotCounter}` : "");
    const serialized = _serializeSnapshot(actual);
    if (!_snapshotFile) {
      throw new Error("toMatchSnapshot: no snapshot file configured. Is __currentTestFile set?");
    }
    const snapshots = _loadSnapshots(_snapshotFile);
    if (_updateSnapshots || !(key in snapshots)) {
      snapshots[key] = serialized;
      _saveSnapshots(_snapshotFile, snapshots);
      return;
    }
    const expected = snapshots[key];
    if (serialized !== expected) {
      throw new Error(
        `Snapshot mismatch for "${key}":
Expected:
${expected}

Received:
${serialized}

Run with --update-snapshots to update.`
      );
    }
  }
  function deepEqual(a, b) {
    if (b != null && typeof b === "object" && b.__htMatcher && typeof b.matches === "function") return b.matches(a);
    if (a != null && typeof a === "object" && a.__htMatcher && typeof a.matches === "function") return a.matches(b);
    if (a === b) return true;
    if (a == null || b == null) return a === b;
    if (typeof a !== typeof b) return false;
    if (Array.isArray(a)) {
      if (!Array.isArray(b) || a.length !== b.length) return false;
      return a.every((v, i) => deepEqual(v, b[i]));
    }
    if (a instanceof Date && b instanceof Date) {
      return a.getTime() === b.getTime();
    }
    if (typeof a === "object") {
      const keysA = Object.keys(a).filter((k) => a[k] !== void 0);
      const keysB = Object.keys(b).filter((k) => b[k] !== void 0);
      if (keysA.length !== keysB.length) return false;
      return keysA.every((k) => deepEqual(a[k], b[k]));
    }
    return false;
  }
  function formatValue(v) {
    if (v === void 0) return "undefined";
    if (v === null) return "null";
    if (typeof v === "string") return JSON.stringify(v);
    if (typeof v === "function") return "[Function]";
    try {
      return JSON.stringify(v);
    } catch {
      return String(v);
    }
  }
  function _getTextContent(node) {
    if (!node || typeof node !== "object") return "";
    if (node.type === "__TEXT__") return node.text || "";
    if (!node.children) return "";
    return node.children.map(_getTextContent).join("");
  }
  function createAssertion(actual, negated) {
    function assert(condition, message) {
      const pass = negated ? !condition : condition;
      if (!pass) {
        let hint = "";
        if (actual === void 0 && !negated) {
          hint = "\n    Hint: Received is undefined. This usually means the module needs to be mocked with ht.mock().";
        }
        throw new Error(message + hint);
      }
    }
    const assertion = {
      toBe(expected) {
        assert(
          actual === expected,
          negated ? `expect(received).not.toBe(expected)

    Expected: not ${formatValue(expected)}
    Received: ${formatValue(actual)}` : `expect(received).toBe(expected)

    Expected: ${formatValue(expected)}
    Received: ${formatValue(actual)}`
        );
      },
      toEqual(expected) {
        assert(
          deepEqual(actual, expected),
          negated ? `expect(received).not.toEqual(expected)

    Expected: not ${formatValue(expected)}
    Received: ${formatValue(actual)}` : `expect(received).toEqual(expected)

    Expected: ${formatValue(expected)}
    Received: ${formatValue(actual)}`
        );
      },
      toBeDefined() {
        assert(
          actual !== void 0,
          negated ? `expect(received).not.toBeDefined()

    Received: ${formatValue(actual)}` : `expect(received).toBeDefined()

    Received: undefined`
        );
      },
      toBeUndefined() {
        assert(
          actual === void 0,
          negated ? `expect(received).not.toBeUndefined()

    Received: ${formatValue(actual)}` : `expect(received).toBeUndefined()

    Received: ${formatValue(actual)}`
        );
      },
      toBeNull() {
        assert(
          actual === null,
          negated ? `expect(received).not.toBeNull()

    Received: ${formatValue(actual)}` : `expect(received).toBeNull()

    Received: ${formatValue(actual)}`
        );
      },
      toHaveLength(expected) {
        const len = actual?.length;
        assert(
          len === expected,
          negated ? `expect(received).not.toHaveLength(expected)

    Expected: not ${expected}
    Received length: ${len}` : `expect(received).toHaveLength(expected)

    Expected: ${expected}
    Received length: ${len}`
        );
      },
      toBeInstanceOf(expected) {
        assert(
          actual instanceof expected,
          negated ? `expect(received).not.toBeInstanceOf(expected)

    Expected: not ${expected?.name ?? expected}` : `expect(received).toBeInstanceOf(expected)

    Expected: ${expected?.name ?? expected}
    Received: ${formatValue(actual)}`
        );
      },
      toBeTruthy() {
        assert(
          !!actual,
          negated ? `expect(received).not.toBeTruthy()

    Received: ${formatValue(actual)}` : `expect(received).toBeTruthy()

    Received: ${formatValue(actual)}`
        );
      },
      toBeFalsy() {
        assert(
          !actual,
          negated ? `expect(received).not.toBeFalsy()

    Received: ${formatValue(actual)}` : `expect(received).toBeFalsy()

    Received: ${formatValue(actual)}`
        );
      },
      toBeGreaterThan(n) {
        assert(
          actual > n,
          negated ? `Expected ${actual} not to be greater than ${n}` : `Expected ${actual} to be greater than ${n}`
        );
      },
      toBeLessThan(n) {
        assert(
          actual < n,
          negated ? `Expected ${actual} not to be less than ${n}` : `Expected ${actual} to be less than ${n}`
        );
      },
      toContain(item) {
        const contains = Array.isArray(actual) ? actual.some((v) => deepEqual(v, item)) : typeof actual === "string" ? actual.includes(item) : false;
        assert(
          contains,
          negated ? `Expected ${formatValue(actual)} not to contain ${formatValue(item)}` : `Expected ${formatValue(actual)} to contain ${formatValue(item)}`
        );
      },
      toContainEqual(item) {
        const contains = Array.isArray(actual) && actual.some((v) => deepEqual(v, item));
        assert(
          contains,
          negated ? `Expected array not to contain equal ${formatValue(item)}` : `Expected array to contain equal ${formatValue(item)}, got ${formatValue(actual)}`
        );
      },
      toBeCloseTo(expected, precision = 2) {
        const pass = Math.abs(actual - expected) < Math.pow(10, -precision) / 2;
        assert(
          pass,
          negated ? `Expected ${actual} not to be close to ${expected}` : `Expected ${actual} to be close to ${expected} (precision ${precision})`
        );
      },
      toMatch(pattern) {
        const regex = typeof pattern === "string" ? new RegExp(pattern) : pattern;
        assert(
          regex.test(String(actual)),
          negated ? `Expected ${formatValue(actual)} not to match ${pattern}` : `Expected ${formatValue(actual)} to match ${pattern}`
        );
      },
      toThrow(message) {
        let threw = false;
        let error;
        try {
          actual();
        } catch (e) {
          threw = true;
          error = e;
        }
        if (message === void 0) {
          assert(
            threw,
            negated ? `Expected function not to throw, but it threw ${formatValue(error)}` : `Expected function to throw, but it did not`
          );
        } else {
          const errMsg = error?.message ?? String(error ?? "");
          const matches = typeof message === "string" ? errMsg.includes(message) : message.test(errMsg);
          assert(
            threw && matches,
            negated ? `Expected function not to throw matching ${message}` : threw ? `Expected thrown error to match ${message}, got "${errMsg}"` : `Expected function to throw, but it did not`
          );
        }
      },
      // Spy assertions
      wasCalled() {
        assert(
          actual.callCount > 0,
          negated ? `Expected spy not to have been called, but it was called ${actual.callCount} times` : `Expected spy to have been called, but it was never called`
        );
      },
      wasCalledOnce() {
        assert(
          actual.callCount === 1,
          negated ? `Expected spy not to have been called once, but it was` : `Expected spy to have been called once, but it was called ${actual.callCount} times`
        );
      },
      wasCalledTimes(n) {
        assert(
          actual.callCount === n,
          negated ? `Expected spy not to have been called ${n} times` : `Expected spy to have been called ${n} times, but it was called ${actual.callCount} times`
        );
      },
      wasCalledWith(...args) {
        const s = actual;
        const match = s.calls.some((call) => deepEqual(call, args));
        assert(
          match,
          negated ? `Expected spy not to have been called with ${formatValue(args)}` : `Expected spy to have been called with ${formatValue(args)}, calls: ${formatValue(s.calls)}`
        );
      },
      wasLastCalledWith(...args) {
        const s = actual;
        const lastCall = s.calls[s.calls.length - 1];
        assert(
          deepEqual(lastCall, args),
          negated ? `Expected last call not to be ${formatValue(args)}` : `Expected last call to be ${formatValue(args)}, got ${formatValue(lastCall)}`
        );
      },
      wasNeverCalled() {
        assert(
          actual.callCount === 0,
          negated ? `Expected spy to have been called, but it was never called` : `Expected spy to never have been called, but it was called ${actual.callCount} times`
        );
      },
      // Jest-compatible aliases
      toHaveBeenCalled() {
        return this.wasCalled();
      },
      toHaveBeenCalledTimes(n) {
        return this.wasCalledTimes(n);
      },
      toHaveBeenCalledWith(...args) {
        return this.wasCalledWith(...args);
      },
      toHaveBeenLastCalledWith(...args) {
        return this.wasLastCalledWith(...args);
      },
      // --- Element matchers (for render() HTNode results) ---
      toBeRendered() {
        const el = actual;
        const isNode = el && typeof el === "object" && "type" in el && "children" in el;
        assert(
          isNode && el.type !== "__ROOT__",
          negated ? `Expected element not to be rendered` : `Expected element to be rendered, got ${formatValue(el)}`
        );
      },
      toHaveTextContent(expected) {
        const text = _getTextContent(actual);
        const matches = typeof expected === "string" ? text === expected || text.includes(expected) : expected.test(text);
        assert(
          matches,
          negated ? `Expected element not to have text content "${expected}", but it does` : `Expected text content "${expected}", got "${text}"`
        );
      },
      toContainElement(child) {
        function _contains(node, target) {
          if (node === target) return true;
          if (!node?.children) return false;
          return node.children.some((c) => _contains(c, target));
        }
        assert(
          _contains(actual, child),
          negated ? `Expected element not to contain the given child` : `Expected element to contain the given child`
        );
      },
      toBeEmpty() {
        const empty = !actual?.children || actual.children.length === 0;
        assert(
          empty,
          negated ? `Expected element not to be empty, but it has no children` : `Expected element to be empty, but it has ${actual?.children?.length} children`
        );
      },
      toHaveDisplayValue(expected) {
        const value = actual?.props?.value ?? "";
        const matches = typeof expected === "string" ? value === expected : expected.test(value);
        assert(
          matches,
          negated ? `Expected display value not to be "${expected}"` : `Expected display value "${expected}", got "${value}"`
        );
      },
      toHaveProp(name, value) {
        const hasProp = actual?.props && name in actual.props;
        if (value === void 0) {
          assert(
            hasProp,
            negated ? `Expected element not to have prop "${name}"` : `Expected element to have prop "${name}"`
          );
        } else {
          const propVal = actual?.props?.[name];
          assert(
            hasProp && deepEqual(propVal, value),
            negated ? `Expected prop "${name}" not to be ${formatValue(value)}` : `Expected prop "${name}" to be ${formatValue(value)}, got ${formatValue(propVal)}`
          );
        }
      },
      toHaveStyle(expected) {
        const style = actual?.props?.style || {};
        const flat = {};
        const styles = Array.isArray(style) ? style : [style];
        for (const s of styles) {
          if (s && typeof s === "object") Object.assign(flat, s);
        }
        const allMatch = Object.keys(expected).every((k) => deepEqual(flat[k], expected[k]));
        const mismatches = Object.keys(expected).filter((k) => !deepEqual(flat[k], expected[k])).map((k) => `${k}: expected ${formatValue(expected[k])}, got ${formatValue(flat[k])}`);
        assert(
          allMatch,
          negated ? `Expected element not to have styles ${formatValue(expected)}` : `Style mismatch: ${mismatches.join("; ")}`
        );
      },
      toBeEnabled() {
        const disabled = actual?.props?.disabled === true || actual?.props?.editable === false || actual?.props?.accessibilityState?.disabled === true || actual?.props?.["aria-disabled"] === true;
        assert(
          !disabled,
          negated ? `Expected element to be disabled, but it is enabled` : `Expected element to be enabled, but it is disabled`
        );
      },
      toBeDisabled() {
        const disabled = actual?.props?.disabled === true || actual?.props?.editable === false || actual?.props?.accessibilityState?.disabled === true || actual?.props?.["aria-disabled"] === true;
        assert(
          disabled,
          negated ? `Expected element not to be disabled, but it is` : `Expected element to be disabled, but it is enabled`
        );
      },
      toBeVisible() {
        const style = actual?.props?.style || {};
        const styles = Array.isArray(style) ? style : [style];
        const flat = {};
        for (const s of styles) {
          if (s && typeof s === "object") Object.assign(flat, s);
        }
        const hidden = flat.display === "none" || flat.opacity === 0 || actual?.props?.accessibilityElementsHidden === true || actual?.props?.importantForAccessibility === "no-hide-descendants";
        assert(
          !hidden,
          negated ? `Expected element not to be visible` : `Expected element to be visible, but it is hidden`
        );
      },
      // --- Snapshot matcher ---
      toMatchSnapshot() {
        _matchSnapshot(actual);
      }
    };
    if (!negated) {
      assertion.not = createAssertion(actual, true);
    }
    return assertion;
  }
  function makeMatcher(matchFn) {
    return { __htMatcher: true, matches: matchFn };
  }
  function expect(actual) {
    const base = createAssertion(actual, false);
    base.resolves = {
      toBeUndefined: async () => {
        const r = await actual;
        if (r !== void 0) throw new Error(`Expected undefined, got ${formatValue(r)}`);
      },
      toBe: async (expected) => {
        const r = await actual;
        if (r !== expected) throw new Error(`Expected ${formatValue(expected)}, got ${formatValue(r)}`);
      },
      toEqual: async (expected) => {
        const r = await actual;
        if (!deepEqual(r, expected)) throw new Error(`Expected deep equal to ${formatValue(expected)}, got ${formatValue(r)}`);
      },
      toBeDefined: async () => {
        const r = await actual;
        if (r === void 0) throw new Error(`Expected value to be defined`);
      },
      toBeTruthy: async () => {
        const r = await actual;
        if (!r) throw new Error(`Expected truthy, got ${formatValue(r)}`);
      },
      toBeFalsy: async () => {
        const r = await actual;
        if (r) throw new Error(`Expected falsy, got ${formatValue(r)}`);
      },
      toBeNull: async () => {
        const r = await actual;
        if (r !== null) throw new Error(`Expected null, got ${formatValue(r)}`);
      }
    };
    base.rejects = {
      toThrow: async (msg) => {
        try {
          await actual;
          throw new Error("Expected promise to reject");
        } catch (e) {
          if (msg) {
            const m = e?.message ?? String(e);
            const ok = typeof msg === "string" ? m.includes(msg) : msg.test(m);
            if (!ok) throw new Error(`Expected rejection matching ${msg}, got "${m}"`);
          }
        }
      }
    };
    return base;
  }
  expect.anything = () => makeMatcher((v) => v !== null && v !== void 0);
  expect.any = (ctor) => makeMatcher((v) => {
    if (ctor === String) return typeof v === "string";
    if (ctor === Number) return typeof v === "number";
    if (ctor === Boolean) return typeof v === "boolean";
    if (ctor === Function) return typeof v === "function";
    return v instanceof ctor;
  });
  expect.objectContaining = (subset) => makeMatcher((v) => {
    if (typeof v !== "object" || v === null) return false;
    return Object.keys(subset).every((k) => deepEqual(v[k], subset[k]));
  });
  expect.arrayContaining = (expected) => makeMatcher((v) => {
    if (!Array.isArray(v)) return false;
    return expected.every((e) => v.some((item) => deepEqual(item, e)));
  });
  expect.stringContaining = (substr) => makeMatcher((v) => typeof v === "string" && v.includes(substr));
  expect.stringMatching = (pattern) => makeMatcher((v) => {
    const re = typeof pattern === "string" ? new RegExp(pattern) : pattern;
    return typeof v === "string" && re.test(v);
  });

  // src/spy.ts
  var _allSpies = [];
  function clearAllMocks() {
    for (const s of _allSpies) s.mockClear();
  }
  function spy(impl) {
    let baseImpl = impl;
    const onceImpls = [];
    const calls = [];
    const returnValues = [];
    const fn = function(...args) {
      calls.push(args);
      let ret;
      if (onceImpls.length > 0) {
        const onceFn = onceImpls.shift();
        ret = onceFn.apply(this, args);
      } else {
        ret = baseImpl ? baseImpl.apply(this, args) : void 0;
      }
      returnValues.push(ret);
      return ret;
    };
    Object.defineProperties(fn, {
      calls: { get: () => calls },
      callCount: { get: () => calls.length },
      returnValues: { get: () => returnValues },
      _isSpy: { value: true }
    });
    fn.reset = () => {
      calls.length = 0;
      returnValues.length = 0;
      onceImpls.length = 0;
    };
    fn.setImpl = (newImpl) => {
      baseImpl = newImpl;
      return fn;
    };
    fn.returns = (value) => {
      baseImpl = (() => value);
      return fn;
    };
    fn.mockImplementation = (newImpl) => {
      baseImpl = newImpl;
      return fn;
    };
    fn.mockImplementationOnce = (onceFn) => {
      onceImpls.push(onceFn);
      return fn;
    };
    fn.mockReturnValue = (value) => {
      baseImpl = (() => value);
      return fn;
    };
    fn.mockReturnValueOnce = (value) => {
      onceImpls.push((() => value));
      return fn;
    };
    fn.mockResolvedValue = (value) => {
      baseImpl = (() => Promise.resolve(value));
      return fn;
    };
    fn.mockResolvedValueOnce = (value) => {
      onceImpls.push((() => Promise.resolve(value)));
      return fn;
    };
    fn.mockRejectedValue = (value) => {
      baseImpl = (() => Promise.reject(value));
      return fn;
    };
    fn.mockRejectedValueOnce = (value) => {
      onceImpls.push((() => Promise.reject(value)));
      return fn;
    };
    fn.mockClear = () => {
      calls.length = 0;
      returnValues.length = 0;
      onceImpls.length = 0;
    };
    fn.mockReset = () => {
      calls.length = 0;
      returnValues.length = 0;
      onceImpls.length = 0;
      baseImpl = void 0;
    };
    fn.mockRestore = () => {
      fn.mockReset();
      if (fn._restore) fn._restore();
    };
    _allSpies.push(fn);
    return fn;
  }
  function spyOn(obj, method) {
    const original = obj[method];
    const s = spy(typeof original === "function" ? original.bind(obj) : void 0);
    s._restore = () => {
      obj[method] = original;
    };
    obj[method] = s;
    return s;
  }

  // src/hooks.ts
  function getReact() {
    const R = globalThis.__HT_React;
    if (!R) throw new Error("React not available. Make sure react is installed in your project.");
    return R;
  }
  function getReconcilerModule() {
    const R = globalThis.__HT_Reconciler;
    if (!R) throw new Error("react-reconciler not available. Make sure it is installed (it ships with hermes-test).");
    return R;
  }
  function getReconcilerConstants() {
    return globalThis.__HT_ReconcilerConstants || {};
  }
  var currentUpdatePriority = 0;
  var hostConfig = {
    supportsMutation: true,
    supportsPersistence: false,
    supportsHydration: false,
    supportsMicrotasks: true,
    isPrimaryRenderer: true,
    warnsIfNotActing: true,
    createInstance(type, props) {
      const { children: _c, ...rest } = props;
      return { type, props: rest, children: [] };
    },
    createTextInstance(text) {
      return { type: "__TEXT__", props: {}, text, children: [] };
    },
    appendInitialChild(p, c) {
      p.children.push(c);
    },
    appendChild(p, c) {
      p.children.push(c);
    },
    appendChildToContainer(p, c) {
      p.children.push(c);
    },
    removeChild(p, c) {
      const i = p.children.indexOf(c);
      if (i !== -1) p.children.splice(i, 1);
    },
    removeChildFromContainer(p, c) {
      const i = p.children.indexOf(c);
      if (i !== -1) p.children.splice(i, 1);
    },
    insertBefore(p, c, b) {
      const i = p.children.indexOf(b);
      p.children.splice(i, 0, c);
    },
    insertInContainerBefore(p, c, b) {
      const i = p.children.indexOf(b);
      p.children.splice(i, 0, c);
    },
    commitUpdate(inst, _type, _oldProps, newProps) {
      const { children: _c, ...rest } = newProps;
      inst.props = rest;
    },
    commitTextUpdate(inst, _oldText, newText) {
      inst.text = newText;
    },
    commitMount() {
    },
    prepareForCommit() {
      return null;
    },
    resetAfterCommit() {
    },
    resetTextContent() {
    },
    finalizeInitialChildren() {
      return false;
    },
    shouldSetTextContent() {
      return false;
    },
    getRootHostContext() {
      return null;
    },
    getChildHostContext(ctx) {
      return ctx;
    },
    getPublicInstance(inst) {
      return inst;
    },
    prepareUpdate() {
      return {};
    },
    clearContainer(c) {
      c.children = [];
    },
    scheduleTimeout: globalThis.setTimeout || ((fn) => fn()),
    cancelTimeout: globalThis.clearTimeout || (() => {
    }),
    noTimeout: -1,
    scheduleMicrotask: typeof queueMicrotask === "function" ? queueMicrotask : (fn) => Promise.resolve().then(fn),
    getCurrentEventPriority() {
      return getReconcilerConstants().DefaultEventPriority ?? 0;
    },
    setCurrentUpdatePriority(priority) {
      currentUpdatePriority = priority;
    },
    getCurrentUpdatePriority() {
      return currentUpdatePriority;
    },
    resolveUpdatePriority() {
      return currentUpdatePriority || (getReconcilerConstants().DefaultEventPriority ?? 0);
    },
    shouldAttemptEagerTransition() {
      return false;
    },
    trackSchedulerEvent() {
    },
    resolveEventType() {
      return "";
    },
    resolveEventTimeStamp() {
      return -1.1;
    },
    requestPostPaintCallback() {
    },
    maySuspendCommit() {
      return false;
    },
    preloadInstance() {
      return true;
    },
    startSuspendingCommit() {
    },
    suspendInstance() {
    },
    waitForCommitToBeReady() {
      return null;
    },
    NotPendingTransition: null,
    resetFormInstance() {
    },
    hideInstance() {
    },
    unhideInstance() {
    },
    hideTextInstance() {
    },
    unhideTextInstance() {
    },
    getInstanceFromNode() {
      return null;
    },
    prepareScopeUpdate() {
    },
    getInstanceFromScope() {
      return null;
    },
    detachDeletedInstance() {
    },
    beforeActiveInstanceBlur() {
    },
    afterActiveInstanceBlur() {
    },
    preparePortalMount() {
    }
  };
  function createReconciler() {
    const Reconciler = getReconcilerModule();
    const create = typeof Reconciler === "function" ? Reconciler : Reconciler.default;
    return create(hostConfig);
  }
  var drain = globalThis.__HT_drain || (() => {
  });
  function flush() {
    drain();
  }
  globalThis.IS_REACT_ACT_ENVIRONMENT = false;
  function act(fn) {
    const React = getReact();
    const reactAct = React.act || React.unstable_act;
    if (!reactAct) {
      fn();
      flush();
      return;
    }
    const prev = globalThis.IS_REACT_ACT_ENVIRONMENT;
    globalThis.IS_REACT_ACT_ENVIRONMENT = true;
    try {
      reactAct(() => {
        const result = fn();
        if (result && typeof result.then === "function") {
          let settled = false;
          let error;
          result.then(
            () => {
              settled = true;
            },
            (e) => {
              settled = true;
              error = e;
            }
          );
          for (let i = 0; i < 50 && !settled; i++) {
            drain();
          }
          if (error) throw error;
        }
      });
      globalThis.IS_REACT_ACT_ENVIRONMENT = prev;
      flush();
    } catch (error) {
      globalThis.IS_REACT_ACT_ENVIRONMENT = prev;
      throw error;
    }
  }
  function renderHook(hookFn, options) {
    const history = [];
    let currentValue;
    const React = getReact();
    const reconciler = createReconciler();
    const container = { children: [] };
    const root = reconciler.createContainer(
      container,
      0,
      // LegacyRoot — effects fire synchronously in act()
      null,
      // hydrationCallbacks
      false,
      // isStrictMode
      false,
      // concurrentUpdatesByDefaultOverride
      "",
      // identifierPrefix
      (err) => {
        throw err;
      },
      // onUncaughtError
      (err) => {
        throw err;
      },
      // onCaughtError
      null,
      // onRecoverableError
      () => {
      }
      // onDefaultTransitionIndicator
    );
    function TestComponent({ hookProps }) {
      const value = hookFn(hookProps);
      currentValue = value;
      history.push(value);
      return null;
    }
    function createTree(props) {
      const testEl = React.createElement(TestComponent, { hookProps: props });
      if (options?.wrapper) {
        return React.createElement(options.wrapper, null, testEl);
      }
      return testEl;
    }
    act(() => {
      reconciler.updateContainer(createTree(options?.initialProps), root, null, null);
    });
    return {
      result: {
        get current() {
          return currentValue;
        }
      },
      get current() {
        return currentValue;
      },
      get history() {
        return history;
      },
      get renderCount() {
        return history.length;
      },
      rerender(props) {
        act(() => {
          reconciler.updateContainer(createTree(props), root, null, null);
        });
      },
      unmount() {
        act(() => {
          reconciler.updateContainer(null, root, null, null);
        });
      }
    };
  }
  function waitFor(predicate, options) {
    const timeout = options?.timeout ?? 1e3;
    const start = Date.now();
    for (let attempt = 0; attempt < 100; attempt++) {
      act(() => {
        drain();
      });
      drain();
      const result = predicate();
      if (result !== false && result !== null && result !== void 0) {
        return result;
      }
      if (Date.now() - start >= timeout) {
        throw new Error(`waitFor timed out after ${timeout}ms`);
      }
    }
    throw new Error(`waitFor exceeded max attempts`);
  }

  // src/render.ts
  function getAllNodes(root) {
    const result = [];
    function walk(node) {
      result.push(node);
      for (const child of node.children) walk(child);
    }
    for (const child of root.children) walk(child);
    return result;
  }
  function getTextContent(node) {
    if (node.type === "__TEXT__") return node.text || "";
    return node.children.map(getTextContent).join("");
  }
  function textMatches(content, text) {
    return typeof text === "string" ? content === text : text.test(content);
  }
  function queryAllByText(root, text) {
    const all = getAllNodes(root).filter((n) => {
      if (n.type === "__TEXT__") return false;
      const content = getTextContent(n);
      return content ? textMatches(content, text) : false;
    });
    return all.filter((n) => !n.children.some((c) => c.type !== "__TEXT__" && all.includes(c)));
  }
  function queryAllByTestId(root, testID) {
    return getAllNodes(root).filter((n) => {
      const id = n.props?.testID;
      if (!id) return false;
      return typeof testID === "string" ? id === testID : testID.test(id);
    });
  }
  function queryAllByProps(root, props) {
    return getAllNodes(root).filter((n) => {
      for (const key of Object.keys(props)) {
        if (n.props?.[key] !== props[key]) return false;
      }
      return true;
    });
  }
  function queryAllByType(root, type) {
    return getAllNodes(root).filter((n) => n.type === type);
  }
  function makeQuery(queryAll, label) {
    return {
      getAll(root, arg) {
        const result = queryAll(root, arg);
        if (result.length === 0) throw new Error(`Unable to find element with ${label}: ${String(arg)}`);
        return result;
      },
      get(root, arg) {
        const result = queryAll(root, arg);
        if (result.length === 0) throw new Error(`Unable to find element with ${label}: ${String(arg)}`);
        if (result.length > 1) throw new Error(`Found ${result.length} elements with ${label}: ${String(arg)}`);
        return result[0];
      },
      queryAll(root, arg) {
        return queryAll(root, arg);
      },
      query(root, arg) {
        const result = queryAll(root, arg);
        if (result.length > 1) throw new Error(`Found ${result.length} elements with ${label}: ${String(arg)}`);
        return result[0] || null;
      }
    };
  }
  var textQ = makeQuery(queryAllByText, "text");
  var testIdQ = makeQuery(queryAllByTestId, "testID");
  var propsQ = makeQuery(queryAllByProps, "props");
  var typeQ = makeQuery(queryAllByType, "type");
  function toJSON(node) {
    if (node.type === "__TEXT__") return node.text || "";
    const children = node.children.map(toJSON);
    const cleanProps = {};
    for (const [k, v] of Object.entries(node.props || {})) {
      if (typeof v === "function") {
        cleanProps[k] = "[Function]";
      } else {
        cleanProps[k] = v;
      }
    }
    return {
      type: node.type,
      props: Object.keys(cleanProps).length > 0 ? cleanProps : void 0,
      children: children.length > 0 ? children : void 0
    };
  }
  function prettyPrint(json, indent = 0) {
    const pad = "  ".repeat(indent);
    if (typeof json === "string") return `${pad}${json}`;
    const { type, props, children } = json;
    let propsStr = "";
    if (props) {
      const entries = Object.entries(props).map(
        ([k, v]) => typeof v === "string" ? `${k}="${v}"` : `${k}={${JSON.stringify(v)}}`
      );
      if (entries.length > 0) propsStr = " " + entries.join(" ");
    }
    if (!children || children.length === 0) {
      return `${pad}<${type}${propsStr} />`;
    }
    const merged = [];
    for (const c of children) {
      if (typeof c === "string" && merged.length > 0 && typeof merged[merged.length - 1] === "string") {
        merged[merged.length - 1] += c;
      } else {
        merged.push(c);
      }
    }
    if (merged.length === 1 && typeof merged[0] === "string") {
      return `${pad}<${type}${propsStr}>${merged[0]}</${type}>`;
    }
    const childrenStr = merged.map((c) => prettyPrint(c, indent + 1)).join("\n");
    return `${pad}<${type}${propsStr}>
${childrenStr}
${pad}</${type}>`;
  }
  var fireEvent = Object.assign(
    function fireEvent2(node, eventName, ...args) {
      const handlerName = "on" + eventName.charAt(0).toUpperCase() + eventName.slice(1);
      const handler = node.props?.[handlerName];
      if (!handler) throw new Error(`No handler "${handlerName}" on <${node.type}>`);
      act(() => {
        handler(...args);
      });
    },
    {
      press(node, event) {
        const handler = node.props?.onPress;
        if (!handler) throw new Error(`No "onPress" handler on <${node.type}>`);
        act(() => {
          handler(event);
        });
      },
      changeText(node, text) {
        const handler = node.props?.onChangeText;
        if (!handler) throw new Error(`No "onChangeText" handler on <${node.type}>`);
        act(() => {
          handler(text);
        });
      },
      scroll(node, event) {
        const handler = node.props?.onScroll;
        if (!handler) throw new Error(`No "onScroll" handler on <${node.type}>`);
        act(() => {
          handler(event);
        });
      }
    }
  );
  function render(element, options) {
    const reconciler = createReconciler();
    const container = { type: "__ROOT__", props: {}, children: [] };
    const root = reconciler.createContainer(
      container,
      0,
      // LegacyRoot
      null,
      // hydrationCallbacks
      false,
      false,
      "",
      (err) => {
        throw err;
      },
      (err) => {
        throw err;
      },
      null,
      () => {
      }
    );
    const React = globalThis.__HT_React;
    if (options?.shallow && React) {
      const topType = element.type;
      const origCE = React.createElement;
      React.createElement = function(type, ...args) {
        if (typeof type === "function" && type !== topType) {
          const name = type.displayName || type.name || "Component";
          return origCE.call(React, name, ...args);
        }
        return origCE.call(React, type, ...args);
      };
      act(() => {
        reconciler.updateContainer(element, root, null, null);
      });
      React.createElement = origCE;
    } else {
      act(() => {
        reconciler.updateContainer(element, root, null, null);
      });
    }
    const result = {
      container,
      getByText: (t) => textQ.get(container, t),
      getAllByText: (t) => textQ.getAll(container, t),
      queryByText: (t) => textQ.query(container, t),
      queryAllByText: (t) => textQ.queryAll(container, t),
      getByTestId: (id) => testIdQ.get(container, id),
      getAllByTestId: (id) => testIdQ.getAll(container, id),
      queryByTestId: (id) => testIdQ.query(container, id),
      queryAllByTestId: (id) => testIdQ.queryAll(container, id),
      getByProps: (p) => propsQ.get(container, p),
      getAllByProps: (p) => propsQ.getAll(container, p),
      queryByProps: (p) => propsQ.query(container, p),
      queryAllByProps: (p) => propsQ.queryAll(container, p),
      getByType: (t) => typeQ.get(container, t),
      getAllByType: (t) => typeQ.getAll(container, t),
      queryByType: (t) => typeQ.query(container, t),
      queryAllByType: (t) => typeQ.queryAll(container, t),
      toJSON() {
        if (container.children.length === 0) return null;
        if (container.children.length === 1) return toJSON(container.children[0]);
        return container.children.map(toJSON);
      },
      toTree() {
        const json = result.toJSON();
        if (json === null) return "";
        if (Array.isArray(json)) return json.map((j) => prettyPrint(j)).join("\n");
        return prettyPrint(json);
      },
      rerender(el) {
        act(() => {
          reconciler.updateContainer(el, root, null, null);
        });
      },
      unmount() {
        act(() => {
          reconciler.updateContainer(null, root, null, null);
        });
      }
    };
    return result;
  }

  // src/mock.ts
  var savedDescriptors = [];
  var mockRegistry = globalThis.__HT_mocks || {};
  globalThis.__HT_mocks = mockRegistry;
  var fileMocks = globalThis.__HT_file_mocks || (globalThis.__HT_file_mocks = {});
  var mockModulePatches = [];
  function mockModule(modulePath, factory) {
    const impl = factory();
    const value = typeof impl === "function" ? impl : wrapWithSpies(impl);
    const currentFile = globalThis.__currentTestFile || "__global__";
    if (!fileMocks[currentFile]) fileMocks[currentFile] = {};
    fileMocks[currentFile][modulePath] = value;
    const globalMock = mockRegistry[modulePath];
    if (globalMock && typeof globalMock === "object" && typeof value === "object") {
      for (const key of Object.keys(value)) {
        if (key === "default" || key === "__esModule") continue;
        try {
          if (key in globalMock) {
            mockModulePatches.push({ target: globalMock, key, original: globalMock[key] });
            globalMock[key] = value[key];
          }
        } catch {
        }
      }
      if ("default" in value && "default" in globalMock) {
        const mockDefault = value["default"];
        const realDefault = globalMock["default"];
        if (realDefault && typeof realDefault === "object" && typeof mockDefault === "object") {
          for (const key of Object.keys(mockDefault)) {
            try {
              if (key in realDefault) {
                mockModulePatches.push({ target: realDefault, key, original: realDefault[key] });
                realDefault[key] = mockDefault[key];
              }
            } catch {
            }
          }
        }
      }
    }
  }
  function resetMockModulePatches() {
    for (const { target, key, original } of mockModulePatches) {
      try {
        target[key] = original;
      } catch {
      }
    }
    mockModulePatches = [];
  }
  function wrapWithSpies(impl) {
    const wrapped = {};
    for (const key of Object.keys(impl)) {
      const value = impl[key];
      if (typeof value === "function" && !value._isSpy) {
        wrapped[key] = spy(value);
      } else {
        wrapped[key] = value;
      }
    }
    return wrapped;
  }
  function useMock(moduleExports, implementation) {
    const wrapped = wrapWithSpies(implementation);
    for (const key of Object.keys(wrapped)) {
      const desc = Object.getOwnPropertyDescriptor(moduleExports, key);
      if (desc) {
        savedDescriptors.push({ target: moduleExports, key, desc });
      }
      const mockValue = wrapped[key];
      try {
        Object.defineProperty(moduleExports, key, {
          get: () => mockValue,
          configurable: true,
          enumerable: true
        });
      } catch {
        try {
          moduleExports[key] = mockValue;
        } catch {
        }
      }
    }
    return wrapped;
  }
  function resetMocks() {
    for (const { target, key, desc } of savedDescriptors) {
      try {
        Object.defineProperty(target, key, desc);
      } catch {
      }
    }
    savedDescriptors = [];
  }

  // src/fetch.ts
  var handlers = [];
  var overrideHandlers = [];
  function matchUrl(pattern, url) {
    if (pattern instanceof RegExp) return pattern.test(url);
    if (url === pattern) return true;
    if (url.startsWith(pattern + "?")) return true;
    const urlBase = url.split("?")[0];
    return urlBase === pattern;
  }
  function findHandler(method, url) {
    for (let i = overrideHandlers.length - 1; i >= 0; i--) {
      const h = overrideHandlers[i];
      if (h.method === method.toUpperCase() && matchUrl(h.url, url)) {
        if (h.once) overrideHandlers.splice(i, 1);
        return h;
      }
    }
    for (let i = handlers.length - 1; i >= 0; i--) {
      const h = handlers[i];
      if (h.method === method.toUpperCase() && matchUrl(h.url, url)) {
        if (h.once) handlers.splice(i, 1);
        return h;
      }
    }
    return void 0;
  }
  function fakeFetch(input, init) {
    let url;
    if (typeof input === "string") {
      url = input;
    } else if (input && typeof input === "object") {
      url = input.url || input.href || String(input);
      if (!init && input.method) {
        init = { method: input.method, headers: input.headers, body: input.body };
      }
    } else {
      url = String(input);
    }
    const method = (init?.method || "GET").toUpperCase();
    let body = init?.body;
    if (typeof body === "string") {
      try {
        body = JSON.parse(body);
      } catch {
      }
    }
    const reqHeaders = {};
    if (init?.headers) {
      if (typeof init.headers.forEach === "function") {
        init.headers.forEach((v, k) => {
          reqHeaders[k] = v;
        });
      } else {
        Object.assign(reqHeaders, init.headers);
      }
    }
    const handler = findHandler(method, url);
    if (!handler) {
      const msg = `[mock.fetch] Unhandled ${method} ${url}`;
      return Promise.resolve({
        ok: false,
        status: 500,
        statusText: msg,
        headers: { get: () => null, has: () => false },
        json: () => Promise.resolve({ error: msg }),
        text: () => Promise.resolve(msg)
      });
    }
    const req = { method, url, headers: reqHeaders, body };
    const res = handler.handler(req);
    const status = res.status ?? 200;
    const responseBody = res.body;
    const responseHeaders = res.headers || {};
    if (responseBody !== void 0 && !responseHeaders["content-type"]) {
      responseHeaders["content-type"] = "application/json";
    }
    return Promise.resolve({
      ok: status >= 200 && status < 300,
      status,
      statusText: res.statusText || (status === 200 ? "OK" : "Error"),
      headers: {
        get: (k) => responseHeaders[k.toLowerCase()] || null,
        has: (k) => k.toLowerCase() in responseHeaders
      },
      json: () => Promise.resolve(responseBody),
      text: () => Promise.resolve(typeof responseBody === "string" ? responseBody : JSON.stringify(responseBody)),
      clone: function() {
        return this;
      }
    });
  }
  function createHandler(method, url, response, once = false) {
    const handler = typeof response === "function" ? response : () => response;
    return { method, url, handler, once };
  }
  function mockFetch(...newHandlers) {
    handlers.push(...newHandlers);
    globalThis.fetch = fakeFetch;
  }
  var http = {
    get(url, handler) {
      return createHandler("GET", url, handler);
    },
    post(url, handler) {
      return createHandler("POST", url, handler);
    },
    put(url, handler) {
      return createHandler("PUT", url, handler);
    },
    delete(url, handler) {
      return createHandler("DELETE", url, handler);
    },
    patch(url, handler) {
      return createHandler("PATCH", url, handler);
    }
  };
  var HttpResponse = {
    json(data, init) {
      return { body: data, status: init?.status ?? 200, headers: init?.headers };
    },
    text(data, init) {
      return { body: data, status: init?.status ?? 200 };
    },
    error() {
      return { status: 500, body: { error: "Internal Server Error" } };
    }
  };
  function mockFetchUse(...newHandlers) {
    overrideHandlers.push(...newHandlers);
    if (globalThis.fetch !== fakeFetch) {
      globalThis.fetch = fakeFetch;
    }
  }
  function mockFetchReset() {
    overrideHandlers.length = 0;
  }
  function mockFetchClear() {
    handlers.length = 0;
    overrideHandlers.length = 0;
  }

  // src/timers.ts
  var _savedDateNow = Date.now;
  var fakeNow = 0;
  var nextId = 1;
  var pending = [];
  var isFake = false;
  var _setTimeout = globalThis.setTimeout;
  var _clearTimeout = globalThis.clearTimeout;
  var _setInterval = globalThis.setInterval;
  var _clearInterval = globalThis.clearInterval;
  var _Date = globalThis.Date;
  function fakeSetTimeout(fn, delay = 0) {
    const id = nextId++;
    pending.push({ id, fn, delay, fireAt: fakeNow + delay, type: "timeout", interval: 0 });
    return id;
  }
  function fakeClearTimeout(id) {
    pending = pending.filter((t) => t.id !== id);
  }
  function fakeSetInterval(fn, delay) {
    const id = nextId++;
    pending.push({ id, fn, delay, fireAt: fakeNow + delay, type: "interval", interval: delay });
    return id;
  }
  function fakeClearInterval(id) {
    pending = pending.filter((t) => t.id !== id);
  }
  function useFakeTimers(initialTime) {
    fakeNow = initialTime ?? 0;
    nextId = 1;
    pending = [];
    isFake = true;
    globalThis.setTimeout = fakeSetTimeout;
    globalThis.clearTimeout = fakeClearTimeout;
    globalThis.setInterval = fakeSetInterval;
    globalThis.clearInterval = fakeClearInterval;
    globalThis.Date = function(...args) {
      if (args.length === 0) return new _Date(fakeNow);
      return new _Date(...args);
    };
    globalThis.Date.now = () => fakeNow;
    globalThis.Date.parse = _Date.parse;
    globalThis.Date.UTC = _Date.UTC;
    globalThis.Date.prototype = _Date.prototype;
  }
  function useRealTimers() {
    isFake = false;
    pending = [];
    globalThis.setTimeout = _setTimeout;
    globalThis.clearTimeout = _clearTimeout;
    globalThis.setInterval = _setInterval;
    globalThis.clearInterval = _clearInterval;
    globalThis.Date = _Date;
  }
  function advanceTimersByTime(ms) {
    if (!isFake) throw new Error("advanceTimersByTime called without useFakeTimers()");
    const target = fakeNow + ms;
    while (fakeNow < target) {
      const ready = pending.filter((t) => t.fireAt <= target).sort((a, b) => a.fireAt - b.fireAt);
      if (ready.length === 0) {
        fakeNow = target;
        break;
      }
      const timer = ready[0];
      fakeNow = timer.fireAt;
      if (timer.type === "timeout") {
        pending = pending.filter((t) => t.id !== timer.id);
        timer.fn();
      } else {
        timer.fireAt += timer.interval;
        timer.fn();
      }
    }
    fakeNow = target;
  }
  function runAllTimers() {
    if (!isFake) throw new Error("runAllTimers called without useFakeTimers()");
    let safety = 1e3;
    while (pending.length > 0 && safety-- > 0) {
      const next = pending.reduce((min, t) => t.fireAt < min.fireAt ? t : min);
      fakeNow = next.fireAt;
      if (next.type === "timeout") {
        pending = pending.filter((t) => t.id !== next.id);
        next.fn();
      } else {
        next.fireAt += next.interval;
        next.fn();
      }
    }
  }
  function getTimerCount() {
    return pending.length;
  }
  function advanceTimersToNextTimer() {
    if (!isFake || pending.length === 0) return;
    const next = pending.reduce((min, t) => t.fireAt < min.fireAt ? t : min);
    advanceTimersByTime(next.fireAt - fakeNow);
  }

  // src/harness.ts
  (function() {
    const p = globalThis.print || (() => {
    });
    function fmt(...args) {
      return args.map((a) => {
        try {
          return typeof a === "string" ? a : JSON.stringify(a, null, 2);
        } catch {
          return String(a);
        }
      }).join(" ");
    }
    globalThis.console = {
      log: (...args) => p(fmt(...args)),
      info: (...args) => p(fmt(...args)),
      debug: (...args) => p(fmt(...args)),
      warn: (...args) => p("\x1B[33m\u26A0 " + fmt(...args) + "\x1B[0m"),
      error: (...args) => {
        const msg = fmt(...args);
        if (msg.includes("Expected host context to exist")) return;
        if (msg.includes("An unhandled error occurred processing a request for the endpoint")) return;
        p("\x1B[31m\u2717 " + msg + "\x1B[0m");
      }
    };
  })();
  var tests = [];
  var beforeEachHooks = [];
  var afterEachHooks = [];
  var beforeAllHooks = [];
  var afterAllHooks = [];
  var currentGroup;
  function test(name, fn, options) {
    tests.push({
      name: currentGroup ? `${currentGroup} > ${name}` : name,
      fn,
      options: options ?? {},
      group: currentGroup,
      file: globalThis.__currentTestFile
    });
  }
  test.only = function(name, fn) {
    test(name, fn, { only: true });
  };
  test.skip = function(name, fn) {
    test(name, fn, { skip: true });
  };
  function group(name, fn) {
    const prev = currentGroup;
    currentGroup = prev ? `${prev} > ${name}` : name;
    fn();
    currentGroup = prev;
  }
  function beforeEach(fn) {
    beforeEachHooks.push({ fn, group: currentGroup });
  }
  function afterEach(fn) {
    afterEachHooks.push({ fn, group: currentGroup });
  }
  function beforeAll(fn) {
    beforeAllHooks.push({ fn, group: currentGroup });
  }
  function afterAll(fn) {
    afterAllHooks.push({ fn, group: currentGroup });
  }
  function hookApplies(hook, testGroup) {
    if (hook.group === void 0) return true;
    if (testGroup === void 0) return false;
    return testGroup === hook.group || testGroup.startsWith(hook.group + " > ");
  }
  var drain2 = globalThis.__HT_drain || (() => {
  });
  var __testMaxDrains = 0;
  var __testDrainCount = 0;
  var __testTimeoutMs = 0;
  var DEFAULT_TIMEOUT_MS = 0;
  var DRAINS_PER_MS = 1;
  function checkDeadline() {
    if (__testMaxDrains > 0 && ++__testDrainCount >= __testMaxDrains) {
      throw new Error("Test timed out after " + __testTimeoutMs + "ms");
    }
  }
  function flushAsync(promise) {
    if (!promise || typeof promise.then !== "function") {
      return promise;
    }
    let result;
    let error;
    let settled = false;
    promise.then(
      (v) => {
        result = v;
        settled = true;
      },
      (e) => {
        error = e;
        settled = true;
      }
    );
    for (let i = 0; i < 100 && !settled; i++) {
      drain2();
      checkDeadline();
    }
    if (!settled) {
      throw new Error("flushAsync: promise did not resolve after 100 drain cycles");
    }
    if (error) throw error;
    return result;
  }
  function resolveSync(value) {
    if (value && typeof value.then === "function") {
      flushAsync(value);
    }
  }
  var _print = globalThis.__HT_print || (() => {
  });
  var _filesCompleted = 0;
  var _testsCompleted = 0;
  var _totalFiles = 0;
  function _printFileResult(file, passed, failed, duration) {
    const total = passed + failed;
    const time = duration > 0 ? ` \x1B[2m(${duration}ms)\x1B[0m` : "";
    _filesCompleted++;
    _testsCompleted += total;
    if (failed > 0) {
      if (globalThis.__HT_coverage) {
        _print(`\r\x1B[K`);
      }
      _print(` \x1B[31mFAIL\x1B[0m  ${file} \x1B[2m(${passed} passed, ${failed} failed)\x1B[0m${time}
`);
    } else if (globalThis.__HT_coverage) {
      _print(`\r\x1B[K \x1B[2mRunning...\x1B[0m ${_filesCompleted}/${_totalFiles} files (${_testsCompleted} tests)`);
    } else {
      _print(` \x1B[32mPASS\x1B[0m  ${file} \x1B[2m(${total} tests)\x1B[0m${time}
`);
    }
  }
  function formatTestError(e) {
    const message = e?.message ?? String(e);
    const stack = e?.stack;
    if (!stack) return message;
    const frames = [];
    for (const raw of stack.split("\n").slice(1)) {
      const m = raw.match(/at\s+(?:([^\s(]+)\s+\()?([^:)]+):(\d+)/);
      if (m) frames.push({ fn: m[1] || "", file: m[2], line: m[3] });
    }
    const skipFn = /* @__PURE__ */ new Set([
      "anonymous",
      "global",
      "__init",
      "apply",
      "map",
      "react-stack-bottom-frame",
      "proxy trap"
    ]);
    const skipPrefix = [
      "render",
      "run",
      "perform",
      "work",
      "flush",
      "begin",
      "update",
      "reconcile",
      "create",
      "complete",
      "commit",
      "process"
    ];
    const appFrames = frames.filter((f) => {
      if (skipFn.has(f.fn)) return false;
      if (f.file.includes("harness") || f.file.includes("runner")) return false;
      if (f.fn === "" && !f.file.includes("/src/") && !f.file.includes("packages/")) return false;
      for (const p of skipPrefix) {
        if (f.fn.startsWith(p)) return false;
      }
      return true;
    });
    let cleanStack = message;
    if (appFrames.length > 0) {
      cleanStack += "\n";
      for (const f of appFrames.slice(0, 8)) {
        const loc = f.fn.includes("/") ? f.fn : f.fn ? f.fn + " (" + f.file + ":" + f.line + ")" : f.file + ":" + f.line;
        cleanStack += "\n    at " + loc;
      }
    }
    const importMap = globalThis.__HT_shallow_imports;
    let hint = "";
    for (const f of appFrames) {
      const fnName = f.fn;
      if (fnName.includes("/") && (fnName.includes(".ts") || fnName.includes(".js"))) {
        const srcPath = fnName.replace(/^(\.\.\/)*/, "");
        const nmMatch = srcPath.match(/node_modules\/((?:@[^/]+\/)?[^/]+)/);
        if (nmMatch) {
          const pkg = nmMatch[1];
          hint = '\n\n  "' + pkg + '" crashed during initialization (native dependency).\n  Add to externals in hermes-test.config.json:\n\n    { "externals": ["' + pkg + '"] }\n\n  Or mock the module that imports it with ht.mock().\n';
        } else {
          const cleanPath = srcPath.replace(/\/index\.(tsx?|jsx?)$/, "");
          hint = '\n\n  Module "' + cleanPath + '" crashed during initialization.\n  A dependency uses an API not available in Hermes.\n  Mock it with ht.mock() or add the native dep to externals.\n';
        }
        break;
      }
      if (fnName && fnName.length > 2 && !fnName.includes("(") && importMap) {
        const modPath = importMap[fnName];
        if (modPath) {
          const siblings = [];
          for (const k in importMap) {
            if (importMap[k] === modPath && siblings.indexOf(k) === -1) siblings.push(k);
          }
          const mockBody = siblings.map((s) => "    " + s + ": () => {}").join(",\n");
          hint = '\n\n  "' + fnName + '" from "' + modPath + `" failed.
  Add this mock to your test file:

    ht.mock('` + modPath + "', () => ({\n" + mockBody + "\n    }));\n";
          break;
        }
      }
    }
    return cleanStack + hint;
  }
  function runTests() {
    const results = [];
    const hasOnly = tests.some((t) => t.options.only);
    const uniqueFiles = new Set(tests.map((t) => t.file));
    _totalFiles = uniqueFiles.size;
    _filesCompleted = 0;
    _testsCompleted = 0;
    let _currentFile;
    let _filePassed = 0;
    let _fileFailed = 0;
    let _fileStart = Date.now();
    let _fileFailures = [];
    function _flushFileResult() {
      if (_currentFile && _filePassed + _fileFailed > 0) {
        _printFileResult(_currentFile, _filePassed, _fileFailed, Date.now() - _fileStart);
        for (const f of _fileFailures) {
          _print(`       \x1B[31m\u2717 ${f.name}\x1B[0m
`);
          if (f.error) _print(`         \x1B[2m${f.error}\x1B[0m
`);
        }
      }
      _filePassed = 0;
      _fileFailed = 0;
      _fileFailures = [];
      _fileStart = Date.now();
    }
    const beforeAllRan = /* @__PURE__ */ new Set();
    for (const entry of tests) {
      if (entry.file !== _currentFile) {
        _flushFileResult();
        _currentFile = entry.file;
      }
      globalThis.__currentTestFile = entry.file;
      if (entry.options.skip || hasOnly && !entry.options.only) {
        results.push({ name: entry.name, status: "skip", duration: 0, file: entry.file });
        continue;
      }
      for (const hook of beforeAllHooks) {
        if (!beforeAllRan.has(hook) && hookApplies(hook, entry.group)) {
          beforeAllRan.add(hook);
          resolveSync(hook.fn());
        }
      }
      {
        const filePath = globalThis.__currentTestFilePath || entry.file || "unknown";
        const clean = filePath.startsWith("./") ? filePath.substring(2) : filePath;
        const lastSlash = clean.lastIndexOf("/");
        const dir = lastSlash >= 0 ? clean.substring(0, lastSlash) : ".";
        const basename = lastSlash >= 0 ? clean.substring(lastSlash + 1) : clean;
        const snapFile = dir + "/__snapshots__/" + basename + ".snap";
        _setSnapshotContext(snapFile, entry.name, !!globalThis.__HT_updateSnapshots);
      }
      const timeoutMs = entry.options.timeout ?? DEFAULT_TIMEOUT_MS;
      __testTimeoutMs = timeoutMs;
      __testDrainCount = 0;
      __testMaxDrains = timeoutMs > 0 ? timeoutMs * DRAINS_PER_MS : 0;
      const start = Date.now();
      try {
        for (const hook of beforeEachHooks) {
          if (hookApplies(hook, entry.group)) {
            resolveSync(hook.fn());
            checkDeadline();
          }
        }
        const ctx = { expect, spy, useMock, renderHook, act, waitFor };
        resolveSync(entry.fn(ctx));
        checkDeadline();
        for (const hook of afterEachHooks) {
          if (hookApplies(hook, entry.group)) {
            resolveSync(hook.fn());
          }
        }
        resetMocks();
        __testMaxDrains = 0;
        _filePassed++;
        results.push({
          name: entry.name,
          status: "pass",
          duration: Date.now() - start,
          file: entry.file
        });
      } catch (e) {
        __testMaxDrains = 0;
        for (const hook of afterEachHooks) {
          if (hookApplies(hook, entry.group)) {
            try {
              resolveSync(hook.fn());
            } catch {
            }
          }
        }
        resetMocks();
        _fileFailed++;
        const errMsg = e?.stack ?? e?.message ?? String(e);
        _fileFailures.push({ name: entry.name, error: errMsg });
        results.push({
          name: entry.name,
          status: "fail",
          error: errMsg,
          duration: Date.now() - start,
          file: entry.file
        });
      }
    }
    _flushFileResult();
    if (globalThis.__HT_coverage) {
      _print(`\r\x1B[K`);
    }
    for (const hook of afterAllHooks) {
      try {
        resolveSync(hook.fn());
      } catch {
      }
    }
    return results;
  }
  function registerCrash(file, error) {
    const formatted = formatTestError({ message: error.split("\n")[0], stack: error });
    tests.push({
      name: `[CRASH] ${file}`,
      fn: () => {
        throw new Error(formatted);
      },
      options: {},
      file
    });
  }
  function resetRegistry() {
    tests.length = 0;
    beforeEachHooks.length = 0;
    afterEachHooks.length = 0;
    beforeAllHooks.length = 0;
    afterAllHooks.length = 0;
    currentGroup = void 0;
    clearAllMocks();
    if (globalThis.__HT_file_mocks) globalThis.__HT_file_mocks = {};
    resetMockModulePatches();
  }
  var mock = mockModule;
  mock.fetch = mockFetch;
  mock.fetch.overwrite = mockFetchUse;
  mock.fetch.reset = mockFetchReset;
  mock.fetch.clear = mockFetchClear;
  var shallow = (_componentPath) => {
  };
  globalThis.ht = { mock, shallow };
  globalThis.__HT = {
    test,
    expect,
    spy,
    spyOn,
    clearAllMocks,
    group,
    beforeEach,
    afterEach,
    beforeAll,
    afterAll,
    runTests,
    renderHook,
    act,
    waitFor,
    useMock,
    http,
    HttpResponse,
    render,
    fireEvent,
    flushAsync,
    registerCrash,
    resetRegistry,
    resetMockModulePatches,
    // Timer control
    useFakeTimers,
    useRealTimers,
    advanceTimersByTime,
    runAllTimers,
    getTimerCount,
    advanceTimersToNextTimer
  };
  return __toCommonJS(harness_exports);
})();
