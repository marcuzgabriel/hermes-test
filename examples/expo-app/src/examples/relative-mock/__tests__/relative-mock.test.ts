// Regression: ht.mock() with a path relative to the TEST FILE must apply,
// even when the consumer imports the same module via a different relative
// specifier (here: session.ts uses './flows/greeting-flow', we use '../flows/...').
import { test, expect } from 'hermes-test';
import { buildSession } from '../session';

ht.mock('../flows/greeting-flow', () => ({
  getGreeting: () => 'mocked greeting',
}));

test('relative mock path resolves from the test file location', () => {
  expect(buildSession()).toBe('session: mocked greeting');
});
