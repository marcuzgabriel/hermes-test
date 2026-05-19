// trivial.test.js — hand-bundled test for Week 1
// No harness, no Metro — just raw JS evaluated by Hermes
(function() {
  var results = [];

  // Test 1: basic arithmetic
  var sum = 1 + 1;
  results.push({
    name: 'basic arithmetic',
    status: sum === 2 ? 'pass' : 'fail'
  });

  // Test 2: string operations
  var greeting = 'hello' + ' ' + 'world';
  results.push({
    name: 'string concatenation',
    status: greeting === 'hello world' ? 'pass' : 'fail'
  });

  // Test 3: object spread (Hermes ES2018+)
  var a = { x: 1 };
  var b = Object.assign({}, a, { y: 2 });
  results.push({
    name: 'object spread',
    status: (b.x === 1 && b.y === 2) ? 'pass' : 'fail'
  });

  // Test 4: Array.prototype.includes
  results.push({
    name: 'array includes',
    status: [1, 2, 3].includes(2) ? 'pass' : 'fail'
  });

  // Test 5: Promise exists (Hermes supports it)
  results.push({
    name: 'Promise exists',
    status: typeof Promise === 'function' ? 'pass' : 'fail'
  });

  return { tests: results, passed: results.filter(function(t) { return t.status === 'pass'; }).length, total: results.length };
})()
