var Stack = class extends Array {
  constructor() {
    super();
  }
  peek() {
    return this[this.length - 1];
  }
};
