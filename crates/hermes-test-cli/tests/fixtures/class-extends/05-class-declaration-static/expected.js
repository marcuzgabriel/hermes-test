class Shape {
  constructor(kind) {
    this.kind = kind;
  }
}
var Circle = (function() {
  function Circle(radius) {
    var _this = Reflect.construct(Shape, ["circle"], new.target || Circle);
    _this.radius = radius;
  
    return _this;
  }
  Circle.prototype = Object.create(Shape.prototype, {
    constructor: { value: Circle, writable: true, configurable: true }
  });
  Object.setPrototypeOf(Circle, Shape);
  Circle.prototype.area = function() {
    return 3 * this.radius * this.radius;
  };
Circle.unit = function() {
    return new Circle(1);
  };
  return Circle;
})();
