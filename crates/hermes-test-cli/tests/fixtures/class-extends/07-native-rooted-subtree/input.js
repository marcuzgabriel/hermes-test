var BaseError = class extends Error {
  constructor(msg) {
    super(msg);
    this.base = "B";
  }
};
var ChildError = class extends BaseError {
  constructor(msg) {
    super(msg);
    this.child = "C";
  }
};
var Plain = class {
  constructor() {
    this.kind = "plain";
  }
};
var UserChild = class extends Plain {
  constructor() {
    super();
    this.extra = 1;
  }
};
