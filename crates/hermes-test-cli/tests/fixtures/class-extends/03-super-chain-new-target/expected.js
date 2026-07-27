var A = class {
  constructor() {
    this.trail = ["A"];
  }
};
var B = (function() {
  function B() {
    return Reflect.construct(A, Array.prototype.slice.call(arguments), new.target || B);
  }
  B.prototype = Object.create(A.prototype, {
    constructor: { value: B, writable: true, configurable: true }
  });
  Object.setPrototypeOf(B, A);
  
  return B;
})();
var C = (function() {
  function C() {
    var _this = Reflect.construct(B, [], new.target || C);
    _this.trail.push("C");
  
    return _this;
  }
  C.prototype = Object.create(B.prototype, {
    constructor: { value: C, writable: true, configurable: true }
  });
  Object.setPrototypeOf(C, B);
  
  return C;
})();
var D = (function() {
  function D() {
    return Reflect.construct(C, Array.prototype.slice.call(arguments), new.target || D);
  }
  D.prototype = Object.create(C.prototype, {
    constructor: { value: D, writable: true, configurable: true }
  });
  Object.setPrototypeOf(D, C);
  
  return D;
})();
