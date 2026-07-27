var Logger = class {
  constructor(prefix) {
    this.prefix = prefix;
  }
  log(msg) {
    return this.prefix + ": " + msg;
  }
};
var TaggedLogger = class _TaggedLogger extends Logger {
  constructor(prefix, tag) {
    super(prefix);
    this.tag = tag;
  }
  tagged(msg) {
    return "[" + this.tag + "] " + this.prefix + ": " + msg;
  }
  static create(tag) {
    return new _TaggedLogger("app", tag);
  }
};
