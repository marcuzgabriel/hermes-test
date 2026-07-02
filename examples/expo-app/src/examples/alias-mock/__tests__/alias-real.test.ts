// Companion: does NOT mock — must get the real implementation in the same run.
import { test, expect } from 'hermes-test';
import { buildAliasSession } from '@app/alias-mock/consumer';
import { getServiceGreeting } from '@app/alias-mock/greeting-service';

test('unmocked file sees the real service through the alias', () => {
  expect(getServiceGreeting()).toBe('real service greeting');
  expect(buildAliasSession()).toBe('alias session: real service greeting');
});
