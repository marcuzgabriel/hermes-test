// Pattern: Multi-API async hook with loading/error/success states
// Demonstrates: renderHook, act, mockFetch, http, HttpResponse, waitFor
import { test, group, beforeEach, afterEach, renderHook, act, waitFor, http, HttpResponse, expect } from 'hermes-test';
import { useState, useEffect, useCallback } from 'react';

// --- Hook under test (self-contained) ---
function useDataFetcher(userId: string) {
  const [users, setUsers] = useState<any>(null);
  const [posts, setPosts] = useState<any[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchAll = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [userRes, postsRes] = await Promise.all([
        fetch(`https://api.example.com/users/${userId}`),
        fetch(`https://api.example.com/users/${userId}/posts`),
      ]);
      if (!userRes.ok || !postsRes.ok) throw new Error('Fetch failed');
      setUsers(await userRes.json());
      setPosts(await postsRes.json());
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }, [userId]);

  useEffect(() => { fetchAll(); }, [fetchAll]);

  return { users, posts, loading, error, refetch: fetchAll };
}

// --- Mock handlers ---
const mockUser = { id: 'u1', name: 'Alice' };
const mockPosts = [{ id: 'p1', title: 'Hello' }, { id: 'p2', title: 'World' }];

ht.mock.fetch(
  http.get('https://api.example.com/users/u1', () => HttpResponse.json(mockUser)),
  http.get('https://api.example.com/users/u1/posts', () => HttpResponse.json(mockPosts)),
);

afterEach(() => { ht.mock.fetch.reset(); });

// --- Tests ---
group('useDataFetcher', () => {
  test('loads user and posts on mount', () => {
    const { current } = renderHook(() => useDataFetcher('u1'));
    waitFor(() => current.loading === false);
    expect(current.users).toEqual(mockUser);
    expect(current.posts).toEqual(mockPosts);
    expect(current.error).toBeNull();
  });

  test('handles API error', () => {
    ht.mock.fetch.overwrite(
      http.get('https://api.example.com/users/u1', () =>
        HttpResponse.json({ msg: 'fail' }, { status: 500 })),
    );
    const { current } = renderHook(() => useDataFetcher('u1'));
    waitFor(() => current.loading === false);
    expect(current.error).toBe('Fetch failed');
    expect(current.users).toBeNull();
  });

  test('refetch reloads data', () => {
    const { current } = renderHook(() => useDataFetcher('u1'));
    waitFor(() => current.loading === false);
    act(() => { current.refetch(); });
    waitFor(() => current.loading === false);
    expect(current.users).toEqual(mockUser);
  });
});
