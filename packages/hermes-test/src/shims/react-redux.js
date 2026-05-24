// hermes-test shim for react-redux
//
// Transparent wrapper that ensures single-instance resolution.
// Re-exports Provider, useSelector, useDispatch, connect, etc.
// unchanged. Prevents dual-instance issues where React context
// from one copy can't be read by hooks from another copy.
//
// Usage in hermes-test.config.json:
//   "shims": { "react-redux": "hermes-test/shims/react-redux" }

var real = require('@__ht_real_pkg/react-redux');
module.exports = real;
