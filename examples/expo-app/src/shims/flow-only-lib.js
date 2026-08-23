// User shim for the Flow-syntax example package. Registered in hermes-test.config.json under
// "shims" — which both externalizes the real package (esbuild never parses it) and serves this
// object to every importer at runtime.
module.exports = {
  __esModule: true,
  default: function FlowOnlyThingShim() {
    return 'shimmed-flow-only-lib';
  },
};
