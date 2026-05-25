// Pattern: Fake timers for debounce and polling
// Demonstrates: useFakeTimers, advanceTimersByTime, useRealTimers
import { test, group, beforeEach, afterEach, renderHook, act, useFakeTimers, advanceTimersByTime, useRealTimers, spy, expect } from 'hermes-test';
import { useState, useEffect, useRef, useCallback } from 'react';

// --- Hooks under test ---
function useDebounce<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);
  return debounced;
}

function usePolling(callback: () => void, intervalMs: number) {
  const cbRef = useRef(callback);
  cbRef.current = callback;
  const [active, setActive] = useState(true);

  useEffect(() => {
    if (!active) return;
    const id = setInterval(() => cbRef.current(), intervalMs);
    return () => clearInterval(id);
  }, [intervalMs, active]);

  return { stop: useCallback(() => setActive(false), []) };
}

// --- Tests ---
beforeEach(() => { useFakeTimers(); });
afterEach(() => { useRealTimers(); });

group('useDebounce', () => {
  test('does not update before delay', () => {
    const result = renderHook(
      ({ value }) => useDebounce(value, 300),
      { initialProps: { value: 'a' } },
    );
    act(() => result.rerender({ value: 'b' }));
    act(() => advanceTimersByTime(100));
    expect(result.current).toBe('a'); // not yet updated
  });

  test('updates after delay', () => {
    const result = renderHook(
      ({ value }) => useDebounce(value, 300),
      { initialProps: { value: 'a' } },
    );
    act(() => result.rerender({ value: 'b' }));
    act(() => advanceTimersByTime(300));
    expect(result.current).toBe('b');
  });

  test('resets timer on rapid changes', () => {
    const result = renderHook(
      ({ value }) => useDebounce(value, 300),
      { initialProps: { value: 'a' } },
    );
    act(() => result.rerender({ value: 'b' }));
    act(() => advanceTimersByTime(200));
    act(() => result.rerender({ value: 'c' }));
    act(() => advanceTimersByTime(200));
    expect(result.current).toBe('a'); // still waiting
    act(() => advanceTimersByTime(100));
    expect(result.current).toBe('c');
  });
});

group('usePolling', () => {
  test('fires callback at interval', () => {
    const cb = spy(() => {});
    renderHook(() => usePolling(cb, 1000));
    advanceTimersByTime(3000);
    expect(cb).wasCalledTimes(3);
  });

  test('stop prevents further calls', () => {
    const cb = spy(() => {});
    const result = renderHook(() => usePolling(cb, 1000));
    advanceTimersByTime(2000);
    expect(cb).wasCalledTimes(2);
    act(() => result.current.stop());
    advanceTimersByTime(3000);
    expect(cb).wasCalledTimes(2); // no more calls
  });
});
