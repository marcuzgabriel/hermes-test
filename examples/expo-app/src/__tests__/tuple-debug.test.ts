import { test, expect, mockFetch, http, HttpResponse, flushAsync } from 'hermes-test';

// Test the Reflect.construct Tuple replacement in isolation
test('Reflect.construct Tuple basics', () => {
  // This is exactly what our new Patch 3 injects
  const Tuple = (function() {
    function Tuple() {
      var instance = Reflect.construct(Array, Array.prototype.slice.call(arguments), Tuple);
      return instance;
    }
    Tuple.prototype = Object.create(Array.prototype, {
      constructor: { value: Tuple, writable: true, configurable: true }
    });
    Object.setPrototypeOf(Tuple, Array);
    Object.defineProperty(Tuple, Symbol.species, { get: function() { return Tuple; } });
    Tuple.prototype.concat = function() {
      return Array.prototype.concat.apply(this, arguments);
    };
    Tuple.prototype.prepend = function() {
      var arr = Array.prototype.slice.call(arguments);
      if (arr.length === 1 && Array.isArray(arr[0])) {
        return new Tuple(...arr[0].concat(this));
      }
      return new Tuple(...arr.concat(this));
    };
    return Tuple;
  })();

  // Basic construction
  const t = new (Tuple as any)('a', 'b', 'c');
  expect(Array.isArray(t)).toBe(true);
  expect(t instanceof Array).toBe(true);
  expect(t.length).toBe(3);
  expect(t[0]).toBe('a');

  // concat with functions (like middleware)
  const thunkMw = function thunk() { return 'thunk'; };
  const apiMw = function api() { return 'api'; };
  const mw = new (Tuple as any)(thunkMw);
  const result = mw.concat(apiMw);
  expect(result.length).toBe(2);
  expect(typeof result[0]).toBe('function');
  expect(typeof result[1]).toBe('function');

  // concat with arrays
  const r2 = mw.concat([apiMw]);
  expect(r2.length).toBe(2);

  // native concat with Tuple as arg
  const r3 = [1, 2].concat(t);
  expect(r3.length).toBe(5);
  expect(r3).toEqual([1, 2, 'a', 'b', 'c']);

  // prepend
  const r4 = mw.prepend(apiMw);
  expect(r4.length).toBe(2);
  expect(typeof r4[0]).toBe('function');
});

// Now test with actual RTK configureStore + mockFetch (the failing scenario)
import { configureStore } from '@reduxjs/toolkit';
import { createApi, fetchBaseQuery } from '@reduxjs/toolkit/query/react';

mockFetch(
  http.get('https://api.example.com/data', () => HttpResponse.json({ result: 'ok' })),
);

test('configureStore + dispatch with mockFetch', () => {
  const api = createApi({
    reducerPath: 'testApi',
    baseQuery: fetchBaseQuery({ baseUrl: 'https://api.example.com' }),
    endpoints: (builder) => ({
      getData: builder.query<any, void>({ query: () => '/data' }),
    }),
  });

  const store = configureStore({
    reducer: { [api.reducerPath]: api.reducer },
    middleware: (gdm) => gdm({ serializableCheck: false, immutableCheck: false }).concat(api.middleware),
  });

  const result = flushAsync(store.dispatch(api.endpoints.getData.initiate()));
  expect(result.data).toEqual({ result: 'ok' });
});
