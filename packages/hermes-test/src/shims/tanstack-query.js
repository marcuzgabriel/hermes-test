// hermes-test shim for @tanstack/react-query
//
// Transparent wrapper with test-friendly defaults. Ensures single-instance
// resolution and provides auto-cleanup tracking for QueryClient instances.
//
// Usage in hermes-test.config.json:
//   "shims": { "@tanstack/react-query": "hermes-test/shims/tanstack-query" }

var real = require('@__ht_real_pkg/@tanstack/react-query');

// Re-export everything unchanged
var mod = {};
var keys = Object.getOwnPropertyNames(real);
for (var i = 0; i < keys.length; i++) {
  if (keys[i] !== 'QueryClient') mod[keys[i]] = real[keys[i]];
}

// Track QueryClient instances for between-test cleanup
var _clients = [];
globalThis.__HT_queryClients = _clients;

var OrigQueryClient = real.QueryClient;

// Wrapper that applies test-friendly defaults
function QueryClient(opts) {
  var config = opts || {};
  var defaults = config.defaultOptions || {};
  var queries = defaults.queries || {};

  // Disable retries by default in tests (prevents timeouts)
  if (queries.retry === undefined) queries.retry = false;

  defaults.queries = queries;
  config.defaultOptions = defaults;

  var client = new OrigQueryClient(config);
  _clients.push(client);
  return client;
}

// Preserve prototype chain so instanceof checks work
QueryClient.prototype = OrigQueryClient.prototype;

mod.QueryClient = QueryClient;

// withQueryClient — test context factory (same pattern as withStore for Redux)
mod.withQueryClient = function withQueryClient(opts) {
  var config = opts || {};
  var client = new mod.QueryClient(config);
  var React = require('react');
  var QCP = mod.QueryClientProvider;

  var wrapper = function (props) {
    return React.createElement(QCP, { client: client }, props.children);
  };

  var renderHook = require('hermes-test').renderHook;

  return {
    queryClient: client,
    wrapper: wrapper,
    renderHookWithQuery: function (hookFn, hookOpts) {
      return renderHook(hookFn, Object.assign({}, hookOpts, { wrapper: wrapper }));
    },
  };
};

module.exports = mod;
