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
