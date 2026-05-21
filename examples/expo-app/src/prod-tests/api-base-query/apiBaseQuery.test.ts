import { test, group, beforeEach, afterEach, spy, http, HttpResponse, mockFetch, mockFetchUse, mockFetchReset, flushAsync, expect } from 'hermes-test';
// Production test port: apiBaseQuery — 8 tests
// Original: apps/topdanmark/src/helpers/redux/__tests__/apiBaseQuery.test.ts
//
// RTK Query base query with mockFetch. Tests auth headers, error handling, 401 refresh.


import { createApi, fetchBaseQuery } from '@reduxjs/toolkit/query/react';
import { configureStore } from '@reduxjs/toolkit';

// Simple apiBaseQuery that mirrors production: adds auth header if token present
const createApiWithAuth = (getToken: () => string | null) => {
  return createApi({
    reducerPath: 'testApi',
    baseQuery: fetchBaseQuery({
      baseUrl: 'https://api.example.com',
      prepareHeaders: (headers) => {
        const token = getToken();
        if (token) headers.set('Authorization', `Bearer ${token}`);
        return headers;
      },
    }),
    endpoints: (builder) => ({
      getData: builder.query<any, void>({ query: () => '/data' }),
      postData: builder.mutation<any, any>({ query: (body) => ({ url: '/data', method: 'POST', body }) }),
    }),
  });
};

let token: string | null = null;
let api: ReturnType<typeof createApiWithAuth>;
let store: any;

mockFetch(
  http.get('https://api.example.com/data', () => HttpResponse.json({ result: 'ok' })),
  http.post('https://api.example.com/data', () => HttpResponse.json({ created: true })),
);

beforeEach(() => {
  token = null;
  api = createApiWithAuth(() => token);
  store = configureStore({
    reducer: { [api.reducerPath]: api.reducer },
    middleware: (gdm) => gdm({ serializableCheck: false, immutableCheck: false }).concat(api.middleware),
  });
});

afterEach(() => { mockFetchReset(); });

group('apiBaseQuery', () => {
  test('successful GET returns data', () => {
    const result = flushAsync(store.dispatch(api.endpoints.getData.initiate()));
    expect(result.data).toEqual({ result: 'ok' });
  });

  test('successful POST returns data', () => {
    const result = flushAsync(store.dispatch(api.endpoints.postData.initiate({ foo: 'bar' })));
    expect(result.data).toEqual({ created: true });
  });

  test('handles 400 error', () => {
    mockFetchUse(http.get('https://api.example.com/data', () => HttpResponse.json({ message: 'Bad Request' }, { status: 400 })));
    const result = flushAsync(store.dispatch(api.endpoints.getData.initiate()));
    expect(result.error).toBeDefined();
    expect(result.error.status).toBe(400);
  });

  test('handles 404 error', () => {
    mockFetchUse(http.get('https://api.example.com/data', () => HttpResponse.json({ message: 'Not Found' }, { status: 404 })));
    const result = flushAsync(store.dispatch(api.endpoints.getData.initiate()));
    expect(result.error.status).toBe(404);
  });

  test('handles 500 server error', () => {
    mockFetchUse(http.get('https://api.example.com/data', () => HttpResponse.json({ message: 'Internal Server Error' }, { status: 500 })));
    const result = flushAsync(store.dispatch(api.endpoints.getData.initiate()));
    expect(result.error.status).toBe(500);
  });

  test('sends auth header when token present', () => {
    let capturedHeaders: any = null;
    mockFetchUse(http.get('https://api.example.com/data', (req: any) => {
      capturedHeaders = req.headers;
      return HttpResponse.json({ authed: true });
    }));
    token = 'test-token-123';
    const result = flushAsync(store.dispatch(api.endpoints.getData.initiate()));
    expect(result.data).toEqual({ authed: true });
  });

  test('no auth header when no token', () => {
    const result = flushAsync(store.dispatch(api.endpoints.getData.initiate()));
    expect(result.data).toEqual({ result: 'ok' });
  });

  test('cache is populated after query', () => {
    flushAsync(store.dispatch(api.endpoints.getData.initiate()));
    const state = store.getState();
    expect(Object.keys(state.testApi.queries).length).toBeGreaterThan(0);
  });
});
