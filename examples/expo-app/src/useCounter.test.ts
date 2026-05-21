import { test, expect, renderHook, act } from 'hermes-test';
import { useCounter } from './useCounter';

test('useCounter starts at initial value', () => {
  const { current, renderCount } = renderHook(() => useCounter(5));
  expect(current.count).toBe(5);
  expect(renderCount).toBe(1);
});

test('useCounter increments', () => {
  const result = renderHook(() => useCounter(0));

  act(() => result.current.increment());

  expect(result.current.count).toBe(1);
  expect(result.renderCount).toBe(2);
});

test('useCounter tracks state history', () => {
  const result = renderHook(() => useCounter(0));

  act(() => result.current.increment());
  act(() => result.current.increment());
  act(() => result.current.decrement());

  expect(result.current.count).toBe(1);
  expect(result.history.map((h: any) => h.count)).toEqual([0, 1, 2, 1]);
  expect(result.renderCount).toBe(4);
});

test('useCounter resets', () => {
  const result = renderHook(() => useCounter(10));

  act(() => result.current.increment());
  act(() => result.current.increment());
  act(() => result.current.reset());

  expect(result.current.count).toBe(10);
});
