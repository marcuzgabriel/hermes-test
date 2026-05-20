// Basic test file using the hermes-test harness (no Metro bundling)
// Uses globalThis.__metroTest since we can't use ES imports without Metro
var mt = globalThis.__metroTest;
var test = mt.test;

test('basic arithmetic', function(ctx) {
  ctx.expect(1 + 1).toBe(2);
  ctx.expect(10 - 3).toBe(7);
  ctx.expect(3 * 4).toBe(12);
});

test('deep equality', function(ctx) {
  ctx.expect({ a: 1, b: [2, 3] }).toEqual({ a: 1, b: [2, 3] });
  ctx.expect({ a: 1 }).not.toEqual({ a: 2 });
});

test('truthiness', function(ctx) {
  ctx.expect(true).toBeTruthy();
  ctx.expect(0).toBeFalsy();
  ctx.expect(null).toBeNull();
  ctx.expect('hello').toBeDefined();
});

test('comparisons', function(ctx) {
  ctx.expect(10).toBeGreaterThan(5);
  ctx.expect(3).toBeLessThan(10);
});

test('arrays', function(ctx) {
  ctx.expect([1, 2, 3]).toContain(2);
  ctx.expect('hello world').toMatch(/world/);
});

test('spy tracking', function(ctx) {
  var fn = ctx.spy(function(x) { return x * 2; });
  fn(5);
  fn(10);
  ctx.expect(fn).wasCalledTimes(2);
  ctx.expect(fn).wasCalledWith(5);
  ctx.expect(fn).wasLastCalledWith(10);
  ctx.expect(fn.returnValues).toEqual([10, 20]);
});

test('spy returns helper', function(ctx) {
  var fn = ctx.spy().returns(42);
  ctx.expect(fn()).toBe(42);
  ctx.expect(fn()).toBe(42);
  ctx.expect(fn).wasCalledTimes(2);
});

test('toThrow', function(ctx) {
  ctx.expect(function() { throw new Error('boom'); }).toThrow('boom');
  ctx.expect(function() {}).not.toThrow();
});
