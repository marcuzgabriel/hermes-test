const { test } = (globalThis as any).__HT;
import { unique, chunk, flatten, zip, sum, range, groupBy, compact, intersection, difference } from './array-utils';

test('unique removes duplicates', ({ expect }: any) => {
  expect(unique([1, 2, 2, 3, 3, 3])).toEqual([1, 2, 3]);
  expect(unique(['a', 'b', 'a'])).toEqual(['a', 'b']);
  expect(unique([])).toEqual([]);
});

test('chunk splits arrays', ({ expect }: any) => {
  expect(chunk([1, 2, 3, 4, 5], 2)).toEqual([[1, 2], [3, 4], [5]]);
  expect(chunk([1, 2, 3], 3)).toEqual([[1, 2, 3]]);
  expect(chunk([], 2)).toEqual([]);
});

test('flatten merges nested arrays', ({ expect }: any) => {
  expect(flatten([[1, 2], [3, 4]])).toEqual([1, 2, 3, 4]);
  expect(flatten([[], [1], []])).toEqual([1]);
});

test('zip pairs elements', ({ expect }: any) => {
  expect(zip([1, 2, 3], ['a', 'b', 'c'])).toEqual([[1, 'a'], [2, 'b'], [3, 'c']]);
  expect(zip([1, 2], ['a'])).toEqual([[1, 'a']]);
});

test('sum adds numbers', ({ expect }: any) => {
  expect(sum([1, 2, 3, 4])).toBe(10);
  expect(sum([])).toBe(0);
  expect(sum([-1, 1])).toBe(0);
});

test('range generates sequences', ({ expect }: any) => {
  expect(range(0, 5)).toEqual([0, 1, 2, 3, 4]);
  expect(range(3, 3)).toEqual([]);
  expect(range(-2, 2)).toEqual([-2, -1, 0, 1]);
});

test('groupBy categorizes items', ({ expect }: any) => {
  const result = groupBy([1, 2, 3, 4, 5], (n: number) => n % 2 === 0 ? 'even' : 'odd');
  expect(result['odd']).toEqual([1, 3, 5]);
  expect(result['even']).toEqual([2, 4]);
});

test('compact removes falsy values', ({ expect }: any) => {
  expect(compact([0, 1, false, 2, '', 3, null, undefined])).toEqual([1, 2, 3]);
  expect(compact([1, 2, 3])).toEqual([1, 2, 3]);
});

test('intersection finds common elements', ({ expect }: any) => {
  expect(intersection([1, 2, 3], [2, 3, 4])).toEqual([2, 3]);
  expect(intersection([1, 2], [3, 4])).toEqual([]);
});

test('difference finds unique to first', ({ expect }: any) => {
  expect(difference([1, 2, 3], [2, 3, 4])).toEqual([1]);
  expect(difference([1, 2], [1, 2])).toEqual([]);
});
