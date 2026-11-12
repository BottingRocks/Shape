// Small JavaScript VM fixture for conditional jumps: the prefix topology of
// javascript-vm-prefix.raw.js plus handlers that push a program value, push
// literals, jump on the top of the stack, write a host property, increment
// the top, build fake-branch tests and return from the VM function. The image `r` is replaced by each test.
(function kernel(host, seed, mode, initialConfig) {
  var r = [0, 0, 0, 0, 0];
  function vmCreateStack() {
    var stack = [];
    Object.defineProperty(stack, "pop", { value: Array.prototype.pop });
    Object.defineProperty(stack, "push", { value: Array.prototype.push });
    Object.defineProperty(stack, "slice", { value: Array.prototype.slice });
    Object.defineProperty(stack, "splice", { value: Array.prototype.splice });
    return stack;
  }
  function Op() { this.o = []; }
  Object.defineProperty(Op.prototype, "o", { value: null });
  Object.defineProperty(Op.prototype, "T", { value: function (index) { this.o[index] = { v: void 0 }; } });
  Object.defineProperty(Op.prototype, "q", { value: function (index) { return this.o[index].v; } });
  Object.defineProperty(Op.prototype, "Tg", { value: function (index, value) { this.o[index].v = value; } });
  Object.defineProperty(Op.prototype, "K", { value: function () { var copy = new Op(); copy.o = this.o.slice(0); return copy; } });
  function vmContext(byteOffset, dispatchState, memory, receiver) {
    this.stack = vmCreateStack();
    this.E = vmCreateStack();
    this.T = vmCreateStack();
    this.G = vmCreateStack();
    this.C = vmCreateStack();
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
    var frame = context.G.pop();
    for (var index = 0; index < frame.c; ++index) {
      var tracker = context.E.pop();
      if (tracker.h) {
        context.T.pop();
      }
    }
    context.E.push({ h: true });
    context.T.push(error);
    context.P = frame.a;
    context.f = frame.b;
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
      context.stack[context.stack.length] = host.flag;
    },
    function (context) {
      context.stack[context.stack.length] = 1;
    },
    function (context) {
      context.stack[context.stack.length] = 2;
    },
    function (context) {
      var value1 = r[context.P];
      var value2 = r[context.P + 1];
      context.P += 2;
      if (!context.stack[context.stack.length - 1]) {
        context.P = value1;
        context.f = value2;
      }
      context.stack.length -= 1;
    },
    function (context) {
      var value1 = r[context.P];
      var value2 = r[context.P + 1];
      context.P = value1;
      context.f = value2;
    },
    function (context) {
      BN = context.stack[context.stack.length - 1];
    },
    function (context) {
      host.out = context.stack[context.stack.length - 1];
      context.stack.length -= 1;
    },
    function (context) {
      context.stack[context.stack.length - 1] = context.stack[context.stack.length - 1] + 1;
    },
    function (context) {
      context.stack[context.stack.length] = [];
    },
    function (context) {
      context.stack[context.stack.length - 1] = typeof context.stack[context.stack.length - 1];
    },
    function (context) {
      context.stack[context.stack.length - 2] = context.stack[context.stack.length - 2] === context.stack[context.stack.length - 1];
      context.stack.length -= 1;
    },
    function (context) {
      context.stack[context.stack.length] = "object";
    },
    function (context) {
      context.stack[context.stack.length - 1] = context.stack[context.stack.length - 1] == null;
    },
    function (context) {
      context.stack.push(function (x) {
        return x + 1;
      });
    },
    function (context) {
      var value1 = context.stack[context.stack.length - 2];
      context.stack[context.stack.length - 2] = new value1(context.stack[context.stack.length - 1]);
      context.stack.length -= 1;
    },
    function (context) {
      context.stack.push(function () {
        return host;
      });
    },
    function (context) {
      context.stack.length -= 1;
    },
    function (context) {
      var value1 = r[context.P];
      var value2 = r[context.P + 1];
      context.P += 2;
      context.G.push({ a: value1, b: value2, c: 0 });
    },
    function (context) {
      context.G.pop();
    },
    function (context) {
      context.E.push({ h: false, s: context.P, t: context.f });
      var value1 = context.G.pop();
      context.P = value1.a;
      context.f = value1.b;
    },
    function (context) {
      var value1 = context.E.pop();
      if (value1.h) {
        throw context.T.pop();
      }
      context.P = value1.s;
      context.f = value1.t;
    },
    function (context) {
      context.E.pop();
      context.stack.push(context.T.pop());
    },
    function (context) {
      var value1 = context.E.pop();
      if (value1.h) {
        context.T.pop();
      }
    },
    function (context) {
      throw context.stack[context.stack.length - 1];
      context.stack.length -= 1;
    },
    function (context) {
      context.stack[context.stack.length] = context.stack[context.stack.length - 1];
    },
    function (context) {
      context.stack[context.stack.length] = context.memory.q(0);
    },
    function (context) {
      var value1 = r[context.P];
      var value2 = r[context.P + 1];
      context.P += 2;
      context.memory.Tg(value1, value2);
    },
    function (context) {
      var value1 = r[context.P];
      var value2 = r[context.P + 1];
      context.P += 2;
      var value3 = context.memory.q(value1);
      var value4 = context.memory.q(value2);
      context.P = value3;
      context.f = value4;
    },
    function (context) {
      "use strict";
      context.stack[context.stack.length - 2] = delete context.stack[context.stack.length - 2][context.stack[context.stack.length - 1]];
      context.stack.length -= 1;
    },
    function (context) {
      if (context.stack[context.stack.length - 1] === null || context.stack[context.stack.length - 1] === void 0) {
        throw new TypeError(context.stack[context.stack.length - 1] + " is not an object");
      }
      context.stack[context.stack.length - 1] = Object(context.stack[context.stack.length - 1]);
    },
    function (context) {
      var value1 = [];
      for (var value2 in context.stack[context.stack.length - 1]) {
        Array.prototype.push.call(value1, value2);
      }
      context.stack[context.stack.length - 1] = value1;
    },
    function (context) {
      var value1 = r[context.P];
      var value2 = r[context.P + 1];
      context.P += 2;
      context.C.push({ x: context.P, y: context.f });
      context.P = value1;
      context.f = value2;
    },
    function (context) {
      var value1 = context.C.pop();
      context.P = value1.x;
      context.f = value1.y;
    }
  ];
  var BD = [
    [[0, 0], [0, 1], [0, 2], [0, 3], [0, 4], [0, 5], [0, 6], [0, 7], [0, 8], [0, 9], [0, 10], [0, 11], [0, 12], [0, 13], [0, 14], [0, 15], [0, 16], [0, 17], [0, 18], [0, 19], [0, 20], [0, 21], [0, 22], [0, 23], [0, 24], [0, 25], [0, 26], [0, 27], [0, 28], [0, 29], [0, 30], [0, 31], [0, 32], [0, 33]]
  ];
  vmRun(new vmContext(0, 0, new Op(), null));
  use(host, seed, mode, initialConfig, Bk, x);
})(typeof window !== "undefined" ? window : self, 0, 0, [2, 0, 3, 1]);
