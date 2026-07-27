function assert(cond, msg) { if (!cond) throw new Error("FIXTURE ASSERT: " + msg); }

// Pattern C: bare `class Name extends Expr` declaration (no assignment).
var c = new Circle(2);
assert(c instanceof Circle, "instanceof Circle");
assert(c instanceof Shape, "instanceof Shape");
assert(c.kind === "circle", "super(literal) reached parent constructor");
assert(c.radius === 2, "this.x after super() lands on instance");
assert(c.area() === 12, "instance method uses instance state");

var u = Circle.unit();
assert(u instanceof Circle, "static method attached to the class, not the prototype");
assert(u.area() === 3, "static factory builds working instances");
"ok";
