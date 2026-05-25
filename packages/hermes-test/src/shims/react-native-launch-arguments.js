// Built-in react-native-launch-arguments shim for hermes-test.
// Returns empty launch arguments — native module not available in test environment.

module.exports = {
  LaunchArguments: {
    value: function() { return {}; },
  },
};
