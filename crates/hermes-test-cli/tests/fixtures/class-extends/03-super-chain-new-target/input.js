var A = class {
  constructor() {
    this.trail = ["A"];
  }
};
var B = class extends A {
};
var C = class extends B {
  constructor() {
    super();
    this.trail.push("C");
  }
};
var D = class extends C {
};
