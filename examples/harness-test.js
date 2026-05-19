// Test the harness running in Hermes (no Metro yet)
var mt = globalThis.__metroTest;

mt.test('basic math', function(ctx) {
  ctx.expect(1 + 1).toBe(2);
  ctx.expect(3 * 4).toBe(12);
});

mt.test('deep equality', function(ctx) {
  ctx.expect({ a: 1, b: [2, 3] }).toEqual({ a: 1, b: [2, 3] });
  ctx.expect([1, 2, 3]).toContain(2);
});

mt.test('negation', function(ctx) {
  ctx.expect(1).not.toBe(2);
  ctx.expect(undefined).not.toBeDefined();
  ctx.expect('hello').not.toMatch(/xyz/);
});

mt.test('spy basics', function(ctx) {
  var fn = ctx.spy();
  fn('a');
  fn('b', 'c');
  ctx.expect(fn).wasCalledTimes(2);
  ctx.expect(fn).wasCalledWith('a');
  ctx.expect(fn).wasLastCalledWith('b', 'c');
});

mt.test('spy returns', function(ctx) {
  var fn = ctx.spy(function(x) { return x * 2; });
  var result = fn(5);
  ctx.expect(result).toBe(10);
  ctx.expect(fn.returnValues[0]).toBe(10);
});

mt.test('toThrow', function(ctx) {
  ctx.expect(function() { throw new Error('boom'); }).toThrow('boom');
  ctx.expect(function() { throw new Error('boom'); }).toThrow(/boo/);
  ctx.expect(function() {}).not.toThrow();
});

mt.test('intentional failure', function(ctx) {
  ctx.expect(1).toBe(2);
});

mt.test('skip me', function(ctx) {
  ctx.expect(true).toBe(false);
}, { skip: true });

// Run and output
var results = mt.runTests();
JSON.stringify({
  tests: results,
  passed: results.filter(function(t) { return t.status === 'pass'; }).length,
  failed: results.filter(function(t) { return t.status === 'fail'; }).length,
  skipped: results.filter(function(t) { return t.status === 'skip'; }).length,
  total: results.length
});
