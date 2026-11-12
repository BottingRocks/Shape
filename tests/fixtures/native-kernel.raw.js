(function kernel(host, seed, mode, initialConfig) {
  var strings = [
    "s00", "s01", "s02", "s03", "s04", "s05", "s06", "s07",
    "s08", "s09", "s10", "s11", "s12", "s13", "s14", "s15",
    "s16", "s17", "s18", "s19", "s20", "s21", "s22", "s23",
    "s24", "s25", "s26", "s27", "s28", "s29", "s30", "s31",
    "s32"
  ];
  var Ok = Object.bind, OH = Object.call, Ov = OH.bind(Ok, OH);
  var N = [].push, I = [].pop, R = [].slice, A = {}.hasOwnProperty;
  var pushValue = Ov(N), popValue = Ov(I), sliceValues = Ov(R), hasOwn = Ov(A);
  var c = Array, B = Object, Og = Ov(A);
  var table = [{ p: [0], S: [1], E: [2], J: [3], G: [4] }];
  function Op() { this.o = []; }
  B.defineProperty(Op.prototype, "o", { value: null });
  B.defineProperty(Op.prototype, "T", { value: function (index) { this.o[index] = { v: void 0 }; } });
  B.defineProperty(Op.prototype, "q", { value: function (index) { return this.o[index].v; } });
  B.defineProperty(Op.prototype, "Tg", { value: function (index, value) { this.o[index].v = value; } });
  B.defineProperty(Op.prototype, "K", { value: function () { var copy = new Op(); copy.o = this.o.slice(0); return copy; } });
  function OC() { var stack = []; B.defineProperty(stack, "Ti", { value: I }); return stack; }
  function Oz(byteOffset, dispatchState, memory, receiver) {
    this.TY = OC();
    this.y = OC();
    this.X = dispatchState;
    this.P = byteOffset;
    this.e = memory;
    this.TR = receiver == null ? host : B(receiver);
    this.TV = receiver;
  }
  function consume(a, b, cx) { return a + b + cx; }
  function run(context) { return consume(context.P, context.X, context); }
  function D(index, offset, state, memory) {
    var descriptor = table[index];
    return W(offset, state, memory, descriptor.p, descriptor.S, descriptor.E, descriptor.J, descriptor.G);
  }
  function W(offset, state, memory, initial, captured, args, argSlot, resultSlot) {
    var copy = new Op();
    return m(offset, state, copy, initial, args, true, argSlot);
  }
  function m(offset, state, memory, initial, args, provided, argSlot) {
    var count = args.length;
    var closure = function () {
      var clone = memory.K();
      var context = new Oz(offset, state, clone, this);
      return run(context);
    };
    return closure;
  }
  var ops = [function (context) { return context.e.q(0); }];
  ops[0](new Oz(0, 0, new Op(), null));
  var memory = new Op();
  memory.T(1); memory.q(1); memory.Tg(1, 2); memory.K();
  var unrelated = { o: 1, v: 2 };
  unrelated.o;
  var OZ = typeof Uint8Array === "function" ? Uint8Array : Array;
  var bytes = OZ(initialConfig[0]);
  pushValue(memory.o, { v: bytes });
  hasOwn(memory, "o");
  c(1);
  Og(memory, "o");
  D(0, 0, 0, memory);
  use(strings, mode);
})(
  typeof window !== "undefined" ? window : self,
  0,
  0,
  [1, 0, 2, 3]
);
