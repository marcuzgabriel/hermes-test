var Base = class {
  constructor(name) {
    this.name = name;
  }
  hello() {
    return "hello " + this.name;
  }
};
var Child = (function() {
  function Child() {
    return Reflect.construct(Base, Array.prototype.slice.call(arguments), new.target || Child);
  }
  Child.prototype = Object.create(Base.prototype, {
    constructor: { value: Child, writable: true, configurable: true }
  });
  Object.setPrototypeOf(Child, Base);
  Child.prototype.shout = function() {
    return this.hello().toUpperCase();
  };
  return Child;
})();
