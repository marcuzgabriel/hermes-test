const { test, renderHook, waitFor, spy } = (globalThis as any).__HT;
import { useUser } from './useUser';

test('useUser loads user data', ({ expect }: any) => {
  const fetchUser = spy(async (id: number) => ({ id, name: 'Test User' }));

  const result = renderHook(() => useUser(1, { fetchUser }));

  // With our synchronous act(), the effect + promise may resolve immediately
  waitFor(() => result.current.user !== null);

  expect(result.current.user).toEqual({ id: 1, name: 'Test User' });
  expect(result.current.loading).toBe(false);
  expect(result.current.error).toBeNull();
  expect(fetchUser).wasCalledWith(1);
});

test('useUser handles errors', ({ expect }: any) => {
  const fetchUser = spy(async () => { throw new Error('Network error'); });

  const result = renderHook(() => useUser(1, { fetchUser }));

  waitFor(() => result.current.error !== null);

  expect(result.current.user).toBeNull();
  expect(result.current.error).toBe('Network error');
  expect(result.current.loading).toBe(false);
});

test('useUser tracks state history', ({ expect }: any) => {
  const fetchUser = spy(async (id: number) => ({ id, name: 'User ' + id }));

  const result = renderHook(() => useUser(1, { fetchUser }));

  waitFor(() => result.current.user !== null);

  // History captures every render
  expect(result.history.length).toBeGreaterThan(1);
  // Final state has the user
  const last = result.history[result.history.length - 1];
  expect(last.loading).toBe(false);
  expect(last.user).toEqual({ id: 1, name: 'User 1' });
});
