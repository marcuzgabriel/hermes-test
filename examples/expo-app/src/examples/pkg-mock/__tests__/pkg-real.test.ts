// Companion: does NOT mock — real moment must work in the same run.
import { test, expect } from 'hermes-test';
import { dateLabel } from '../date-label';

test('unmocked file uses the real package', () => {
  expect(dateLabel('2026-07-02')).toBe('label: 2026');
});
