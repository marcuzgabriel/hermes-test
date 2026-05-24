// hermes-test shim for @reduxjs/toolkit
//
// Transparent wrapper that ensures single-instance resolution.
// Re-exports everything unchanged — createSlice, configureStore,
// createAsyncThunk, createSelector, etc. all work as normal.
//
// Usage in hermes-test.config.json:
//   "shims": { "@reduxjs/toolkit": "hermes-test/shims/reduxjs-toolkit" }

var real = require('@__ht_real_pkg/@reduxjs/toolkit');
module.exports = real;
