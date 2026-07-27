var Stack = (function() {
  function Stack() {
    var _this = Reflect.construct(Array, [], new.target || Stack);
  
    return _this;
  }
  Stack.prototype = Object.create(Array.prototype, {
    constructor: { value: Stack, writable: true, configurable: true }
  });
  Object.setPrototypeOf(Stack, Array);
  Object.defineProperty(Stack, Symbol.species, { get: function() { return Stack; } });
  Stack.prototype.peek = function() {
    return this[this.length - 1];
  };
  return Stack;
})();
