// Pattern: Spy and assertion patterns — full API coverage
// Demonstrates: spy, spyOn, mockReturnValue, mockImplementation, clearAllMocks
import { test, group, spy, spyOn, clearAllMocks, expect } from 'hermes-test';

group('spy basics', () => {
  test('tracks calls and arguments', () => {
    const fn = spy((a: number, b: number) => a + b);
    fn(1, 2);
    fn(3, 4);
    expect(fn).wasCalledTimes(2);
    expect(fn).wasCalledWith(1, 2);
    expect(fn.returnValues).toEqual([3, 7]);
  });

  test('mockReturnValue overrides return', () => {
    const fn = spy(() => 'original');
    fn.mockReturnValue('mocked');
    expect(fn()).toBe('mocked');
    expect(fn()).toBe('mocked');
  });

  test('mockReturnValueOnce queues returns', () => {
    const fn = spy(() => 'default');
    fn.mockReturnValueOnce('first').mockReturnValueOnce('second');
    expect(fn()).toBe('first');
    expect(fn()).toBe('second');
    expect(fn()).toBe('default'); // falls back
  });

  test('mockImplementation replaces logic', () => {
    const fn = spy((_x: number) => 0);
    fn.mockImplementation((x: number) => x * 10);
    expect(fn(3)).toBe(30);
    expect(fn(5)).toBe(50);
  });

  test('mockImplementationOnce queues implementations', () => {
    const fn = spy((x: number) => x);
    fn.mockImplementationOnce((x: number) => x * 100);
    expect(fn(2)).toBe(200);
    expect(fn(2)).toBe(2); // falls back to original
  });

  test('reset clears all tracking', () => {
    const fn = spy(() => 42);
    fn(); fn(); fn();
    expect(fn).wasCalledTimes(3);
    fn.reset();
    expect(fn).wasCalledTimes(0);
    expect(fn.returnValues).toEqual([]);
  });
});

group('spyOn', () => {
  test('spies on object methods', () => {
    const calculator = { add: (a: number, b: number) => a + b };
    const addSpy = spyOn(calculator, 'add');
    calculator.add(2, 3);
    expect(addSpy).wasCalledOnce();
    expect(addSpy).wasCalledWith(2, 3);
    expect(addSpy.returnValues[0]).toBe(5);
  });
});

group('clearAllMocks', () => {
  test('resets all active spies', () => {
    const a = spy(() => 1);
    const b = spy(() => 2);
    a(); b(); b();
    expect(a).wasCalledTimes(1);
    expect(b).wasCalledTimes(2);
    clearAllMocks();
    expect(a).wasCalledTimes(0);
    expect(b).wasCalledTimes(0);
  });
});

group('assertion matchers', () => {
  test('wasCalled and wasCalledOnce', () => {
    const fn = spy(() => {});
    expect(fn).wasCalledTimes(0);
    fn();
    expect(fn).wasCalled();
    expect(fn).wasCalledOnce();
  });

  test('wasCalledWith checks arguments', () => {
    const fn = spy((_a: string, _b: number) => {});
    fn('hello', 42);
    expect(fn).wasCalledWith('hello', 42);
  });
});
