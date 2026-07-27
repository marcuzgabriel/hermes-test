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
var Registry = class extends Map {
  constructor() {
    super();
  }
};
