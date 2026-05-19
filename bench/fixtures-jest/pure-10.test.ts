import { add, sub, mul, div, clamp, isEven, isPrime, reverse, capitalize, unique } from './bench-helpers';

test('add', () => { expect(add(2, 3)).toBe(5); expect(add(-1, 1)).toBe(0); });
test('sub', () => { expect(sub(10, 3)).toBe(7); expect(sub(0, 0)).toBe(0); });
test('mul', () => { expect(mul(3, 4)).toBe(12); expect(mul(0, 5)).toBe(0); });
test('div', () => { expect(div(10, 2)).toBe(5); expect(div(1, 0)).toBe(0); });
test('clamp', () => { expect(clamp(5, 0, 10)).toBe(5); expect(clamp(-1, 0, 10)).toBe(0); expect(clamp(15, 0, 10)).toBe(10); });
test('isEven', () => { expect(isEven(4)).toBe(true); expect(isEven(3)).toBe(false); });
test('isPrime', () => { expect(isPrime(2)).toBe(true); expect(isPrime(4)).toBe(false); expect(isPrime(97)).toBe(true); });
test('reverse', () => { expect(reverse('hello')).toBe('olleh'); expect(reverse('')).toBe(''); });
test('capitalize', () => { expect(capitalize('hello')).toBe('Hello'); expect(capitalize('')).toBe(''); });
test('unique', () => { expect(unique([1, 2, 2, 3])).toEqual([1, 2, 3]); expect(unique([])).toEqual([]); });
