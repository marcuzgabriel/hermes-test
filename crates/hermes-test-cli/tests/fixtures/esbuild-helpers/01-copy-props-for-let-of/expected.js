  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __copyProps = (to, from, except, desc) => {
    if (from && typeof from === "object" || typeof from === "function") {
      var keys = __getOwnPropNames(from);
      for (var i = 0; i < keys.length; i++) {
        var key = keys[i];
        if (!__hasOwnProp.call(to, key) && key !== except)
          __defProp(to, key, { get: ((k) => from[k]).bind(null, key), enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable, configurable: true });
      }
    }
    return to;
  };
