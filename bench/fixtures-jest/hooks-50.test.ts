import { renderHook, act } from '@testing-library/react-hooks';
import { useCounter } from './useCounter';

test('hook test 0: counter from 0', () => {
  const { result } = renderHook(() => useCounter(0));
  expect(result.current.count).toBe(0);
  act(() => result.current.increment());
  expect(result.current.count).toBe(1);
});
test('hook test 1: counter from 1', () => {
  const { result } = renderHook(() => useCounter(1));
  expect(result.current.count).toBe(1);
  act(() => result.current.increment());
  expect(result.current.count).toBe(2);
});
test('hook test 2: counter from 2', () => {
  const { result } = renderHook(() => useCounter(2));
  expect(result.current.count).toBe(2);
  act(() => result.current.increment());
  expect(result.current.count).toBe(3);
});
test('hook test 3: counter from 3', () => {
  const { result } = renderHook(() => useCounter(3));
  expect(result.current.count).toBe(3);
  act(() => result.current.increment());
  expect(result.current.count).toBe(4);
});
test('hook test 4: counter from 4', () => {
  const { result } = renderHook(() => useCounter(4));
  expect(result.current.count).toBe(4);
  act(() => result.current.increment());
  expect(result.current.count).toBe(5);
});
test('hook test 5: counter from 5', () => {
  const { result } = renderHook(() => useCounter(5));
  expect(result.current.count).toBe(5);
  act(() => result.current.increment());
  expect(result.current.count).toBe(6);
});
test('hook test 6: counter from 6', () => {
  const { result } = renderHook(() => useCounter(6));
  expect(result.current.count).toBe(6);
  act(() => result.current.increment());
  expect(result.current.count).toBe(7);
});
test('hook test 7: counter from 7', () => {
  const { result } = renderHook(() => useCounter(7));
  expect(result.current.count).toBe(7);
  act(() => result.current.increment());
  expect(result.current.count).toBe(8);
});
test('hook test 8: counter from 8', () => {
  const { result } = renderHook(() => useCounter(8));
  expect(result.current.count).toBe(8);
  act(() => result.current.increment());
  expect(result.current.count).toBe(9);
});
test('hook test 9: counter from 9', () => {
  const { result } = renderHook(() => useCounter(9));
  expect(result.current.count).toBe(9);
  act(() => result.current.increment());
  expect(result.current.count).toBe(10);
});
test('hook test 10: counter from 10', () => {
  const { result } = renderHook(() => useCounter(10));
  expect(result.current.count).toBe(10);
  act(() => result.current.increment());
  expect(result.current.count).toBe(11);
});
test('hook test 11: counter from 11', () => {
  const { result } = renderHook(() => useCounter(11));
  expect(result.current.count).toBe(11);
  act(() => result.current.increment());
  expect(result.current.count).toBe(12);
});
test('hook test 12: counter from 12', () => {
  const { result } = renderHook(() => useCounter(12));
  expect(result.current.count).toBe(12);
  act(() => result.current.increment());
  expect(result.current.count).toBe(13);
});
test('hook test 13: counter from 13', () => {
  const { result } = renderHook(() => useCounter(13));
  expect(result.current.count).toBe(13);
  act(() => result.current.increment());
  expect(result.current.count).toBe(14);
});
test('hook test 14: counter from 14', () => {
  const { result } = renderHook(() => useCounter(14));
  expect(result.current.count).toBe(14);
  act(() => result.current.increment());
  expect(result.current.count).toBe(15);
});
test('hook test 15: counter from 15', () => {
  const { result } = renderHook(() => useCounter(15));
  expect(result.current.count).toBe(15);
  act(() => result.current.increment());
  expect(result.current.count).toBe(16);
});
test('hook test 16: counter from 16', () => {
  const { result } = renderHook(() => useCounter(16));
  expect(result.current.count).toBe(16);
  act(() => result.current.increment());
  expect(result.current.count).toBe(17);
});
test('hook test 17: counter from 17', () => {
  const { result } = renderHook(() => useCounter(17));
  expect(result.current.count).toBe(17);
  act(() => result.current.increment());
  expect(result.current.count).toBe(18);
});
test('hook test 18: counter from 18', () => {
  const { result } = renderHook(() => useCounter(18));
  expect(result.current.count).toBe(18);
  act(() => result.current.increment());
  expect(result.current.count).toBe(19);
});
test('hook test 19: counter from 19', () => {
  const { result } = renderHook(() => useCounter(19));
  expect(result.current.count).toBe(19);
  act(() => result.current.increment());
  expect(result.current.count).toBe(20);
});
test('hook test 20: counter from 20', () => {
  const { result } = renderHook(() => useCounter(20));
  expect(result.current.count).toBe(20);
  act(() => result.current.increment());
  expect(result.current.count).toBe(21);
});
test('hook test 21: counter from 21', () => {
  const { result } = renderHook(() => useCounter(21));
  expect(result.current.count).toBe(21);
  act(() => result.current.increment());
  expect(result.current.count).toBe(22);
});
test('hook test 22: counter from 22', () => {
  const { result } = renderHook(() => useCounter(22));
  expect(result.current.count).toBe(22);
  act(() => result.current.increment());
  expect(result.current.count).toBe(23);
});
test('hook test 23: counter from 23', () => {
  const { result } = renderHook(() => useCounter(23));
  expect(result.current.count).toBe(23);
  act(() => result.current.increment());
  expect(result.current.count).toBe(24);
});
test('hook test 24: counter from 24', () => {
  const { result } = renderHook(() => useCounter(24));
  expect(result.current.count).toBe(24);
  act(() => result.current.increment());
  expect(result.current.count).toBe(25);
});
test('hook test 25: counter from 25', () => {
  const { result } = renderHook(() => useCounter(25));
  expect(result.current.count).toBe(25);
  act(() => result.current.increment());
  expect(result.current.count).toBe(26);
});
test('hook test 26: counter from 26', () => {
  const { result } = renderHook(() => useCounter(26));
  expect(result.current.count).toBe(26);
  act(() => result.current.increment());
  expect(result.current.count).toBe(27);
});
test('hook test 27: counter from 27', () => {
  const { result } = renderHook(() => useCounter(27));
  expect(result.current.count).toBe(27);
  act(() => result.current.increment());
  expect(result.current.count).toBe(28);
});
test('hook test 28: counter from 28', () => {
  const { result } = renderHook(() => useCounter(28));
  expect(result.current.count).toBe(28);
  act(() => result.current.increment());
  expect(result.current.count).toBe(29);
});
test('hook test 29: counter from 29', () => {
  const { result } = renderHook(() => useCounter(29));
  expect(result.current.count).toBe(29);
  act(() => result.current.increment());
  expect(result.current.count).toBe(30);
});
test('hook test 30: counter from 30', () => {
  const { result } = renderHook(() => useCounter(30));
  expect(result.current.count).toBe(30);
  act(() => result.current.increment());
  expect(result.current.count).toBe(31);
});
test('hook test 31: counter from 31', () => {
  const { result } = renderHook(() => useCounter(31));
  expect(result.current.count).toBe(31);
  act(() => result.current.increment());
  expect(result.current.count).toBe(32);
});
test('hook test 32: counter from 32', () => {
  const { result } = renderHook(() => useCounter(32));
  expect(result.current.count).toBe(32);
  act(() => result.current.increment());
  expect(result.current.count).toBe(33);
});
test('hook test 33: counter from 33', () => {
  const { result } = renderHook(() => useCounter(33));
  expect(result.current.count).toBe(33);
  act(() => result.current.increment());
  expect(result.current.count).toBe(34);
});
test('hook test 34: counter from 34', () => {
  const { result } = renderHook(() => useCounter(34));
  expect(result.current.count).toBe(34);
  act(() => result.current.increment());
  expect(result.current.count).toBe(35);
});
test('hook test 35: counter from 35', () => {
  const { result } = renderHook(() => useCounter(35));
  expect(result.current.count).toBe(35);
  act(() => result.current.increment());
  expect(result.current.count).toBe(36);
});
test('hook test 36: counter from 36', () => {
  const { result } = renderHook(() => useCounter(36));
  expect(result.current.count).toBe(36);
  act(() => result.current.increment());
  expect(result.current.count).toBe(37);
});
test('hook test 37: counter from 37', () => {
  const { result } = renderHook(() => useCounter(37));
  expect(result.current.count).toBe(37);
  act(() => result.current.increment());
  expect(result.current.count).toBe(38);
});
test('hook test 38: counter from 38', () => {
  const { result } = renderHook(() => useCounter(38));
  expect(result.current.count).toBe(38);
  act(() => result.current.increment());
  expect(result.current.count).toBe(39);
});
test('hook test 39: counter from 39', () => {
  const { result } = renderHook(() => useCounter(39));
  expect(result.current.count).toBe(39);
  act(() => result.current.increment());
  expect(result.current.count).toBe(40);
});
test('hook test 40: counter from 40', () => {
  const { result } = renderHook(() => useCounter(40));
  expect(result.current.count).toBe(40);
  act(() => result.current.increment());
  expect(result.current.count).toBe(41);
});
test('hook test 41: counter from 41', () => {
  const { result } = renderHook(() => useCounter(41));
  expect(result.current.count).toBe(41);
  act(() => result.current.increment());
  expect(result.current.count).toBe(42);
});
test('hook test 42: counter from 42', () => {
  const { result } = renderHook(() => useCounter(42));
  expect(result.current.count).toBe(42);
  act(() => result.current.increment());
  expect(result.current.count).toBe(43);
});
test('hook test 43: counter from 43', () => {
  const { result } = renderHook(() => useCounter(43));
  expect(result.current.count).toBe(43);
  act(() => result.current.increment());
  expect(result.current.count).toBe(44);
});
test('hook test 44: counter from 44', () => {
  const { result } = renderHook(() => useCounter(44));
  expect(result.current.count).toBe(44);
  act(() => result.current.increment());
  expect(result.current.count).toBe(45);
});
test('hook test 45: counter from 45', () => {
  const { result } = renderHook(() => useCounter(45));
  expect(result.current.count).toBe(45);
  act(() => result.current.increment());
  expect(result.current.count).toBe(46);
});
test('hook test 46: counter from 46', () => {
  const { result } = renderHook(() => useCounter(46));
  expect(result.current.count).toBe(46);
  act(() => result.current.increment());
  expect(result.current.count).toBe(47);
});
test('hook test 47: counter from 47', () => {
  const { result } = renderHook(() => useCounter(47));
  expect(result.current.count).toBe(47);
  act(() => result.current.increment());
  expect(result.current.count).toBe(48);
});
test('hook test 48: counter from 48', () => {
  const { result } = renderHook(() => useCounter(48));
  expect(result.current.count).toBe(48);
  act(() => result.current.increment());
  expect(result.current.count).toBe(49);
});
test('hook test 49: counter from 49', () => {
  const { result } = renderHook(() => useCounter(49));
  expect(result.current.count).toBe(49);
  act(() => result.current.increment());
  expect(result.current.count).toBe(50);
});
