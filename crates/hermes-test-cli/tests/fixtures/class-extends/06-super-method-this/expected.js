var Base = class {
  constructor(name) {
    this.name = name;
  }
  describe() {
    return "base:" + this.name;
  }
};
var Sub = class extends Base {
  constructor(name) {
    super(name);
    this.initial = super.describe();
  }
  describe() {
    return "sub(" + super.describe() + ")";
  }
};
