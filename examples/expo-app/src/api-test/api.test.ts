// RTK Query store test — real Redux store, mockFetch intercepts network
// Mirrors: reducers/slices/api/login/__tests__/login.test.ts

const { test, group, beforeEach, afterEach, http, HttpResponse, mockFetch, mockFetchUse, mockFetchReset, expect } =
  (globalThis as any).__metroTest;

import appApi from './api';
import type { LoginPayload } from './api';
import { createTestStore } from './store';

// --- Mock data (like loginMock in real app) ---
const loginMock = {
  accessToken: 'mock-access-token-xyz',
  refreshToken: 'mock-refresh-token-abc',
  expiresIn: 3600,
};

const profileMock = {
  id: 'user-123',
  name: 'Test User',
  email: 'test@example.com',
};

// --- Base handlers (like jestApiMockServerHandlers.ts) ---
mockFetch(
  http.post('https://api.example.com/auth/login', () =>
    HttpResponse.json(loginMock)
  ),
  http.get('https://api.example.com/users/user-123', () =>
    HttpResponse.json(profileMock)
  ),
  http.put('https://api.example.com/users/user-123', (req: any) =>
    HttpResponse.json({ ...profileMock, ...req.body })
  ),
);

// --- Fresh store per test (like storeRef pattern) ---
let store: ReturnType<typeof createTestStore>;

beforeEach(() => {
  store = createTestStore();
});

afterEach(() => {
  store.dispatch(appApi.util.resetApiState());
  mockFetchReset();
});

// =============================================
// Login endpoint
// =============================================
group('login endpoint', () => {
  const payload: LoginPayload = {
    payload: JSON.stringify({ deviceId: 'dev-1', initCode: 'code-1' }),
    signature: 'mock-sig',
  };

  test('successful login returns tokens', ({ expect }: any) => {
    const res: any = store.dispatch(
      appApi.endpoints.login.initiate(payload)
    );

    // RTK Query returns a promise-like thunk result
    expect(res).toBeDefined();

    // Wait for the async result
    let result: any;
    res.then((r: any) => { result = r; });

    // Drain microtasks to resolve the fetch + RTK Query processing
    const drain = (globalThis as any).__drainMicrotasks || (() => {});
    for (let i = 0; i < 500; i++) drain();

    expect(result).toBeDefined();
    expect(result.data).toEqual(loginMock);
    expect(result.data.accessToken).toBe('mock-access-token-xyz');
  });

  test('failed login returns error', ({ expect }: any) => {
    // Override the login handler for this test (like jestApiMockServer.use())
    mockFetchUse(
      http.post('https://api.example.com/auth/login', () =>
        HttpResponse.json({ message: 'Invalid credentials' }, { status: 400 })
      ),
    );

    let result: any;
    store.dispatch(appApi.endpoints.login.initiate(payload))
      .then((r: any) => { result = r; });

    const drain = (globalThis as any).__drainMicrotasks || (() => {});
    for (let i = 0; i < 500; i++) drain();

    expect(result).toBeDefined();
    expect(result.error).toBeDefined();
    expect(result.error.status).toBe(400);
  });
});

// =============================================
// Query endpoint (getProfile)
// =============================================
group('getProfile query', () => {
  test('fetches user profile', ({ expect }: any) => {
    let result: any;
    store.dispatch(appApi.endpoints.getProfile.initiate('user-123'))
      .then((r: any) => { result = r; });

    const drain = (globalThis as any).__drainMicrotasks || (() => {});
    for (let i = 0; i < 500; i++) drain();

    expect(result).toBeDefined();
    expect(result.data).toEqual(profileMock);
    expect(result.data.name).toBe('Test User');
    expect(result.data.email).toBe('test@example.com');
  });

  test('handles 404 profile', ({ expect }: any) => {
    mockFetchUse(
      http.get('https://api.example.com/users/user-123', () =>
        HttpResponse.json({ message: 'Not found' }, { status: 404 })
      ),
    );

    let result: any;
    store.dispatch(appApi.endpoints.getProfile.initiate('user-123'))
      .then((r: any) => { result = r; });

    const drain = (globalThis as any).__drainMicrotasks || (() => {});
    for (let i = 0; i < 500; i++) drain();

    expect(result).toBeDefined();
    expect(result.error).toBeDefined();
    expect(result.error.status).toBe(404);
  });
});

// =============================================
// Mutation endpoint (updateProfile)
// =============================================
group('updateProfile mutation', () => {
  test('updates user name', ({ expect }: any) => {
    let result: any;
    store.dispatch(
      appApi.endpoints.updateProfile.initiate({ id: 'user-123', name: 'New Name' })
    ).then((r: any) => { result = r; });

    const drain = (globalThis as any).__drainMicrotasks || (() => {});
    for (let i = 0; i < 500; i++) drain();

    expect(result).toBeDefined();
    expect(result.data).toBeDefined();
    expect(result.data.name).toBe('New Name');
  });
});

// =============================================
// Store state after dispatch
// =============================================
group('store state', () => {
  test('login result is cached in store', ({ expect }: any) => {
    const payload: LoginPayload = {
      payload: 'test',
      signature: 'sig',
    };

    store.dispatch(appApi.endpoints.login.initiate(payload))
      .then(() => {});

    const drain = (globalThis as any).__drainMicrotasks || (() => {});
    for (let i = 0; i < 500; i++) drain();

    const state: any = store.getState();
    expect(state.appApi).toBeDefined();
    // RTK Query caches mutations too
    expect(state.appApi.mutations).toBeDefined();
  });

  test('resetApiState clears cache', ({ expect }: any) => {
    store.dispatch(appApi.endpoints.getProfile.initiate('user-123'))
      .then(() => {});

    const drain = (globalThis as any).__drainMicrotasks || (() => {});
    for (let i = 0; i < 500; i++) drain();

    // Should have cached query
    let state: any = store.getState();
    expect(Object.keys(state.appApi.queries).length).toBeGreaterThan(0);

    // Reset
    store.dispatch(appApi.util.resetApiState());
    for (let i = 0; i < 50; i++) drain();

    state = store.getState();
    expect(Object.keys(state.appApi.queries).length).toBe(0);
  });
});
