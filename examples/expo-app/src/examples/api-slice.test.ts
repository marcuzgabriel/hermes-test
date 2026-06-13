// Pattern: RTK Query API slice with real Redux store
// Demonstrates: setupApiStore, mockFetch, flushAsync, http, HttpResponse
import { test, group, beforeEach, afterEach, setupApiStore, flushAsync, http, HttpResponse, expect, mock } from 'hermes-test';
import { createApi, fetchBaseQuery } from '@reduxjs/toolkit/query';

// --- API slice (self-contained) ---
const postsApi = createApi({
  reducerPath: 'postsApi',
  baseQuery: fetchBaseQuery({ baseUrl: 'https://api.example.com' }),
  tagTypes: ['Post'],
  endpoints: (b) => ({
    getPosts: b.query<any[], void>({ query: () => '/posts', providesTags: ['Post'] }),
    createPost: b.mutation<any, { title: string }>({
      query: (body) => ({ url: '/posts', method: 'POST', body }),
      invalidatesTags: ['Post'],
    }),
  }),
});

// --- Mock data ---
const mockPosts = [{ id: 1, title: 'First' }, { id: 2, title: 'Second' }];

mock.fetch(
  http.get('https://api.example.com/posts', () => HttpResponse.json(mockPosts)),
  http.post('https://api.example.com/posts', (req: any) =>
    HttpResponse.json({ id: 3, ...req.body })),
);

let ctx: ReturnType<typeof setupApiStore>;

beforeEach(() => { ctx = setupApiStore([postsApi]); });
afterEach(() => { ctx.store.dispatch(postsApi.util.resetApiState()); mock.fetch.reset(); });

// --- Tests ---
group('postsApi', () => {
  test('getPosts returns list', () => {
    const result = flushAsync(ctx.dispatch(postsApi.endpoints.getPosts.initiate()));
    expect(result.data).toEqual(mockPosts);
    expect(result.data.length).toBe(2);
  });

  test('getPosts error response', () => {
    mock.fetch.overwrite(http.get('https://api.example.com/posts', () =>
      HttpResponse.json({ msg: 'forbidden' }, { status: 403 })));
    const result = flushAsync(ctx.dispatch(postsApi.endpoints.getPosts.initiate()));
    expect(result.error).toBeDefined();
    expect(result.error.status).toBe(403);
  });

  test('createPost mutation', () => {
    const result = flushAsync(
      ctx.dispatch(postsApi.endpoints.createPost.initiate({ title: 'New Post' })));
    expect(result.data.title).toBe('New Post');
    expect(result.data.id).toBe(3);
  });

  test('query result is cached in store', () => {
    flushAsync(ctx.dispatch(postsApi.endpoints.getPosts.initiate()));
    const state: any = ctx.getState();
    expect(Object.keys(state.postsApi.queries).length).toBeGreaterThan(0);
  });
});
