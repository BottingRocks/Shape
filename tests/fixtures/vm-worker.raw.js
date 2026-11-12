(function kernel(host, seed, mode, initialConfig) {
  var table = [{ p: [1], S: [2], E: [3], J: [4], G: [5] }];
  var B = Object;
  var f = { value: null, writable: true };
  function Op() { this.o = []; }
  B.defineProperty(Op.prototype, "o", f);
  B.defineProperty(Op.prototype, "T", { value: function (index) { this.o[index] = { v: void 0 }; } });
  B.defineProperty(Op.prototype, "q", { value: function (index) { return this.o[index].v; } });
  B.defineProperty(Op.prototype, "Tg", { value: function (index, value) { this.o[index].v = value; } });
  B.defineProperty(Op.prototype, "K", { value: function () { var copy = new Op(); copy.o = this.o.slice(0); return copy; } });
  var u = typeof URL === "function" ? URL.createObjectURL : null;
  var OT = typeof Blob === "function" ? Blob : null;
  var OS = typeof Worker === "function" ? Worker : null;
  var C;
  var x = [];
  function OX() {
    if (C === false) return false;
    if (C == null) {
      try {
        var l = "(" + kernel + ")(self," + 32547 + "," + 4 + "," + JSON.stringify(table[0].p) + ")";
        var O = new OT([l], { type: "application/javascript" });
        C = new OS(u(O));
        C.onmessage = function (l) { x[l.data.a](l.data.b); };
      } catch (l) { C = false; return false; }
    }
    return true;
  }
  function request(index, payload) {
    if (!OX()) return false;
    x.push(index);
    C.postMessage([index, payload]);
    return true;
  }
  use(table, request(0, seed), mode, initialConfig[0]);
})(typeof window !== "undefined" ? window : self, 0, 0, [1, 0, 2, 3]);
