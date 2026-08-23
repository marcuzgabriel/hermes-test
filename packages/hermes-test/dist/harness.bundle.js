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

// Intl NumberFormat fallback for Linux Hermes builds where locale is ignored.
// Detect broken behavior first so macOS/Android native Intl remains untouched.
(function () {
  function pickLocale(locales) {
    if (Array.isArray(locales) && locales.length > 0) return String(locales[0] || 'en-US');
    if (typeof locales === 'string' && locales) return locales;
    return 'en-US';
  }

  function normalizeDigits(style, options) {
    var minDefault = 0;
    var maxDefault = 3;
    if (style === 'currency') {
      minDefault = 2;
      maxDefault = 2;
    } else if (style === 'percent') {
      minDefault = 0;
      maxDefault = 0;
    }
    var minDigits = typeof options.minimumFractionDigits === 'number' ? options.minimumFractionDigits : minDefault;
    var maxDigits = typeof options.maximumFractionDigits === 'number' ? options.maximumFractionDigits : maxDefault;
    if (maxDigits < minDigits) maxDigits = minDigits;
    return { min: minDigits, max: maxDigits };
  }

  function separatorsForLocale(locale) {
    var lc = String(locale || 'en-US').toLowerCase();
    if (lc.indexOf('da') === 0) return { group: '.', decimal: ',' };
    if (lc.indexOf('de') === 0) return { group: '.', decimal: ',' };
    if (lc.indexOf('fr') === 0) return { group: ' ', decimal: ',' };
    return { group: ',', decimal: '.' };
  }

  function addGrouping(intPart, group) {
    var out = '';
    var count = 0;
    for (var i = intPart.length - 1; i >= 0; i--) {
      out = intPart[i] + out;
      count++;
      if (i > 0 && count % 3 === 0) out = group + out;
    }
    return out;
  }

  function trimFraction(fracPart, minDigits) {
    while (fracPart.length > minDigits && fracPart[fracPart.length - 1] === '0') {
      fracPart = fracPart.slice(0, -1);
    }
    return fracPart;
  }

  function formatNumberValue(value, locales, options) {
    var num = Number(value);
    if (!isFinite(num)) return String(num);
    options = options || {};

    var locale = pickLocale(locales);
    var style = options.style || 'decimal';
    var useGrouping = options.useGrouping !== false;
    var digits = normalizeDigits(style, options);
    var minDigits = digits.min;
    var maxDigits = digits.max;

    if (style === 'percent') num = num * 100;
    var sign = num < 0 ? '-' : '';
    var abs = Math.abs(num);

    var fixed = abs.toFixed(maxDigits);
    var parts = fixed.split('.');
    var intPart = parts[0];
    var fracPart = parts[1] || '';

    if (maxDigits > minDigits) fracPart = trimFraction(fracPart, minDigits);

    var sep = separatorsForLocale(locale);
    if (useGrouping) intPart = addGrouping(intPart, sep.group);

    var formatted = sign + intPart + (fracPart ? sep.decimal + fracPart : '');

    if (style === 'percent') return formatted + '%';
    if (style === 'currency' && options.currency) {
      if (String(locale).toLowerCase().indexOf('en') === 0) return options.currency + ' ' + formatted;
      return formatted + ' ' + options.currency;
    }
    return formatted;
  }

  function isBrokenIntlNumberFormatting() {
    try {
      var da = (1234.56).toLocaleString('da-DK');
      var en = (1234.56).toLocaleString('en-US');
      if (typeof da !== 'string' || typeof en !== 'string') return true;
      if (da === en) return true;
      if (da.indexOf('560000') !== -1 || en.indexOf('560000') !== -1) return true;
      return false;
    } catch (_e) {
      return true;
    }
  }

  if (!isBrokenIntlNumberFormatting()) return;

  if (typeof globalThis.Intl === 'undefined') globalThis.Intl = {};

  function NumberFormat(locales, options) {
    if (!(this instanceof NumberFormat)) return new NumberFormat(locales, options);
    this._locales = locales;
    this._options = options || {};
    this._boundFormat = null;
  }
  NumberFormat.supportedLocalesOf = function (locales) {
    if (Array.isArray(locales)) return locales.map(String);
    if (typeof locales === 'string') return [locales];
    return [];
  };
  NumberFormat.prototype.format = function (value) {
    return formatNumberValue(value, this._locales, this._options);
  };
  function getBoundFormat(instance) {
    if (!instance._boundFormat) {
      var self = instance;
      instance._boundFormat = function (value) {
        return formatNumberValue(value, self._locales, self._options);
      };
    }
    return instance._boundFormat;
  }
  Object.defineProperty(NumberFormat.prototype, 'format', {
    configurable: true,
    enumerable: false,
    get: function () {
      return getBoundFormat(this);
    },
  });
  NumberFormat.prototype.resolvedOptions = function () {
    var style = this._options.style || 'decimal';
    var digits = normalizeDigits(style, this._options);
    return {
      locale: pickLocale(this._locales),
      style: style,
      useGrouping: this._options.useGrouping !== false,
      minimumFractionDigits: digits.min,
      maximumFractionDigits: digits.max,
    };
  };

  globalThis.Intl.NumberFormat = NumberFormat;
  Number.prototype.toLocaleString = function (locales, options) {
    return formatNumberValue(Number(this), locales, options);
  };
})();

// Intl String locale casing fallback for Linux Hermes ICU stub behavior.
// Some Linux builds return placeholder values (e.g. "lowered"/"UPPERED")
// instead of transformed text. Keep native behavior when it works.
(function () {
  function isBrokenIntlStringCasing() {
    try {
      var lower = 'AbC'.toLocaleLowerCase();
      var upper = 'aBc'.toLocaleUpperCase();
      if (typeof lower !== 'string' || typeof upper !== 'string') return true;
      return lower !== 'abc' || upper !== 'ABC';
    } catch (_e) {
      return true;
    }
  }

  if (!isBrokenIntlStringCasing()) return;

  Object.defineProperty(String.prototype, 'toLocaleLowerCase', {
    configurable: true,
    writable: true,
    value: function () {
      return String(this).toLowerCase();
    },
  });

  Object.defineProperty(String.prototype, 'toLocaleUpperCase', {
    configurable: true,
    writable: true,
    value: function () {
      return String(this).toUpperCase();
    },
  });
})();

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

