// npm package mock: must apply to the consumer; pkg-real.test.ts must still
// get the real package in the same run.
import { test, expect } from 'hermes-test';
import { dateLabel } from '../date-label';

ht.mock('moment', () => ({
  default: () => ({ format: () => 'MOCKED' }),
}));

test('package mock applies to the consumer', () => {
  expect(dateLabel('2026-07-02')).toBe('label: MOCKED');
});
