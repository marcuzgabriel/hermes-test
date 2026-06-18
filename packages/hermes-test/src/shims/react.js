// Shim for react inside the harness bundle.
// react-reconciler imports 'react' — this shim delegates to globalThis.__HT_React.
// The Proxy ensures late-binding: properties are read at access time, not import time.
var handler = {
  get: function (_, prop) {
    if (prop === Symbol.toPrimitive || prop === 'then') return undefined;
    var R = globalThis.__HT_React;
    return R ? R[prop] : undefined;
  },
  set: function (_, prop, value) {
    var R = globalThis.__HT_React;
    if (R) R[prop] = value;
    return true;
  },
};
module.exports = new Proxy({}, handler);
