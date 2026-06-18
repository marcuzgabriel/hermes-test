// Default react-native shim for hermes-test.
// Provides stub implementations of commonly used RN APIs.
// Users can override via hermes-test.config.json "shims" or mockModule().

var noop = function () {};
var noopReturn = function (x) {
  return x;
};

module.exports = {
  // Platform
  Platform: {
    OS: 'ios',
    Version: 19,
    select: function (obj) {
      return obj.ios !== undefined ? obj.ios : obj.default;
    },
    isPad: false,
    isTVOS: false,
    isTV: false,
  },

  // StyleSheet
  StyleSheet: {
    create: function (styles) {
      return styles;
    },
    flatten: function (style) {
      if (!style) return {};
      if (Array.isArray(style)) {
        var result = {};
        for (var i = 0; i < style.length; i++) {
          if (style[i]) Object.assign(result, style[i]);
        }
        return result;
      }
      return style;
    },
    absoluteFill: { position: 'absolute', left: 0, right: 0, top: 0, bottom: 0 },
    absoluteFillObject: { position: 'absolute', left: 0, right: 0, top: 0, bottom: 0 },
    hairlineWidth: 1,
  },

  // Dimensions
  Dimensions: {
    get: function () {
      return { width: 375, height: 812, scale: 3, fontScale: 1 };
    },
    addEventListener: function () {
      return { remove: noop };
    },
    removeEventListener: noop,
  },

  // PixelRatio
  PixelRatio: {
    get: function () {
      return 3;
    },
    getFontScale: function () {
      return 1;
    },
    getPixelSizeForLayoutSize: function (size) {
      return size * 3;
    },
    roundToNearestPixel: function (size) {
      return Math.round(size * 3) / 3;
    },
  },

  // AppState
  AppState: {
    currentState: 'active',
    addEventListener: function (event, cb) {
      return { remove: noop };
    },
    removeEventListener: noop,
  },

  // Alert
  Alert: {
    alert: noop,
    prompt: noop,
  },

  // Linking
  Linking: {
    openURL: function () {
      return Promise.resolve();
    },
    canOpenURL: function () {
      return Promise.resolve(true);
    },
    getInitialURL: function () {
      return Promise.resolve(null);
    },
    addEventListener: function () {
      return { remove: noop };
    },
    removeEventListener: noop,
  },

  // Keyboard
  Keyboard: {
    dismiss: noop,
    addListener: function () {
      return { remove: noop };
    },
    removeListener: noop,
    removeAllListeners: noop,
  },

  // Animated
  Animated: {
    Value: function (val) {
      this._value = val;
      this.setValue = function (v) {
        this._value = v;
      };
      this.interpolate = function () {
        return new module.exports.Animated.Value(0);
      };
      this.addListener = function () {
        return { remove: noop };
      };
      this.removeListener = noop;
      this.removeAllListeners = noop;
      this.stopAnimation = function (cb) {
        if (cb) cb(this._value);
      };
    },
    ValueXY: function () {
      this.x = new module.exports.Animated.Value(0);
      this.y = new module.exports.Animated.Value(0);
      this.setValue = noop;
      this.getLayout = function () {
        return { left: this.x, top: this.y };
      };
    },
    timing: function () {
      return {
        start: function (cb) {
          if (cb) cb({ finished: true });
        },
      };
    },
    spring: function () {
      return {
        start: function (cb) {
          if (cb) cb({ finished: true });
        },
      };
    },
    decay: function () {
      return {
        start: function (cb) {
          if (cb) cb({ finished: true });
        },
      };
    },
    parallel: function () {
      return {
        start: function (cb) {
          if (cb) cb({ finished: true });
        },
      };
    },
    sequence: function () {
      return {
        start: function (cb) {
          if (cb) cb({ finished: true });
        },
      };
    },
    stagger: function () {
      return {
        start: function (cb) {
          if (cb) cb({ finished: true });
        },
      };
    },
    loop: function () {
      return { start: noop, stop: noop };
    },
    event: function () {
      return noop;
    },
    createAnimatedComponent: noopReturn,
    View: 'Animated.View',
    Text: 'Animated.Text',
    Image: 'Animated.Image',
    ScrollView: 'Animated.ScrollView',
    FlatList: 'Animated.FlatList',
  },

  // Hooks
  useWindowDimensions: function () {
    return { width: 375, height: 812, scale: 3, fontScale: 1 };
  },
  useColorScheme: function () {
    return 'light';
  },

  // Components (string stubs — not rendered in test)
  View: 'View',
  Text: 'Text',
  Image: 'Image',
  ScrollView: 'ScrollView',
  FlatList: 'FlatList',
  SectionList: 'SectionList',
  TouchableOpacity: 'TouchableOpacity',
  TouchableHighlight: 'TouchableHighlight',
  TouchableWithoutFeedback: 'TouchableWithoutFeedback',
  Pressable: 'Pressable',
  TextInput: 'TextInput',
  Switch: 'Switch',
  ActivityIndicator: 'ActivityIndicator',
  Modal: 'Modal',
  SafeAreaView: 'SafeAreaView',
  StatusBar: 'StatusBar',
  KeyboardAvoidingView: 'KeyboardAvoidingView',

  // I18nManager
  I18nManager: {
    isRTL: false,
    allowRTL: noop,
    forceRTL: noop,
  },

  // Appearance
  Appearance: {
    getColorScheme: function () {
      return 'light';
    },
    addChangeListener: function () {
      return { remove: noop };
    },
  },

  // NativeModules fallback
  NativeModules: {},
  NativeEventEmitter: function () {
    this.addListener = function () {
      return { remove: noop };
    };
    this.removeListener = noop;
    this.removeAllListeners = noop;
  },

  // AccessibilityInfo
  AccessibilityInfo: {
    isScreenReaderEnabled: function () {
      return Promise.resolve(false);
    },
    addEventListener: function () {
      return { remove: noop };
    },
    announceForAccessibility: noop,
  },
};