// Web API polyfills — the ones React Native's InitializeCore installs (Libraries/Core/setUpXHR.js).
// Headers / Request / Response come from `whatwg-fetch` and AbortController / AbortSignal from
// `abort-controller` — the exact packages RN requires — bundled into the harness by bundle.mjs
// (see src/rn-globals.ts). The hand-written RN-shape mirrors below are for globals RN implements
// in Flow files we cannot bundle (URL / URLSearchParams / FormData).
(function () {
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

  // URL — always install: Hermes has a built-in URL that doesn't parse searchParams correctly.
  // Shape follows React Native's own polyfill (Libraries/Blob/URL.js): http(s) host/hostname/
  // origin/pathname, userinfo (`user:pass@host`) stripped from host, `protocol` for any scheme.
  // Non-http schemes get host '' exactly like RN does on device — do not "improve" that here.
  {
    function URL(url, base) {
      if (base && url.indexOf('://') === -1) {
        url = base.replace(/\/$/, '') + '/' + url.replace(/^\//, '');
      }
      this.href = url;
      var protoMatch = url.match(/^([a-zA-Z][a-zA-Z\d+\-.]*):/);
      var match = url.match(/^(https?:)\/\/(?:([^@/?#]*)@)?([^/:?#]+)(:\d+)?(\/[^?#]*)?(\?[^#]*)?(#.*)?$/);
      if (match) {
        this.protocol = match[1];
        var userinfo = match[2] || '';
        var sep = userinfo.indexOf(':');
        this.username = sep === -1 ? userinfo : userinfo.slice(0, sep);
        this.password = sep === -1 ? '' : userinfo.slice(sep + 1);
        this.hostname = match[3];
        this.port = match[4] ? match[4].slice(1) : '';
        this.pathname = match[5] || '/';
        this.search = match[6] || '';
        this.hash = match[7] || '';
        this.host = this.hostname + (this.port ? ':' + this.port : '');
        this.origin = this.protocol + '//' + this.host;
      } else {
        this.protocol = protoMatch ? protoMatch[1] + ':' : '';
        this.username = '';
        this.password = '';
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
    URL.prototype.toJSON = function () {
      return this.href;
    };
    globalThis.URL = URL;
  }

  // FormData — React Native installs its own (Libraries/Network/FormData.js) in InitializeCore;
  // Hermes has none. Same API as RN: append / getAll / getParts. Installed only if absent so a
  // project-level polyfill wins.
  if (typeof globalThis.FormData === 'undefined') {
    function FormData() {
      this._parts = [];
    }
    FormData.prototype.append = function (key, value) {
      this._parts.push([key, value]);
    };
    FormData.prototype.getAll = function (key) {
      return this._parts
        .filter(function (p) { return p[0] === key; })
        .map(function (p) { return p[1]; });
    };
    FormData.prototype.getParts = function () {
      return this._parts.map(function (p) {
        var name = p[0];
        var value = p[1];
        var headers = { 'content-disposition': 'form-data; name="' + name + '"' };
        if (typeof value === 'object' && !Array.isArray(value) && value) {
          if (typeof value.name === 'string') {
            headers['content-disposition'] += '; filename="' + value.name.replace(/"/g, '%22') + '"';
          }
          if (typeof value.type === 'string') {
            headers['content-type'] = value.type;
          }
          return Object.assign({}, value, { headers: headers, fieldName: name });
        }
        return { string: String(value), headers: headers, fieldName: name };
      });
    };
    globalThis.FormData = FormData;
  }

  // Stub fetch (mockFetch will override with handler-based implementation)
  if (typeof globalThis.fetch === 'undefined') {
    globalThis.fetch = function () {
      return Promise.reject(
        new Error('fetch not configured — use mockFetch() to register handlers'),
      );
    };
  }

})();

"use strict";
var __metroTestHarness = (() => {
  var __create = Object.create;
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __getProtoOf = Object.getPrototypeOf;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __commonJS = (cb, mod) => function __require() {
    return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
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
  var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(
    // If the importer is in node compatibility mode or this is not an ESM
    // file that has been converted to a CommonJS file using a Babel-
    // compatible transform (i.e. "__esModule" has not been set), then set
    // "default" to the CommonJS "module.exports" for node compatibility.
    isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target,
    mod
  ));
  var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

  // ../../node_modules/.bun/event-target-shim@5.0.1/node_modules/event-target-shim/dist/event-target-shim.js
  var require_event_target_shim = __commonJS({
    "../../node_modules/.bun/event-target-shim@5.0.1/node_modules/event-target-shim/dist/event-target-shim.js"(exports, module) {
      "use strict";
      Object.defineProperty(exports, "__esModule", { value: true });
      var privateData = /* @__PURE__ */ new WeakMap();
      var wrappers = /* @__PURE__ */ new WeakMap();
      function pd(event) {
        const retv = privateData.get(event);
        console.assert(
          retv != null,
          "'this' is expected an Event object, but got",
          event
        );
        return retv;
      }
      function setCancelFlag(data) {
        if (data.passiveListener != null) {
          if (typeof console !== "undefined" && typeof console.error === "function") {
            console.error(
              "Unable to preventDefault inside passive event listener invocation.",
              data.passiveListener
            );
          }
          return;
        }
        if (!data.event.cancelable) {
          return;
        }
        data.canceled = true;
        if (typeof data.event.preventDefault === "function") {
          data.event.preventDefault();
        }
      }
      function Event(eventTarget, event) {
        privateData.set(this, {
          eventTarget,
          event,
          eventPhase: 2,
          currentTarget: eventTarget,
          canceled: false,
          stopped: false,
          immediateStopped: false,
          passiveListener: null,
          timeStamp: event.timeStamp || Date.now()
        });
        Object.defineProperty(this, "isTrusted", { value: false, enumerable: true });
        const keys = Object.keys(event);
        for (let i = 0; i < keys.length; ++i) {
          const key = keys[i];
          if (!(key in this)) {
            Object.defineProperty(this, key, defineRedirectDescriptor(key));
          }
        }
      }
      Event.prototype = {
        /**
         * The type of this event.
         * @type {string}
         */
        get type() {
          return pd(this).event.type;
        },
        /**
         * The target of this event.
         * @type {EventTarget}
         */
        get target() {
          return pd(this).eventTarget;
        },
        /**
         * The target of this event.
         * @type {EventTarget}
         */
        get currentTarget() {
          return pd(this).currentTarget;
        },
        /**
         * @returns {EventTarget[]} The composed path of this event.
         */
        composedPath() {
          const currentTarget = pd(this).currentTarget;
          if (currentTarget == null) {
            return [];
          }
          return [currentTarget];
        },
        /**
         * Constant of NONE.
         * @type {number}
         */
        get NONE() {
          return 0;
        },
        /**
         * Constant of CAPTURING_PHASE.
         * @type {number}
         */
        get CAPTURING_PHASE() {
          return 1;
        },
        /**
         * Constant of AT_TARGET.
         * @type {number}
         */
        get AT_TARGET() {
          return 2;
        },
        /**
         * Constant of BUBBLING_PHASE.
         * @type {number}
         */
        get BUBBLING_PHASE() {
          return 3;
        },
        /**
         * The target of this event.
         * @type {number}
         */
        get eventPhase() {
          return pd(this).eventPhase;
        },
        /**
         * Stop event bubbling.
         * @returns {void}
         */
        stopPropagation() {
          const data = pd(this);
          data.stopped = true;
          if (typeof data.event.stopPropagation === "function") {
            data.event.stopPropagation();
          }
        },
        /**
         * Stop event bubbling.
         * @returns {void}
         */
        stopImmediatePropagation() {
          const data = pd(this);
          data.stopped = true;
          data.immediateStopped = true;
          if (typeof data.event.stopImmediatePropagation === "function") {
            data.event.stopImmediatePropagation();
          }
        },
        /**
         * The flag to be bubbling.
         * @type {boolean}
         */
        get bubbles() {
          return Boolean(pd(this).event.bubbles);
        },
        /**
         * The flag to be cancelable.
         * @type {boolean}
         */
        get cancelable() {
          return Boolean(pd(this).event.cancelable);
        },
        /**
         * Cancel this event.
         * @returns {void}
         */
        preventDefault() {
          setCancelFlag(pd(this));
        },
        /**
         * The flag to indicate cancellation state.
         * @type {boolean}
         */
        get defaultPrevented() {
          return pd(this).canceled;
        },
        /**
         * The flag to be composed.
         * @type {boolean}
         */
        get composed() {
          return Boolean(pd(this).event.composed);
        },
        /**
         * The unix time of this event.
         * @type {number}
         */
        get timeStamp() {
          return pd(this).timeStamp;
        },
        /**
         * The target of this event.
         * @type {EventTarget}
         * @deprecated
         */
        get srcElement() {
          return pd(this).eventTarget;
        },
        /**
         * The flag to stop event bubbling.
         * @type {boolean}
         * @deprecated
         */
        get cancelBubble() {
          return pd(this).stopped;
        },
        set cancelBubble(value) {
          if (!value) {
            return;
          }
          const data = pd(this);
          data.stopped = true;
          if (typeof data.event.cancelBubble === "boolean") {
            data.event.cancelBubble = true;
          }
        },
        /**
         * The flag to indicate cancellation state.
         * @type {boolean}
         * @deprecated
         */
        get returnValue() {
          return !pd(this).canceled;
        },
        set returnValue(value) {
          if (!value) {
            setCancelFlag(pd(this));
          }
        },
        /**
         * Initialize this event object. But do nothing under event dispatching.
         * @param {string} type The event type.
         * @param {boolean} [bubbles=false] The flag to be possible to bubble up.
         * @param {boolean} [cancelable=false] The flag to be possible to cancel.
         * @deprecated
         */
        initEvent() {
        }
      };
      Object.defineProperty(Event.prototype, "constructor", {
        value: Event,
        configurable: true,
        writable: true
      });
      if (typeof window !== "undefined" && typeof window.Event !== "undefined") {
        Object.setPrototypeOf(Event.prototype, window.Event.prototype);
        wrappers.set(window.Event.prototype, Event);
      }
      function defineRedirectDescriptor(key) {
        return {
          get() {
            return pd(this).event[key];
          },
          set(value) {
            pd(this).event[key] = value;
          },
          configurable: true,
          enumerable: true
        };
      }
      function defineCallDescriptor(key) {
        return {
          value() {
            const event = pd(this).event;
            return event[key].apply(event, arguments);
          },
          configurable: true,
          enumerable: true
        };
      }
      function defineWrapper(BaseEvent, proto) {
        const keys = Object.keys(proto);
        if (keys.length === 0) {
          return BaseEvent;
        }
        function CustomEvent(eventTarget, event) {
          BaseEvent.call(this, eventTarget, event);
        }
        CustomEvent.prototype = Object.create(BaseEvent.prototype, {
          constructor: { value: CustomEvent, configurable: true, writable: true }
        });
        for (let i = 0; i < keys.length; ++i) {
          const key = keys[i];
          if (!(key in BaseEvent.prototype)) {
            const descriptor = Object.getOwnPropertyDescriptor(proto, key);
            const isFunc = typeof descriptor.value === "function";
            Object.defineProperty(
              CustomEvent.prototype,
              key,
              isFunc ? defineCallDescriptor(key) : defineRedirectDescriptor(key)
            );
          }
        }
        return CustomEvent;
      }
      function getWrapper(proto) {
        if (proto == null || proto === Object.prototype) {
          return Event;
        }
        let wrapper = wrappers.get(proto);
        if (wrapper == null) {
          wrapper = defineWrapper(getWrapper(Object.getPrototypeOf(proto)), proto);
          wrappers.set(proto, wrapper);
        }
        return wrapper;
      }
      function wrapEvent(eventTarget, event) {
        const Wrapper = getWrapper(Object.getPrototypeOf(event));
        return new Wrapper(eventTarget, event);
      }
      function isStopped(event) {
        return pd(event).immediateStopped;
      }
      function setEventPhase(event, eventPhase) {
        pd(event).eventPhase = eventPhase;
      }
      function setCurrentTarget(event, currentTarget) {
        pd(event).currentTarget = currentTarget;
      }
      function setPassiveListener(event, passiveListener) {
        pd(event).passiveListener = passiveListener;
      }
      var listenersMap = /* @__PURE__ */ new WeakMap();
      var CAPTURE = 1;
      var BUBBLE = 2;
      var ATTRIBUTE = 3;
      function isObject(x) {
        return x !== null && typeof x === "object";
      }
      function getListeners(eventTarget) {
        const listeners = listenersMap.get(eventTarget);
        if (listeners == null) {
          throw new TypeError(
            "'this' is expected an EventTarget object, but got another value."
          );
        }
        return listeners;
      }
      function defineEventAttributeDescriptor(eventName) {
        return {
          get() {
            const listeners = getListeners(this);
            let node = listeners.get(eventName);
            while (node != null) {
              if (node.listenerType === ATTRIBUTE) {
                return node.listener;
              }
              node = node.next;
            }
            return null;
          },
          set(listener) {
            if (typeof listener !== "function" && !isObject(listener)) {
              listener = null;
            }
            const listeners = getListeners(this);
            let prev = null;
            let node = listeners.get(eventName);
            while (node != null) {
              if (node.listenerType === ATTRIBUTE) {
                if (prev !== null) {
                  prev.next = node.next;
                } else if (node.next !== null) {
                  listeners.set(eventName, node.next);
                } else {
                  listeners.delete(eventName);
                }
              } else {
                prev = node;
              }
              node = node.next;
            }
            if (listener !== null) {
              const newNode = {
                listener,
                listenerType: ATTRIBUTE,
                passive: false,
                once: false,
                next: null
              };
              if (prev === null) {
                listeners.set(eventName, newNode);
              } else {
                prev.next = newNode;
              }
            }
          },
          configurable: true,
          enumerable: true
        };
      }
      function defineEventAttribute(eventTargetPrototype, eventName) {
        Object.defineProperty(
          eventTargetPrototype,
          `on${eventName}`,
          defineEventAttributeDescriptor(eventName)
        );
      }
      function defineCustomEventTarget(eventNames) {
        function CustomEventTarget() {
          EventTarget.call(this);
        }
        CustomEventTarget.prototype = Object.create(EventTarget.prototype, {
          constructor: {
            value: CustomEventTarget,
            configurable: true,
            writable: true
          }
        });
        for (let i = 0; i < eventNames.length; ++i) {
          defineEventAttribute(CustomEventTarget.prototype, eventNames[i]);
        }
        return CustomEventTarget;
      }
      function EventTarget() {
        if (this instanceof EventTarget) {
          listenersMap.set(this, /* @__PURE__ */ new Map());
          return;
        }
        if (arguments.length === 1 && Array.isArray(arguments[0])) {
          return defineCustomEventTarget(arguments[0]);
        }
        if (arguments.length > 0) {
          const types = new Array(arguments.length);
          for (let i = 0; i < arguments.length; ++i) {
            types[i] = arguments[i];
          }
          return defineCustomEventTarget(types);
        }
        throw new TypeError("Cannot call a class as a function");
      }
      EventTarget.prototype = {
        /**
         * Add a given listener to this event target.
         * @param {string} eventName The event name to add.
         * @param {Function} listener The listener to add.
         * @param {boolean|{capture?:boolean,passive?:boolean,once?:boolean}} [options] The options for this listener.
         * @returns {void}
         */
        addEventListener(eventName, listener, options) {
          if (listener == null) {
            return;
          }
          if (typeof listener !== "function" && !isObject(listener)) {
            throw new TypeError("'listener' should be a function or an object.");
          }
          const listeners = getListeners(this);
          const optionsIsObj = isObject(options);
          const capture = optionsIsObj ? Boolean(options.capture) : Boolean(options);
          const listenerType = capture ? CAPTURE : BUBBLE;
          const newNode = {
            listener,
            listenerType,
            passive: optionsIsObj && Boolean(options.passive),
            once: optionsIsObj && Boolean(options.once),
            next: null
          };
          let node = listeners.get(eventName);
          if (node === void 0) {
            listeners.set(eventName, newNode);
            return;
          }
          let prev = null;
          while (node != null) {
            if (node.listener === listener && node.listenerType === listenerType) {
              return;
            }
            prev = node;
            node = node.next;
          }
          prev.next = newNode;
        },
        /**
         * Remove a given listener from this event target.
         * @param {string} eventName The event name to remove.
         * @param {Function} listener The listener to remove.
         * @param {boolean|{capture?:boolean,passive?:boolean,once?:boolean}} [options] The options for this listener.
         * @returns {void}
         */
        removeEventListener(eventName, listener, options) {
          if (listener == null) {
            return;
          }
          const listeners = getListeners(this);
          const capture = isObject(options) ? Boolean(options.capture) : Boolean(options);
          const listenerType = capture ? CAPTURE : BUBBLE;
          let prev = null;
          let node = listeners.get(eventName);
          while (node != null) {
            if (node.listener === listener && node.listenerType === listenerType) {
              if (prev !== null) {
                prev.next = node.next;
              } else if (node.next !== null) {
                listeners.set(eventName, node.next);
              } else {
                listeners.delete(eventName);
              }
              return;
            }
            prev = node;
            node = node.next;
          }
        },
        /**
         * Dispatch a given event.
         * @param {Event|{type:string}} event The event to dispatch.
         * @returns {boolean} `false` if canceled.
         */
        dispatchEvent(event) {
          if (event == null || typeof event.type !== "string") {
            throw new TypeError('"event.type" should be a string.');
          }
          const listeners = getListeners(this);
          const eventName = event.type;
          let node = listeners.get(eventName);
          if (node == null) {
            return true;
          }
          const wrappedEvent = wrapEvent(this, event);
          let prev = null;
          while (node != null) {
            if (node.once) {
              if (prev !== null) {
                prev.next = node.next;
              } else if (node.next !== null) {
                listeners.set(eventName, node.next);
              } else {
                listeners.delete(eventName);
              }
            } else {
              prev = node;
            }
            setPassiveListener(
              wrappedEvent,
              node.passive ? node.listener : null
            );
            if (typeof node.listener === "function") {
              try {
                node.listener.call(this, wrappedEvent);
              } catch (err) {
                if (typeof console !== "undefined" && typeof console.error === "function") {
                  console.error(err);
                }
              }
            } else if (node.listenerType !== ATTRIBUTE && typeof node.listener.handleEvent === "function") {
              node.listener.handleEvent(wrappedEvent);
            }
            if (isStopped(wrappedEvent)) {
              break;
            }
            node = node.next;
          }
          setPassiveListener(wrappedEvent, null);
          setEventPhase(wrappedEvent, 0);
          setCurrentTarget(wrappedEvent, null);
          return !wrappedEvent.defaultPrevented;
        }
      };
      Object.defineProperty(EventTarget.prototype, "constructor", {
        value: EventTarget,
        configurable: true,
        writable: true
      });
      if (typeof window !== "undefined" && typeof window.EventTarget !== "undefined") {
        Object.setPrototypeOf(EventTarget.prototype, window.EventTarget.prototype);
      }
      exports.defineEventAttribute = defineEventAttribute;
      exports.EventTarget = EventTarget;
      exports.default = EventTarget;
      module.exports = EventTarget;
      module.exports.EventTarget = module.exports["default"] = EventTarget;
      module.exports.defineEventAttribute = defineEventAttribute;
    }
  });

  // ../../node_modules/.bun/abort-controller@3.0.0/node_modules/abort-controller/dist/abort-controller.js
  var require_abort_controller = __commonJS({
    "../../node_modules/.bun/abort-controller@3.0.0/node_modules/abort-controller/dist/abort-controller.js"(exports, module) {
      "use strict";
      Object.defineProperty(exports, "__esModule", { value: true });
      var eventTargetShim = require_event_target_shim();
      var AbortSignal2 = class extends eventTargetShim.EventTarget {
        /**
         * AbortSignal cannot be constructed directly.
         */
        constructor() {
          super();
          throw new TypeError("AbortSignal cannot be constructed directly");
        }
        /**
         * Returns `true` if this `AbortSignal`'s `AbortController` has signaled to abort, and `false` otherwise.
         */
        get aborted() {
          const aborted = abortedFlags.get(this);
          if (typeof aborted !== "boolean") {
            throw new TypeError(`Expected 'this' to be an 'AbortSignal' object, but got ${this === null ? "null" : typeof this}`);
          }
          return aborted;
        }
      };
      eventTargetShim.defineEventAttribute(AbortSignal2.prototype, "abort");
      function createAbortSignal() {
        const signal = Object.create(AbortSignal2.prototype);
        eventTargetShim.EventTarget.call(signal);
        abortedFlags.set(signal, false);
        return signal;
      }
      function abortSignal(signal) {
        if (abortedFlags.get(signal) !== false) {
          return;
        }
        abortedFlags.set(signal, true);
        signal.dispatchEvent({ type: "abort" });
      }
      var abortedFlags = /* @__PURE__ */ new WeakMap();
      Object.defineProperties(AbortSignal2.prototype, {
        aborted: { enumerable: true }
      });
      if (typeof Symbol === "function" && typeof Symbol.toStringTag === "symbol") {
        Object.defineProperty(AbortSignal2.prototype, Symbol.toStringTag, {
          configurable: true,
          value: "AbortSignal"
        });
      }
      var AbortController3 = class {
        /**
         * Initialize this controller.
         */
        constructor() {
          signals.set(this, createAbortSignal());
        }
        /**
         * Returns the `AbortSignal` object associated with this object.
         */
        get signal() {
          return getSignal(this);
        }
        /**
         * Abort and signal to any observers that the associated activity is to be aborted.
         */
        abort() {
          abortSignal(getSignal(this));
        }
      };
      var signals = /* @__PURE__ */ new WeakMap();
      function getSignal(controller) {
        const signal = signals.get(controller);
        if (signal == null) {
          throw new TypeError(`Expected 'this' to be an 'AbortController' object, but got ${controller === null ? "null" : typeof controller}`);
        }
        return signal;
      }
      Object.defineProperties(AbortController3.prototype, {
        signal: { enumerable: true },
        abort: { enumerable: true }
      });
      if (typeof Symbol === "function" && typeof Symbol.toStringTag === "symbol") {
        Object.defineProperty(AbortController3.prototype, Symbol.toStringTag, {
          configurable: true,
          value: "AbortController"
        });
      }
      exports.AbortController = AbortController3;
      exports.AbortSignal = AbortSignal2;
      exports.default = AbortController3;
      module.exports = AbortController3;
      module.exports.AbortController = module.exports["default"] = AbortController3;
      module.exports.AbortSignal = AbortSignal2;
    }
  });

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
    describe: () => describe,
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

  // ../../node_modules/.bun/whatwg-fetch@3.6.20/node_modules/whatwg-fetch/fetch.js
  var g = typeof globalThis !== "undefined" && globalThis || typeof self !== "undefined" && self || // eslint-disable-next-line no-undef
  typeof global !== "undefined" && global || {};
  var support = {
    searchParams: "URLSearchParams" in g,
    iterable: "Symbol" in g && "iterator" in Symbol,
    blob: "FileReader" in g && "Blob" in g && (function() {
      try {
        new Blob();
        return true;
      } catch (e) {
        return false;
      }
    })(),
    formData: "FormData" in g,
    arrayBuffer: "ArrayBuffer" in g
  };
  function isDataView(obj) {
    return obj && DataView.prototype.isPrototypeOf(obj);
  }
  if (support.arrayBuffer) {
    viewClasses = [
      "[object Int8Array]",
      "[object Uint8Array]",
      "[object Uint8ClampedArray]",
      "[object Int16Array]",
      "[object Uint16Array]",
      "[object Int32Array]",
      "[object Uint32Array]",
      "[object Float32Array]",
      "[object Float64Array]"
    ];
    isArrayBufferView = ArrayBuffer.isView || function(obj) {
      return obj && viewClasses.indexOf(Object.prototype.toString.call(obj)) > -1;
    };
  }
  var viewClasses;
  var isArrayBufferView;
  function normalizeName(name) {
    if (typeof name !== "string") {
      name = String(name);
    }
    if (/[^a-z0-9\-#$%&'*+.^_`|~!]/i.test(name) || name === "") {
      throw new TypeError('Invalid character in header field name: "' + name + '"');
    }
    return name.toLowerCase();
  }
  function normalizeValue(value) {
    if (typeof value !== "string") {
      value = String(value);
    }
    return value;
  }
  function iteratorFor(items) {
    var iterator = {
      next: function() {
        var value = items.shift();
        return { done: value === void 0, value };
      }
    };
    if (support.iterable) {
      iterator[Symbol.iterator] = function() {
        return iterator;
      };
    }
    return iterator;
  }
  function Headers(headers) {
    this.map = {};
    if (headers instanceof Headers) {
      headers.forEach(function(value, name) {
        this.append(name, value);
      }, this);
    } else if (Array.isArray(headers)) {
      headers.forEach(function(header) {
        if (header.length != 2) {
          throw new TypeError("Headers constructor: expected name/value pair to be length 2, found" + header.length);
        }
        this.append(header[0], header[1]);
      }, this);
    } else if (headers) {
      Object.getOwnPropertyNames(headers).forEach(function(name) {
        this.append(name, headers[name]);
      }, this);
    }
  }
  Headers.prototype.append = function(name, value) {
    name = normalizeName(name);
    value = normalizeValue(value);
    var oldValue = this.map[name];
    this.map[name] = oldValue ? oldValue + ", " + value : value;
  };
  Headers.prototype["delete"] = function(name) {
    delete this.map[normalizeName(name)];
  };
  Headers.prototype.get = function(name) {
    name = normalizeName(name);
    return this.has(name) ? this.map[name] : null;
  };
  Headers.prototype.has = function(name) {
    return this.map.hasOwnProperty(normalizeName(name));
  };
  Headers.prototype.set = function(name, value) {
    this.map[normalizeName(name)] = normalizeValue(value);
  };
  Headers.prototype.forEach = function(callback, thisArg) {
    for (var name in this.map) {
      if (this.map.hasOwnProperty(name)) {
        callback.call(thisArg, this.map[name], name, this);
      }
    }
  };
  Headers.prototype.keys = function() {
    var items = [];
    this.forEach(function(value, name) {
      items.push(name);
    });
    return iteratorFor(items);
  };
  Headers.prototype.values = function() {
    var items = [];
    this.forEach(function(value) {
      items.push(value);
    });
    return iteratorFor(items);
  };
  Headers.prototype.entries = function() {
    var items = [];
    this.forEach(function(value, name) {
      items.push([name, value]);
    });
    return iteratorFor(items);
  };
  if (support.iterable) {
    Headers.prototype[Symbol.iterator] = Headers.prototype.entries;
  }
  function consumed(body) {
    if (body._noBody) return;
    if (body.bodyUsed) {
      return Promise.reject(new TypeError("Already read"));
    }
    body.bodyUsed = true;
  }
  function fileReaderReady(reader) {
    return new Promise(function(resolve, reject) {
      reader.onload = function() {
        resolve(reader.result);
      };
      reader.onerror = function() {
        reject(reader.error);
      };
    });
  }
  function readBlobAsArrayBuffer(blob) {
    var reader = new FileReader();
    var promise = fileReaderReady(reader);
    reader.readAsArrayBuffer(blob);
    return promise;
  }
  function readBlobAsText(blob) {
    var reader = new FileReader();
    var promise = fileReaderReady(reader);
    var match = /charset=([A-Za-z0-9_-]+)/.exec(blob.type);
    var encoding = match ? match[1] : "utf-8";
    reader.readAsText(blob, encoding);
    return promise;
  }
  function readArrayBufferAsText(buf) {
    var view = new Uint8Array(buf);
    var chars = new Array(view.length);
    for (var i = 0; i < view.length; i++) {
      chars[i] = String.fromCharCode(view[i]);
    }
    return chars.join("");
  }
  function bufferClone(buf) {
    if (buf.slice) {
      return buf.slice(0);
    } else {
      var view = new Uint8Array(buf.byteLength);
      view.set(new Uint8Array(buf));
      return view.buffer;
    }
  }
  function Body() {
    this.bodyUsed = false;
    this._initBody = function(body) {
      this.bodyUsed = this.bodyUsed;
      this._bodyInit = body;
      if (!body) {
        this._noBody = true;
        this._bodyText = "";
      } else if (typeof body === "string") {
        this._bodyText = body;
      } else if (support.blob && Blob.prototype.isPrototypeOf(body)) {
        this._bodyBlob = body;
      } else if (support.formData && FormData.prototype.isPrototypeOf(body)) {
        this._bodyFormData = body;
      } else if (support.searchParams && URLSearchParams.prototype.isPrototypeOf(body)) {
        this._bodyText = body.toString();
      } else if (support.arrayBuffer && support.blob && isDataView(body)) {
        this._bodyArrayBuffer = bufferClone(body.buffer);
        this._bodyInit = new Blob([this._bodyArrayBuffer]);
      } else if (support.arrayBuffer && (ArrayBuffer.prototype.isPrototypeOf(body) || isArrayBufferView(body))) {
        this._bodyArrayBuffer = bufferClone(body);
      } else {
        this._bodyText = body = Object.prototype.toString.call(body);
      }
      if (!this.headers.get("content-type")) {
        if (typeof body === "string") {
          this.headers.set("content-type", "text/plain;charset=UTF-8");
        } else if (this._bodyBlob && this._bodyBlob.type) {
          this.headers.set("content-type", this._bodyBlob.type);
        } else if (support.searchParams && URLSearchParams.prototype.isPrototypeOf(body)) {
          this.headers.set("content-type", "application/x-www-form-urlencoded;charset=UTF-8");
        }
      }
    };
    if (support.blob) {
      this.blob = function() {
        var rejected = consumed(this);
        if (rejected) {
          return rejected;
        }
        if (this._bodyBlob) {
          return Promise.resolve(this._bodyBlob);
        } else if (this._bodyArrayBuffer) {
          return Promise.resolve(new Blob([this._bodyArrayBuffer]));
        } else if (this._bodyFormData) {
          throw new Error("could not read FormData body as blob");
        } else {
          return Promise.resolve(new Blob([this._bodyText]));
        }
      };
    }
    this.arrayBuffer = function() {
      if (this._bodyArrayBuffer) {
        var isConsumed = consumed(this);
        if (isConsumed) {
          return isConsumed;
        } else if (ArrayBuffer.isView(this._bodyArrayBuffer)) {
          return Promise.resolve(
            this._bodyArrayBuffer.buffer.slice(
              this._bodyArrayBuffer.byteOffset,
              this._bodyArrayBuffer.byteOffset + this._bodyArrayBuffer.byteLength
            )
          );
        } else {
          return Promise.resolve(this._bodyArrayBuffer);
        }
      } else if (support.blob) {
        return this.blob().then(readBlobAsArrayBuffer);
      } else {
        throw new Error("could not read as ArrayBuffer");
      }
    };
    this.text = function() {
      var rejected = consumed(this);
      if (rejected) {
        return rejected;
      }
      if (this._bodyBlob) {
        return readBlobAsText(this._bodyBlob);
      } else if (this._bodyArrayBuffer) {
        return Promise.resolve(readArrayBufferAsText(this._bodyArrayBuffer));
      } else if (this._bodyFormData) {
        throw new Error("could not read FormData body as text");
      } else {
        return Promise.resolve(this._bodyText);
      }
    };
    if (support.formData) {
      this.formData = function() {
        return this.text().then(decode);
      };
    }
    this.json = function() {
      return this.text().then(JSON.parse);
    };
    return this;
  }
  var methods = ["CONNECT", "DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT", "TRACE"];
  function normalizeMethod(method) {
    var upcased = method.toUpperCase();
    return methods.indexOf(upcased) > -1 ? upcased : method;
  }
  function Request(input, options) {
    if (!(this instanceof Request)) {
      throw new TypeError('Please use the "new" operator, this DOM object constructor cannot be called as a function.');
    }
    options = options || {};
    var body = options.body;
    if (input instanceof Request) {
      if (input.bodyUsed) {
        throw new TypeError("Already read");
      }
      this.url = input.url;
      this.credentials = input.credentials;
      if (!options.headers) {
        this.headers = new Headers(input.headers);
      }
      this.method = input.method;
      this.mode = input.mode;
      this.signal = input.signal;
      if (!body && input._bodyInit != null) {
        body = input._bodyInit;
        input.bodyUsed = true;
      }
    } else {
      this.url = String(input);
    }
    this.credentials = options.credentials || this.credentials || "same-origin";
    if (options.headers || !this.headers) {
      this.headers = new Headers(options.headers);
    }
    this.method = normalizeMethod(options.method || this.method || "GET");
    this.mode = options.mode || this.mode || null;
    this.signal = options.signal || this.signal || (function() {
      if ("AbortController" in g) {
        var ctrl = new AbortController();
        return ctrl.signal;
      }
    })();
    this.referrer = null;
    if ((this.method === "GET" || this.method === "HEAD") && body) {
      throw new TypeError("Body not allowed for GET or HEAD requests");
    }
    this._initBody(body);
    if (this.method === "GET" || this.method === "HEAD") {
      if (options.cache === "no-store" || options.cache === "no-cache") {
        var reParamSearch = /([?&])_=[^&]*/;
        if (reParamSearch.test(this.url)) {
          this.url = this.url.replace(reParamSearch, "$1_=" + (/* @__PURE__ */ new Date()).getTime());
        } else {
          var reQueryString = /\?/;
          this.url += (reQueryString.test(this.url) ? "&" : "?") + "_=" + (/* @__PURE__ */ new Date()).getTime();
        }
      }
    }
  }
  Request.prototype.clone = function() {
    return new Request(this, { body: this._bodyInit });
  };
  function decode(body) {
    var form = new FormData();
    body.trim().split("&").forEach(function(bytes) {
      if (bytes) {
        var split = bytes.split("=");
        var name = split.shift().replace(/\+/g, " ");
        var value = split.join("=").replace(/\+/g, " ");
        form.append(decodeURIComponent(name), decodeURIComponent(value));
      }
    });
    return form;
  }
  function parseHeaders(rawHeaders) {
    var headers = new Headers();
    var preProcessedHeaders = rawHeaders.replace(/\r?\n[\t ]+/g, " ");
    preProcessedHeaders.split("\r").map(function(header) {
      return header.indexOf("\n") === 0 ? header.substr(1, header.length) : header;
    }).forEach(function(line) {
      var parts = line.split(":");
      var key = parts.shift().trim();
      if (key) {
        var value = parts.join(":").trim();
        try {
          headers.append(key, value);
        } catch (error) {
          console.warn("Response " + error.message);
        }
      }
    });
    return headers;
  }
  Body.call(Request.prototype);
  function Response(bodyInit, options) {
    if (!(this instanceof Response)) {
      throw new TypeError('Please use the "new" operator, this DOM object constructor cannot be called as a function.');
    }
    if (!options) {
      options = {};
    }
    this.type = "default";
    this.status = options.status === void 0 ? 200 : options.status;
    if (this.status < 200 || this.status > 599) {
      throw new RangeError("Failed to construct 'Response': The status provided (0) is outside the range [200, 599].");
    }
    this.ok = this.status >= 200 && this.status < 300;
    this.statusText = options.statusText === void 0 ? "" : "" + options.statusText;
    this.headers = new Headers(options.headers);
    this.url = options.url || "";
    this._initBody(bodyInit);
  }
  Body.call(Response.prototype);
  Response.prototype.clone = function() {
    return new Response(this._bodyInit, {
      status: this.status,
      statusText: this.statusText,
      headers: new Headers(this.headers),
      url: this.url
    });
  };
  Response.error = function() {
    var response = new Response(null, { status: 200, statusText: "" });
    response.ok = false;
    response.status = 0;
    response.type = "error";
    return response;
  };
  var redirectStatuses = [301, 302, 303, 307, 308];
  Response.redirect = function(url, status) {
    if (redirectStatuses.indexOf(status) === -1) {
      throw new RangeError("Invalid status code");
    }
    return new Response(null, { status, headers: { location: url } });
  };
  var DOMException = g.DOMException;
  try {
    new DOMException();
  } catch (err) {
    DOMException = function(message, name) {
      this.message = message;
      this.name = name;
      var error = Error(message);
      this.stack = error.stack;
    };
    DOMException.prototype = Object.create(Error.prototype);
    DOMException.prototype.constructor = DOMException;
  }
  function fetch(input, init) {
    return new Promise(function(resolve, reject) {
      var request = new Request(input, init);
      if (request.signal && request.signal.aborted) {
        return reject(new DOMException("Aborted", "AbortError"));
      }
      var xhr = new XMLHttpRequest();
      function abortXhr() {
        xhr.abort();
      }
      xhr.onload = function() {
        var options = {
          statusText: xhr.statusText,
          headers: parseHeaders(xhr.getAllResponseHeaders() || "")
        };
        if (request.url.indexOf("file://") === 0 && (xhr.status < 200 || xhr.status > 599)) {
          options.status = 200;
        } else {
          options.status = xhr.status;
        }
        options.url = "responseURL" in xhr ? xhr.responseURL : options.headers.get("X-Request-URL");
        var body = "response" in xhr ? xhr.response : xhr.responseText;
        setTimeout(function() {
          resolve(new Response(body, options));
        }, 0);
      };
      xhr.onerror = function() {
        setTimeout(function() {
          reject(new TypeError("Network request failed"));
        }, 0);
      };
      xhr.ontimeout = function() {
        setTimeout(function() {
          reject(new TypeError("Network request timed out"));
        }, 0);
      };
      xhr.onabort = function() {
        setTimeout(function() {
          reject(new DOMException("Aborted", "AbortError"));
        }, 0);
      };
      function fixUrl(url) {
        try {
          return url === "" && g.location.href ? g.location.href : url;
        } catch (e) {
          return url;
        }
      }
      xhr.open(request.method, fixUrl(request.url), true);
      if (request.credentials === "include") {
        xhr.withCredentials = true;
      } else if (request.credentials === "omit") {
        xhr.withCredentials = false;
      }
      if ("responseType" in xhr) {
        if (support.blob) {
          xhr.responseType = "blob";
        } else if (support.arrayBuffer) {
          xhr.responseType = "arraybuffer";
        }
      }
      if (init && typeof init.headers === "object" && !(init.headers instanceof Headers || g.Headers && init.headers instanceof g.Headers)) {
        var names = [];
        Object.getOwnPropertyNames(init.headers).forEach(function(name) {
          names.push(normalizeName(name));
          xhr.setRequestHeader(name, normalizeValue(init.headers[name]));
        });
        request.headers.forEach(function(value, name) {
          if (names.indexOf(name) === -1) {
            xhr.setRequestHeader(name, value);
          }
        });
      } else {
        request.headers.forEach(function(value, name) {
          xhr.setRequestHeader(name, value);
        });
      }
      if (request.signal) {
        request.signal.addEventListener("abort", abortXhr);
        xhr.onreadystatechange = function() {
          if (xhr.readyState === 4) {
            request.signal.removeEventListener("abort", abortXhr);
          }
        };
      }
      xhr.send(typeof request._bodyInit === "undefined" ? null : request._bodyInit);
    });
  }
  fetch.polyfill = true;
  if (!g.fetch) {
    g.fetch = fetch;
    g.Headers = Headers;
    g.Request = Request;
    g.Response = Response;
  }

  // src/rn-globals.ts
  var import_abort_controller = __toESM(require_abort_controller());
  function installReactNativeGlobals(g2 = globalThis) {
    if (typeof g2.Headers === "undefined") g2.Headers = Headers;
    if (typeof g2.Request === "undefined") g2.Request = Request;
    if (typeof g2.Response === "undefined") g2.Response = Response;
    if (typeof g2.AbortController === "undefined") g2.AbortController = import_abort_controller.AbortController;
    if (typeof g2.AbortSignal === "undefined") g2.AbortSignal = import_abort_controller.AbortSignal;
  }

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
    return JSON.stringify(
      value,
      (_key, val) => {
        if (typeof val === "function") return "[Function]";
        return val;
      },
      2
    );
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
  var _totalSnapshotCount = 0;
  function getSnapshotCount() {
    return _totalSnapshotCount;
  }
  function _matchSnapshot(actual) {
    _snapshotCounter++;
    _totalSnapshotCount++;
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
    if (b != null && typeof b === "object" && b.__htMatcher && typeof b.matches === "function")
      return b.matches(a);
    if (a != null && typeof a === "object" && a.__htMatcher && typeof a.matches === "function")
      return a.matches(b);
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

    Expected: not ${formatValue(
            expected
          )}
    Received: ${formatValue(actual)}` : `expect(received).toBe(expected)

    Expected: ${formatValue(
            expected
          )}
    Received: ${formatValue(actual)}`
        );
      },
      toEqual(expected) {
        assert(
          deepEqual(actual, expected),
          negated ? `expect(received).not.toEqual(expected)

    Expected: not ${formatValue(
            expected
          )}
    Received: ${formatValue(actual)}` : `expect(received).toEqual(expected)

    Expected: ${formatValue(
            expected
          )}
    Received: ${formatValue(actual)}`
        );
      },
      toMatchObject(expected) {
        function matchesObject(a, b) {
          if (b != null && typeof b === "object" && b.__htMatcher && typeof b.matches === "function")
            return b.matches(a);
          if (typeof b !== "object" || b === null) return a === b;
          if (Array.isArray(b)) {
            if (!Array.isArray(a) || a.length < b.length) return false;
            return b.every((v, i) => matchesObject(a[i], v));
          }
          for (const key of Object.keys(b)) {
            if (!(key in a) || !matchesObject(a[key], b[key])) return false;
          }
          return true;
        }
        assert(
          matchesObject(actual, expected),
          negated ? `expect(received).not.toMatchObject(expected)

    Expected: not ${formatValue(
            expected
          )}
    Received: ${formatValue(actual)}` : `expect(received).toMatchObject(expected)

    Expected: ${formatValue(
            expected
          )}
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
      toBeGreaterThanOrEqual(n) {
        assert(
          actual >= n,
          negated ? `Expected ${actual} not to be greater than or equal to ${n}` : `Expected ${actual} to be greater than or equal to ${n}`
        );
      },
      toBeLessThanOrEqual(n) {
        assert(
          actual <= n,
          negated ? `Expected ${actual} not to be less than or equal to ${n}` : `Expected ${actual} to be less than or equal to ${n}`
        );
      },
      toHaveProperty(path, value) {
        const parts = Array.isArray(path) ? path : String(path).split(".");
        let cur = actual;
        let found = true;
        for (const part of parts) {
          if (cur !== null && cur !== void 0 && (typeof cur === "object" || typeof cur === "function") && part in cur) {
            cur = cur[part];
          } else {
            found = false;
            break;
          }
        }
        const checkValue = arguments.length > 1;
        const ok = found && (!checkValue || deepEqual(cur, value));
        const label = parts.join(".");
        assert(
          ok,
          negated ? `Expected object not to have property "${label}"${checkValue ? ` with value ${formatValue(value)}` : ""}` : found && checkValue ? `Expected property "${label}" to equal ${formatValue(value)}, received ${formatValue(cur)}` : `Expected object to have property "${label}", received ${formatValue(actual)}`
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
          negated ? `Expected spy not to have been called with ${formatValue(args)}` : `Expected spy to have been called with ${formatValue(args)}, calls: ${formatValue(
            s.calls
          )}`
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
      toHaveBeenCalledOnce() {
        return this.wasCalledOnce();
      },
      toHaveBeenNthCalledWith(n, ...args) {
        const s = actual;
        const call = s && s.calls ? s.calls[n - 1] : void 0;
        const matches = call !== void 0 && deepEqual(call, args);
        assert(
          matches,
          negated ? `Expected spy not to have been called with ${formatValue(args)} on call ${n}` : call === void 0 ? `Expected spy to have been called at least ${n} times, but it was called ${s && s.calls ? s.calls.length : 0} times` : `Expected call ${n} to be ${formatValue(args)}, received ${formatValue(call)}`
        );
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
        if (r !== expected)
          throw new Error(`Expected ${formatValue(expected)}, got ${formatValue(r)}`);
      },
      toEqual: async (expected) => {
        const r = await actual;
        if (!deepEqual(r, expected))
          throw new Error(`Expected deep equal to ${formatValue(expected)}, got ${formatValue(r)}`);
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
      const active = onceImpls.length > 0 ? onceImpls.shift() : baseImpl;
      if (active) {
        ret = new.target ? Reflect.construct(active, args, new.target) : active.apply(this, args);
      } else {
        ret = void 0;
      }
      returnValues.push(ret);
      return ret;
    };
    if (impl && impl.prototype) {
      fn.prototype = impl.prototype;
    }
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
    if (!R)
      throw new Error(
        "react-reconciler not available. Make sure it is installed (it ships with hermes-test)."
      );
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
      c._parent = p;
    },
    appendChild(p, c) {
      p.children.push(c);
      c._parent = p;
    },
    appendChildToContainer(p, c) {
      p.children.push(c);
      c._parent = p;
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
      c._parent = p;
    },
    insertInContainerBefore(p, c, b) {
      const i = p.children.indexOf(b);
      p.children.splice(i, 0, c);
      c._parent = p;
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
          drain();
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
        if (result.length === 0)
          throw new Error(`Unable to find element with ${label}: ${String(arg)}`);
        return result;
      },
      get(root, arg) {
        const result = queryAll(root, arg);
        if (result.length === 0)
          throw new Error(`Unable to find element with ${label}: ${String(arg)}`);
        if (result.length > 1)
          throw new Error(`Found ${result.length} elements with ${label}: ${String(arg)}`);
        return result[0];
      },
      queryAll(root, arg) {
        return queryAll(root, arg);
      },
      query(root, arg) {
        const result = queryAll(root, arg);
        if (result.length > 1)
          throw new Error(`Found ${result.length} elements with ${label}: ${String(arg)}`);
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
        let target = node;
        while (target && !target.props?.onPress) {
          target = target._parent;
        }
        const handler = target?.props?.onPress || node.props?.onPress;
        if (!handler) throw new Error(`No "onPress" handler on <${node.type}>`);
        if ((target || node).props?.disabled) return;
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
        const body2 = input._bodyInit !== void 0 ? input._bodyInit : input._bodyText !== void 0 ? input._bodyText : input.body;
        init = { method: input.method, headers: input.headers, body: body2 };
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
      text: () => Promise.resolve(
        typeof responseBody === "string" ? responseBody : JSON.stringify(responseBody)
      ),
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
    for (const nh of newHandlers) {
      for (let i = handlers.length - 1; i >= 0; i--) {
        const h = handlers[i];
        if (h.method === nh.method && String(h.url) === String(nh.url)) {
          handlers.splice(i, 1);
        }
      }
      for (let i = overrideHandlers.length - 1; i >= 0; i--) {
        const h = overrideHandlers[i];
        if (h.method === nh.method && String(h.url) === String(nh.url)) {
          overrideHandlers.splice(i, 1);
        }
      }
    }
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
    for (const nh of newHandlers) {
      for (let i = overrideHandlers.length - 1; i >= 0; i--) {
        const h = overrideHandlers[i];
        if (h.method === nh.method && String(h.url) === String(nh.url)) {
          overrideHandlers.splice(i, 1);
        }
      }
      overrideHandlers.push(nh);
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
      },
      // The rest of the console surface React Native's console polyfill provides
      // (@react-native/js-polyfills/console.js). Libraries call these (event-target-shim uses
      // console.assert); missing methods were a TypeError, not a no-op.
      assert: (cond, ...args) => {
        if (!cond) p("\x1B[31m\u2717 Assertion failed" + (args.length ? ": " + fmt(...args) : "") + "\x1B[0m");
      },
      trace: (...args) => p(fmt(...args)),
      dir: (...args) => p(fmt(...args)),
      table: (...args) => p(fmt(...args)),
      group: (...args) => {
        if (args.length) p(fmt(...args));
      },
      groupCollapsed: (...args) => {
        if (args.length) p(fmt(...args));
      },
      groupEnd: () => {
      },
      time: () => {
      },
      timeEnd: () => {
      },
      timeLog: () => {
      },
      count: () => {
      },
      countReset: () => {
      }
    };
  })();
  installReactNativeGlobals();
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
      file: globalThis.__currentTestFile,
      filePath: globalThis.__currentTestFilePath
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
  var describe = group;
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
    drain2();
    if (!settled) {
      for (let i = 0; i < 100 && !settled; i++) {
        drain2();
        checkDeadline();
      }
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
      _print(
        ` \x1B[31mFAIL\x1B[0m  ${file} \x1B[2m(${passed} passed, ${failed} failed)\x1B[0m${time}
`
      );
    } else if (globalThis.__HT_coverage) {
      _print(
        `\r\x1B[K \x1B[2mRunning...\x1B[0m ${_filesCompleted}/${_totalFiles} files (${_testsCompleted} tests)`
      );
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
        if (_currentFile) {
          drain2();
        }
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
        const filePath = entry.filePath || globalThis.__currentTestFilePath || entry.file || "unknown";
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
        drain2();
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
        drain2();
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
  var unmock = (_modulePath) => {
  };
  globalThis.ht = { mock, shallow, unmock };
  globalThis.__HT = {
    test,
    expect,
    spy,
    spyOn,
    clearAllMocks,
    group,
    describe,
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
    getSnapshotCount,
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
