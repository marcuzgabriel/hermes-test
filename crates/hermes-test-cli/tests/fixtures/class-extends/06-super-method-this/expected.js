var Base = class {
  constructor(name) {
    this.name = name;
  }
  describe() {
    return "base:" + this.name;
  }
};
var Sub = (function() {
  function Sub(name) {
    var _this = Reflect.construct(Base, [name], new.target || Sub);
    _this.initial = Base.prototype.describe.call(_this);
  
    return _this;
  }
  Sub.prototype = Object.create(Base.prototype, {
    constructor: { value: Sub, writable: true, configurable: true }
  });
  Object.setPrototypeOf(Sub, Base);
  Sub.prototype.describe = function() {
    return "sub(" + Base.prototype.describe.call(this) + ")";
  };
  return Sub;
})();
