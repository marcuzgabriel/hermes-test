// Companion to relative-mock.test.ts: this file does NOT mock the flow module.
// It must get the REAL implementation even though a sibling test file mocks the
// same module in the same run (isolation must not leak across bundles).
import { test, expect } from 'hermes-test';
import { buildSession } from '../session';
import { getGreeting } from '../flows/greeting-flow';

test('unmocked file sees the real flow implementation', () => {
  expect(getGreeting()).toBe('real greeting');
  expect(buildSession()).toBe('session: real greeting');
});
