// Pattern: matchers added in 1.3.1
// Demonstrates: toHaveBeenNthCalledWith, toHaveBeenCalledOnce, toHaveProperty,
// toBeGreaterThanOrEqual, toBeLessThanOrEqual — and their .not forms.
import { test, group, expect, spy } from 'hermes-test';

group('extra matchers', () => {
  test('toHaveBeenNthCalledWith / toHaveBeenCalledOnce', () => {
    const fn = spy();
    fn('a', 1);
    expect(fn).toHaveBeenCalledOnce();
    fn('b', { x: [1, 2] });
    expect(fn).toHaveBeenNthCalledWith(1, 'a', 1);
    expect(fn).toHaveBeenNthCalledWith(2, 'b', { x: [1, 2] });
    expect(fn).not.toHaveBeenNthCalledWith(2, 'a', 1);
    expect(fn).not.toHaveBeenCalledOnce();
    expect(() => expect(fn).toHaveBeenNthCalledWith(3, 'c')).toThrow('called at least 3 times');
  });

  test('toHaveProperty with dotted paths, array paths and values', () => {
    const obj = { a: { b: { c: 42 } }, list: [{ id: 'x' }], nil: null };
    expect(obj).toHaveProperty('a.b.c');
    expect(obj).toHaveProperty('a.b.c', 42);
    expect(obj).toHaveProperty(['list', 0, 'id'], 'x');
    expect(obj).toHaveProperty('nil');
    expect(obj).not.toHaveProperty('a.b.d');
    expect(obj).not.toHaveProperty('a.b.c', 43);
    expect(() => expect(obj).toHaveProperty('zzz')).toThrow('to have property "zzz"');
  });

  test('toBeGreaterThanOrEqual / toBeLessThanOrEqual', () => {
    expect(5).toBeGreaterThanOrEqual(5);
    expect(6).toBeGreaterThanOrEqual(5);
    expect(5).toBeLessThanOrEqual(5);
    expect(4).toBeLessThanOrEqual(5);
    expect(4).not.toBeGreaterThanOrEqual(5);
    expect(6).not.toBeLessThanOrEqual(5);
  });
});
