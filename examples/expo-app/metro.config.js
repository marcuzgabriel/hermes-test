const { getDefaultConfig } = require('metro-config');

module.exports = (async () => {
  const config = await getDefaultConfig(__dirname);
  return {
    ...config,
    transformer: {
      ...config.transformer,
      babelTransformerPath: require.resolve('@react-native/metro-babel-transformer'),
    },
    // Prevent worker explosion — keep it single-threaded for test bundling
    maxWorkers: 1,
  };
})();
