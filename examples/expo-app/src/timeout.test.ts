import { test, expect } from 'hermes-test';

// Test 1: A fast test should pass normally (default 5000ms timeout)
test('fast test passes within default timeout', () => {
  expect(1 + 1).toBe(2);
});

// Test 2: A test with a very short timeout that does CPU-bound work should fail
test('sync busy loop exceeds short timeout', () => {
  // Spin for ~200ms - should exceed our 50ms timeout
  const end = Date.now() + 200;
  while (Date.now() < end) { /* busy wait */ }
  expect(true).toBe(true);
}, { timeout: 50 });

// Test 3: A test with a generous timeout should pass
test('fast test passes with explicit timeout', () => {
  expect('hello').toBe('hello');
}, { timeout: 10000 });
