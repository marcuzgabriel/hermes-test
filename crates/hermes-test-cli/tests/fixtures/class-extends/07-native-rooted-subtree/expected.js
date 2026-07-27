var BaseError = (function() {
  function BaseError(msg) {
    var _this = Reflect.construct(Error, [msg], new.target || BaseError);
    _this.base = "B";
  
    return _this;
  }
  BaseError.prototype = Object.create(Error.prototype, {
    constructor: { value: BaseError, writable: true, configurable: true }
  });
  Object.setPrototypeOf(BaseError, Error);
  
  return BaseError;
})();
var ChildError = (function() {
  function ChildError(msg) {
    var _this = Reflect.construct(BaseError, [msg], new.target || ChildError);
    _this.child = "C";
  
    return _this;
  }
  ChildError.prototype = Object.create(BaseError.prototype, {
    constructor: { value: ChildError, writable: true, configurable: true }
  });
  Object.setPrototypeOf(ChildError, BaseError);
  
  return ChildError;
})();
var Plain = class {
  constructor() {
    this.kind = "plain";
  }
};
var UserChild = class extends Plain {
  constructor() {
    super();
    this.extra = 1;
  }
};
