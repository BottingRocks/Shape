(function vmEntry(host, entryPc, dispatchMode, initialMemorySlots) {
  var vmStringTable = ["alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa", "lambda", "mu", "nu", "xi", "omicron", "pi"];
  var vmThreadDescriptors = [
    { argumentSlots: [4], initialSlots: [0, 2], capturedSlots: [7], resultSlot: 1 },
    { argumentSlots: [], initialSlots: [], capturedSlots: [] }
  ];
  var vmOperations = {
    7: function (context, operand) { context.memory.writeSlot(operand, context.stack.pop()); },
    2: function (context) { context.stack.push(context.memory.readSlot(0)); },
    11: function (context) { context.stack.push(1); },
    4: function (context) { return context.stack.length; }
  };
  var vmDispatchTable = [[1, 2], [3, 4]];
  var vmNumberConstants = [3, -0, NaN, Infinity];
  var vmNativeFunctions = [Object, Array.prototype.push, void 0];
  var vmBytecode = [7, 2, 11, 4];
  var vmSlotDescriptor = { value: null, writable: true };
  function vmMemory() {
    this.slots = [];
  }
  function vmContext(byteOffset, dispatchState, memory, receiver) {
    this.stack = [];
    this.errorStack = [];
    this.byteOffset = byteOffset;
    this.dispatchState = dispatchState;
    this.memory = memory;
    this.receiverValue = receiver;
  }
  function vmCreateStack() {
    var stack = [];
    Object.defineProperty(stack, "pop", { value: Array.prototype.pop });
    Object.defineProperty(stack, "push", { value: Array.prototype.push });
    Object.defineProperty(stack, "slice", { value: Array.prototype.slice });
    Object.defineProperty(stack, "splice", { value: Array.prototype.splice });
    return stack;
  }
  vmOperations[2](new vmContext(0, 1, new vmMemory(), host), 0);
})(typeof window !== "undefined" ? window : self, 17, 3, [9, 8, 7]);
