const { test, expect } = (globalThis as any).__HT;
import { add, multiply, fibonacci, isPrime } from './math';

test('add returns sum', ({ expect }: any) => {
  expect(add(2, 3)).toBe(5);
  expect(add(-1, 1)).toBe(0);
  expect(add(0, 0)).toBe(0);
});

test('multiply returns product', ({ expect }: any) => {
  expect(multiply(3, 4)).toBe(12);
  expect(multiply(0, 100)).toBe(0);
  expect(multiply(-2, 3)).toBe(-6);
});

test('fibonacci sequence', ({ expect }: any) => {
  expect(fibonacci(0)).toBe(0);
  expect(fibonacci(1)).toBe(1);
  expect(fibonacci(10)).toBe(55);
  expect(fibonacci(20)).toBe(6765);
});

test('isPrime', ({ expect }: any) => {
  expect(isPrime(2)).toBe(true);
  expect(isPrime(17)).toBe(true);
  expect(isPrime(1)).toBe(false);
  expect(isPrime(4)).toBe(false);
  expect(isPrime(97)).toBe(true);
});
