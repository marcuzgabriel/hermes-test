import { useState, useEffect, useCallback } from 'react';
import { fetchUser as defaultFetchUser } from './api';

// Accept fetchUser as a parameter for testability (dependency injection)
export function useUser(id: number, deps?: { fetchUser?: typeof defaultFetchUser }) {
  const fetchUser = deps?.fetchUser ?? defaultFetchUser;
  const [user, setUser] = useState<{ id: number; name: string } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await fetchUser(id);
      setUser(data);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }, [id, fetchUser]);

  useEffect(() => {
    load();
  }, [load]);

  return { user, loading, error, reload: load };
}
