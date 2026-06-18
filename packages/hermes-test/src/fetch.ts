// mock.fetch — lightweight fetch mock for Hermes
// Replaces globalThis.fetch with a handler-based mock (like MSW but pure JS)

type Method = 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH' | 'HEAD' | 'OPTIONS';

interface MockHandler {
  method: Method;
  url: string | RegExp;
  handler: (req: MockRequest) => MockResponseInit;
  once: boolean;
}

interface MockRequest {
  method: string;
  url: string;
  headers: Record<string, string>;
  body: any;
}

interface MockResponseInit {
  status?: number;
  statusText?: string;
  headers?: Record<string, string>;
  body?: any;
}

const handlers: MockHandler[] = [];
const overrideHandlers: MockHandler[] = [];

function matchUrl(pattern: string | RegExp, url: string): boolean {
  if (pattern instanceof RegExp) return pattern.test(url);
  // Exact match or prefix match (ignore query params for prefix)
  if (url === pattern) return true;
  if (url.startsWith(pattern + '?')) return true;
  // Wildcard: pattern without query matches url base
  const urlBase = url.split('?')[0];
  return urlBase === pattern;
}

function findHandler(method: string, url: string): MockHandler | undefined {
  // Override handlers take priority (per-test overrides, like MSW server.use())
  for (let i = overrideHandlers.length - 1; i >= 0; i--) {
    const h = overrideHandlers[i];
    if (h.method === method.toUpperCase() && matchUrl(h.url, url)) {
      if (h.once) overrideHandlers.splice(i, 1);
      return h;
    }
  }
  // Then base handlers
  for (let i = handlers.length - 1; i >= 0; i--) {
    const h = handlers[i];
    if (h.method === method.toUpperCase() && matchUrl(h.url, url)) {
      if (h.once) handlers.splice(i, 1);
      return h;
    }
  }
  return undefined;
}

// The fake fetch
function fakeFetch(input: any, init?: any): any {
  // Handle Request objects, strings, and URL objects
  let url: string;
  if (typeof input === 'string') {
    url = input;
  } else if (input && typeof input === 'object') {
    url = input.url || input.href || String(input);
    // If init wasn't provided, pull from the Request object
    if (!init && input.method) {
      init = { method: input.method, headers: input.headers, body: input.body };
    }
  } else {
    url = String(input);
  }
  const method = (init?.method || 'GET').toUpperCase();
  let body = init?.body;
  if (typeof body === 'string') {
    try {
      body = JSON.parse(body);
    } catch {}
  }

  const reqHeaders: Record<string, string> = {};
  if (init?.headers) {
    if (typeof init.headers.forEach === 'function') {
      init.headers.forEach((v: string, k: string) => {
        reqHeaders[k] = v;
      });
    } else {
      Object.assign(reqHeaders, init.headers);
    }
  }

  const handler = findHandler(method, url);

  if (!handler) {
    // Unhandled request — return a rejected-style response
    const msg = `[mock.fetch] Unhandled ${method} ${url}`;
    // Return a promise that resolves to a 500 response
    return Promise.resolve({
      ok: false,
      status: 500,
      statusText: msg,
      headers: { get: () => null, has: () => false },
      json: () => Promise.resolve({ error: msg }),
      text: () => Promise.resolve(msg),
    });
  }

  const req: MockRequest = { method, url, headers: reqHeaders, body };
  const res = handler.handler(req);
  const status = res.status ?? 200;

  const responseBody = res.body;
  const responseHeaders = res.headers || {};
  if (responseBody !== undefined && !responseHeaders['content-type']) {
    responseHeaders['content-type'] = 'application/json';
  }

  return Promise.resolve({
    ok: status >= 200 && status < 300,
    status,
    statusText: res.statusText || (status === 200 ? 'OK' : 'Error'),
    headers: {
      get: (k: string) => responseHeaders[k.toLowerCase()] || null,
      has: (k: string) => k.toLowerCase() in responseHeaders,
    },
    json: () => Promise.resolve(responseBody),
    text: () =>
      Promise.resolve(
        typeof responseBody === 'string' ? responseBody : JSON.stringify(responseBody),
      ),
    clone: function () {
      return this;
    },
  });
}

// --- Public API ---

function createHandler(
  method: Method,
  url: string | RegExp,
  response: MockResponseInit | ((req: MockRequest) => MockResponseInit),
  once = false,
): MockHandler {
  const handler = typeof response === 'function' ? response : () => response;
  return { method, url, handler, once };
}

// Register handlers — automatically replaces any existing handler with same method+url
export function mockFetch(...newHandlers: MockHandler[]): void {
  for (const nh of newHandlers) {
    // Remove existing handler with same method+url (auto-overwrite)
    for (let i = handlers.length - 1; i >= 0; i--) {
      const h = handlers[i];
      if (h.method === nh.method && String(h.url) === String(nh.url)) {
        handlers.splice(i, 1);
      }
    }
    // Also remove from overrides
    for (let i = overrideHandlers.length - 1; i >= 0; i--) {
      const h = overrideHandlers[i];
      if (h.method === nh.method && String(h.url) === String(nh.url)) {
        overrideHandlers.splice(i, 1);
      }
    }
  }
  handlers.push(...newHandlers);
  // Install fetch on globalThis
  (globalThis as any).fetch = fakeFetch;
}

// http helper — mirrors MSW's http.get/post/etc API
export const http = {
  get(url: string | RegExp, handler: (req: MockRequest) => MockResponseInit): MockHandler {
    return createHandler('GET', url, handler);
  },
  post(url: string | RegExp, handler: (req: MockRequest) => MockResponseInit): MockHandler {
    return createHandler('POST', url, handler);
  },
  put(url: string | RegExp, handler: (req: MockRequest) => MockResponseInit): MockHandler {
    return createHandler('PUT', url, handler);
  },
  delete(url: string | RegExp, handler: (req: MockRequest) => MockResponseInit): MockHandler {
    return createHandler('DELETE', url, handler);
  },
  patch(url: string | RegExp, handler: (req: MockRequest) => MockResponseInit): MockHandler {
    return createHandler('PATCH', url, handler);
  },
};

// Response helpers — mirrors MSW's HttpResponse
export const HttpResponse = {
  json(data: any, init?: { status?: number; headers?: Record<string, string> }) {
    return { body: data, status: init?.status ?? 200, headers: init?.headers };
  },
  text(data: string, init?: { status?: number }) {
    return { body: data, status: init?.status ?? 200 };
  },
  error() {
    return { status: 500, body: { error: 'Internal Server Error' } };
  },
};

// Per-test overrides — now just delegates to mockFetch (auto-overwrite handles dedup)
// Kept for backwards compatibility with existing tests using mock.fetch.overwrite()
export function mockFetchUse(...newHandlers: MockHandler[]): void {
  mockFetch(...newHandlers);
}

// Reset per-test overrides (like MSW server.resetHandlers())
export function mockFetchReset(): void {
  overrideHandlers.length = 0;
}

// Clear everything
export function mockFetchClear(): void {
  handlers.length = 0;
  overrideHandlers.length = 0;
}
