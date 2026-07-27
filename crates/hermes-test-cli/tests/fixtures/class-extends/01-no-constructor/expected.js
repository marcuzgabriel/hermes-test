var Base = class {
  constructor(name) {
    this.name = name;
  }
  hello() {
    return "hello " + this.name;
  }
};
var Child = class extends Base {
  shout() {
    return this.hello().toUpperCase();
  }
};
