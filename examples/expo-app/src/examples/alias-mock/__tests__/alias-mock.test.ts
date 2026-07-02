// Alias-path mock: must apply to the consumer's aliased import; the sibling
// alias-real.test.ts must still see the real module in the same run.
import { test, expect } from 'hermes-test';
import { buildAliasSession } from '@app/alias-mock/consumer';

ht.mock('@app/alias-mock/greeting-service', () => ({
  getServiceGreeting: () => 'mocked service greeting',
}));

test('alias mock applies through the aliased import', () => {
  expect(buildAliasSession()).toBe('alias session: mocked service greeting');
});
