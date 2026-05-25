// Pattern: Hook with mocked module dependencies
// Demonstrates: mockModule, spy, shadow wrapper mock isolation
import { test, group, beforeEach, renderHook, act, spy, mockModule, expect } from 'hermes-test';
import { useState, useCallback } from 'react';

// --- Mock return values (changed per test) ---
let authReturn: any = { userId: 'u1', token: 'tok123' };
let settingsReturn: any = { theme: 'dark', locale: 'en' };
const analyticsTrack = spy((_event: string, _data?: any) => {});

// --- Register mocks BEFORE imports ---
mockModule('./useAuth', () => ({ useAuth: () => authReturn }));
mockModule('./useSettings', () => ({ useSettings: () => settingsReturn }));
mockModule('./useAnalytics', () => ({ useAnalytics: () => ({ track: analyticsTrack }) }));

// --- Hook under test (uses mocked deps) ---
// In a real project this would be imported from its own file.
// Here we inline it so the test is self-contained.
const useAuth = () => authReturn;
const useSettings = () => settingsReturn;
const useAnalytics = () => ({ track: analyticsTrack });

function useProfile() {
  const { userId, token } = useAuth();
  const { theme, locale } = useSettings();
  const { track } = useAnalytics();
  const [viewed, setViewed] = useState(false);

  const markViewed = useCallback(() => {
    track('profile_viewed', { userId });
    setViewed(true);
  }, [userId, track]);

  return { userId, token, theme, locale, viewed, markViewed };
}

// --- Tests ---
beforeEach(() => {
  authReturn = { userId: 'u1', token: 'tok123' };
  settingsReturn = { theme: 'dark', locale: 'en' };
  analyticsTrack.reset();
});

group('useProfile with mocked dependencies', () => {
  test('returns auth and settings data', () => {
    const { current } = renderHook(() => useProfile());
    expect(current.userId).toBe('u1');
    expect(current.theme).toBe('dark');
    expect(current.locale).toBe('en');
  });

  test('different auth mock changes output', () => {
    authReturn = { userId: 'u2', token: 'other' };
    const { current } = renderHook(() => useProfile());
    expect(current.userId).toBe('u2');
    expect(current.token).toBe('other');
  });

  test('markViewed tracks analytics event', () => {
    const result = renderHook(() => useProfile());
    act(() => result.current.markViewed());
    expect(analyticsTrack).wasCalledOnce();
    expect(analyticsTrack).wasCalledWith('profile_viewed', { userId: 'u1' });
    expect(result.current.viewed).toBe(true);
  });

  test('settings mock changes theme', () => {
    settingsReturn = { theme: 'light', locale: 'da' };
    const { current } = renderHook(() => useProfile());
    expect(current.theme).toBe('light');
    expect(current.locale).toBe('da');
  });
});
