var Logger = class {
  constructor(prefix) {
    this.prefix = prefix;
  }
  log(msg) {
    return this.prefix + ": " + msg;
  }
};
var TaggedLogger = (function() {
  function TaggedLogger(prefix, tag) {
    var _this = Reflect.construct(Logger, [prefix], new.target || TaggedLogger);
    _this.tag = tag;
  
    return _this;
  }
  TaggedLogger.prototype = Object.create(Logger.prototype, {
    constructor: { value: TaggedLogger, writable: true, configurable: true }
  });
  Object.setPrototypeOf(TaggedLogger, Logger);
  TaggedLogger.prototype.tagged = function(msg) {
    return "[" + this.tag + "] " + this.prefix + ": " + msg;
  };
TaggedLogger.create = function(tag) {
    return new TaggedLogger("app", tag);
  };
  return TaggedLogger;
})();
