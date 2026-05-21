import { test, expect, spy } from 'hermes-test';
import { capitalize, slugify, truncate } from './string-utils';

test('capitalize', () => {
  expect(capitalize('hello')).toBe('Hello');
  expect(capitalize('world')).toBe('World');
  expect(capitalize('')).toBe('');
});

test('slugify', () => {
  expect(slugify('Hello World')).toBe('hello-world');
  expect(slugify('  Multiple   Spaces  ')).toBe('multiple-spaces');
  expect(slugify('already-slug')).toBe('already-slug');
});

test('truncate', () => {
  expect(truncate('short', 10)).toBe('short');
  expect(truncate('this is a long string', 10)).toBe('this is...');
  expect(truncate('exactly10c', 10)).toBe('exactly10c');
});

test('spy with string utils', () => {
  const transform = spy((s: string) => capitalize(slugify(s)));
  transform('Hello World');
  transform('Another Test');
  expect(transform).wasCalledTimes(2);
  expect(transform.returnValues[0]).toBe('Hello-world');
});
