import { test, group, beforeEach, afterEach, http, HttpResponse, mockFetch, mockFetchUse, mockFetchReset, flushAsync, expect } from 'hermes-test';
// RTK Query store test — real Redux store, mockFetch intercepts network
// Mirrors: reducers/slices/api/login/__tests__/login.test.ts


import appApi from './api';
import type { LoginPayload } from './api';
import { createTestStore } from './store';

// --- Mock data ---
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

let store: ReturnType<typeof createTestStore>;

beforeEach(() => {
  store = createTestStore();
});

afterEach(() => {
  store.dispatch(appApi.util.resetApiState());
  mockFetchReset();
});

// =============================================
group('login endpoint', () => {
  const payload: LoginPayload = {
    payload: JSON.stringify({ deviceId: 'dev-1', initCode: 'code-1' }),
    signature: 'mock-sig',
  };

  test('successful login returns tokens', () => {
    const result = flushAsync(
      store.dispatch(appApi.endpoints.login.initiate(payload))
    );

    expect(result.data).toEqual(loginMock);
    expect(result.data.accessToken).toBe('mock-access-token-xyz');
  });

  test('failed login returns error', () => {
    mockFetchUse(
      http.post('https://api.example.com/auth/login', () =>
        HttpResponse.json({ message: 'Invalid credentials' }, { status: 400 })
      ),
    );

    const result = flushAsync(
      store.dispatch(appApi.endpoints.login.initiate(payload))
    );

    expect(result.error).toBeDefined();
    expect(result.error.status).toBe(400);
  });
});

// =============================================
group('getProfile query', () => {
  test('fetches user profile', () => {
    const result = flushAsync(
      store.dispatch(appApi.endpoints.getProfile.initiate('user-123'))
    );

    expect(result.data).toEqual(profileMock);
    expect(result.data.name).toBe('Test User');
  });

  test('handles 404 profile', () => {
    mockFetchUse(
      http.get('https://api.example.com/users/user-123', () =>
        HttpResponse.json({ message: 'Not found' }, { status: 404 })
      ),
    );

    const result = flushAsync(
      store.dispatch(appApi.endpoints.getProfile.initiate('user-123'))
    );

    expect(result.error).toBeDefined();
    expect(result.error.status).toBe(404);
  });
});

// =============================================
group('updateProfile mutation', () => {
  test('updates user name', () => {
    const result = flushAsync(
      store.dispatch(
        appApi.endpoints.updateProfile.initiate({ id: 'user-123', name: 'New Name' })
      )
    );

    expect(result.data).toBeDefined();
    expect(result.data.name).toBe('New Name');
  });
});

// =============================================
group('store state', () => {
  test('query result is cached in store', () => {
    flushAsync(store.dispatch(appApi.endpoints.getProfile.initiate('user-123')));

    const state: any = store.getState();
    expect(Object.keys(state.appApi.queries).length).toBeGreaterThan(0);
  });

  test('resetApiState clears cache', () => {
    flushAsync(store.dispatch(appApi.endpoints.getProfile.initiate('user-123')));

    store.dispatch(appApi.util.resetApiState());
    flushAsync(Promise.resolve());

    const state: any = store.getState();
    expect(Object.keys(state.appApi.queries).length).toBe(0);
  });
});
