import { test, group, beforeEach, renderHook, act, flushAsync, spy, expect } from 'hermes-test';
// Production test port: useLogin — 12 tests

import { useLogin, keychainStorage } from './useLogin';

const mockSession = { accessToken: 'access-123', refreshToken: 'refresh-456', expiresIn: 3600 };

let authenticateMock: any;
let openAuthSessionMock: any;
let fetchTokensMock: any;

beforeEach(() => {
  keychainStorage.reset();
  authenticateMock = spy(async () => ({ success: true }));
  openAuthSessionMock = spy(async () => ({ type: 'success', url: 'https://callback?code=auth-code-123' }));
  fetchTokensMock = spy(async () => mockSession);
});

const makeDeps = () => ({ authenticate: authenticateMock, openAuthSession: openAuthSessionMock, fetchTokens: fetchTokensMock });

group('requestTokensFromKeychain', () => {
  test('returns stored session with successful biometric', () => {
    keychainStorage._store['session'] = mockSession;
    const hook = renderHook(() => useLogin(makeDeps()));
    const result = flushAsync(hook.current.requestTokensFromKeychain());
    expect(result.success).toBe(true);
    expect(result.session).toEqual(mockSession);
    expect(authenticateMock).wasCalledOnce();
  });

  test('fails when no stored session', () => {
    const hook = renderHook(() => useLogin(makeDeps()));
    const result = flushAsync(hook.current.requestTokensFromKeychain());
    expect(result.success).toBe(false);
    expect(result.error).toContain('No stored session');
  });

  test('fails when biometric fails', () => {
    keychainStorage._store['session'] = mockSession;
    authenticateMock = spy(async () => ({ success: false }));
    const hook = renderHook(() => useLogin({ ...makeDeps(), authenticate: authenticateMock }));
    const result = flushAsync(hook.current.requestTokensFromKeychain());
    expect(result.success).toBe(false);
    expect(result.error).toContain('Biometric');
  });

  test('works without biometric (no authenticate dep)', () => {
    keychainStorage._store['session'] = mockSession;
    const hook = renderHook(() => useLogin({ ...makeDeps(), authenticate: undefined }));
    const result = flushAsync(hook.current.requestTokensFromKeychain());
    expect(result.success).toBe(true);
  });
});

group('webViewLogin', () => {
  test('successful web login stores session', () => {
    const hook = renderHook(() => useLogin(makeDeps()));
    const result = flushAsync(hook.current.webViewLogin('https://auth.example.com'));
    expect(result.success).toBe(true);
    expect(result.session).toEqual(mockSession);
    expect(openAuthSessionMock).wasCalledOnce();
    expect(fetchTokensMock).wasCalledWith('auth-code-123');
    expect(keychainStorage._store['session']).toEqual(mockSession);
  });

  test('fails when auth cancelled', () => {
    openAuthSessionMock = spy(async () => ({ type: 'cancel' }));
    const hook = renderHook(() => useLogin({ ...makeDeps(), openAuthSession: openAuthSessionMock }));
    const result = flushAsync(hook.current.webViewLogin('https://auth.example.com'));
    expect(result.success).toBe(false);
    expect(result.error).toContain('cancelled');
  });

  test('fails when no code in callback URL', () => {
    openAuthSessionMock = spy(async () => ({ type: 'success', url: 'https://callback?nocode=true' }));
    const hook = renderHook(() => useLogin({ ...makeDeps(), openAuthSession: openAuthSessionMock }));
    const result = flushAsync(hook.current.webViewLogin('https://auth.example.com'));
    expect(result.success).toBe(false);
    expect(result.error).toContain('No auth code');
  });

  test('fails when no openAuthSession handler', () => {
    const hook = renderHook(() => useLogin({ ...makeDeps(), openAuthSession: undefined }));
    const result = flushAsync(hook.current.webViewLogin('https://auth.example.com'));
    expect(result.success).toBe(false);
  });
});

group('setSession', () => {
  test('manually sets session and stores in keychain', () => {
    const hook = renderHook(() => useLogin(makeDeps()));
    act(() => { flushAsync(hook.current.setSession(mockSession)); });
    expect(hook.current.session).toEqual(mockSession);
    expect(keychainStorage._store['session']).toEqual(mockSession);
  });

  test('overwrites previous session', () => {
    keychainStorage._store['session'] = { accessToken: 'old', refreshToken: 'old', expiresIn: 100 };
    const hook = renderHook(() => useLogin(makeDeps()));
    act(() => { flushAsync(hook.current.setSession(mockSession)); });
    expect(hook.current.session.accessToken).toBe('access-123');
  });

  test('session starts as null', () => {
    const hook = renderHook(() => useLogin(makeDeps()));
    expect(hook.current.session).toBeNull();
    expect(hook.current.isLoading).toBe(false);
  });
});
