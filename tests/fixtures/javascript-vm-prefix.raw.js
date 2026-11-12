// Realistic small JavaScript VM topology fixture: transparent dispatch block, immutable
// `var x = Ctor.prototype` installation, BN/Bi run loop, a bounded
// bytecode-write handler, and a literal image whose next opcode is patched so
// the following dispatch consumes the mutated byte.
(function kernel(host, seed, mode, initialConfig) {
  var r = [0, 5, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
  function vmContext(byteOffset, dispatchState, memory, receiver) {
    this.stack = [];
    this.G = [];
    this.f = dispatchState;
    this.P = byteOffset;
    this.memory = memory;
    this.receiverObject = receiver == null ? host : Object(receiver);
    this.receiverValue = receiver;
  }
  var x = vmContext.prototype;
  Object.defineProperty(x, "q", { value: function () {
    {
      var record = BD[this.f][r[this.P++]];
      this.f = record[0];
      return record[1];
    }
  } });
  var Bk = {}, Bi = {}, BN = Bi;
  function BH(callable, context) {
    try {
      callable(context);
    } catch (error) {
      Bn(error, context);
    }
  }
  function Bn(error, context) {
    return error;
  }
  function vmRun(context) {
    var v, O;
    for (;;) {
      if (BN !== Bi) {
        O = BN;
        BN = Bi;
        return O;
      }
      v = context.q();
      if (context.G.length === 0) {
        vmOperations[v](context);
      } else {
        BH(vmOperations[v], context);
      }
    }
  }
  var vmOperations = [
    function (context) {
      var value1 = r[context.P] | (r[context.P + 1] | r[context.P + 2] << 8) << 8;
      var value2 = r[context.P + 3];
      context.P += 4;
      r[value1] = value2;
    },
    function (context) {
      return;
    },
    function (context) {
      context.stack[context.stack.length] = 1;
    },
    function (context) {
      context.G.push(1);
    }
  ];
  var BD = [
    [[0, 0], [0, 1]]
  ];
  vmRun(new vmContext(0, 0, null, null));
  use(host, seed, mode, initialConfig, Bk, x);
})(typeof window !== "undefined" ? window : self, 0, 0, [2, 0, 3, 1]);
