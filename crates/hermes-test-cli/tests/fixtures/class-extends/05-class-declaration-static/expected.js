class Shape {
  constructor(kind) {
    this.kind = kind;
  }
}
class Circle extends Shape {
  constructor(radius) {
    super("circle");
    this.radius = radius;
  }
  area() {
    return 3 * this.radius * this.radius;
  }
  static unit() {
    return new Circle(1);
  }
}
