// Real production hook — simplified auth flow
// Patterns: async token fetch, keychain storage, biometric check, session management
import { useCallback, useState } from 'react';

interface Session { accessToken: string; refreshToken: string; expiresIn: number; }
interface LoginResult { success: boolean; session?: Session; error?: string; }

// Simulated storage (in prod: expo-secure-store / keychain)
export const keychainStorage = {
  _store: {} as Record<string, any>,
  async get(key: string) { return this._store[key] ?? null; },
  async set(key: string, value: any) { this._store[key] = value; },
  async remove(key: string) { delete this._store[key]; },
  reset() { this._store = {}; },
};

export const useLogin = (deps: {
  authenticate?: () => Promise<{ success: boolean }>;
  openAuthSession?: (url: string) => Promise<{ type: string; url?: string }>;
  fetchTokens?: (code: string) => Promise<Session>;
  signPayload?: (payload: string) => string;
}) => {
  const [session, setSession] = useState<Session | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  const requestTokensFromKeychain = useCallback(async (): Promise<LoginResult> => {
    setIsLoading(true);
    try {
      const stored = await keychainStorage.get('session');
      if (!stored) return { success: false, error: 'No stored session' };
      if (deps.authenticate) {
        const authResult = await deps.authenticate();
        if (!authResult.success) return { success: false, error: 'Biometric failed' };
      }
      setSession(stored);
      return { success: true, session: stored };
    } finally { setIsLoading(false); }
  }, [deps]);

  const webViewLogin = useCallback(async (authUrl: string): Promise<LoginResult> => {
    setIsLoading(true);
    try {
      if (!deps.openAuthSession) return { success: false, error: 'No auth session handler' };
      const result = await deps.openAuthSession(authUrl);
      if (result.type !== 'success' || !result.url) return { success: false, error: 'Auth cancelled' };
      const code = new URL(result.url).searchParams.get('code');
      if (!code) return { success: false, error: 'No auth code' };
      if (!deps.fetchTokens) return { success: false, error: 'No token fetcher' };
      const newSession = await deps.fetchTokens(code);
      await keychainStorage.set('session', newSession);
      setSession(newSession);
      return { success: true, session: newSession };
    } finally { setIsLoading(false); }
  }, [deps]);

  const setSessionManual = useCallback(async (s: Session) => {
    await keychainStorage.set('session', s);
    setSession(s);
  }, []);

  return { session, isLoading, requestTokensFromKeychain, webViewLogin, setSession: setSessionManual };
};
