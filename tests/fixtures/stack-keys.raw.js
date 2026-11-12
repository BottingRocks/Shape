(function kernel(host, seed, mode, initialConfig) {
  var Ok = Object.bind, OH = Object.call, Ov = OH.bind(Ok, OH);
  var nativeArrayPop = [].pop, nativeArrayPush = [].push, nativeArraySlice = [].slice, nativeArraySplice = [].splice;
  var popValue = Ov(nativeArrayPop), pushValue = Ov(nativeArrayPush), sliceValues = Ov(nativeArraySlice), spliceValues = Ov(nativeArraySplice);
  var B = Object;
  function OC() {
    var s = [];
    B.defineProperty(s, "Ti", { value: nativeArrayPop });
    B.defineProperty(s, "TK", { value: nativeArrayPush });
    B.defineProperty(s, "TB", { value: nativeArraySlice });
    B.defineProperty(s, "Tb", { value: nativeArraySplice });
    return s;
  }
  function Oz(byteOffset, dispatchState, memory, receiver) {
    this.stack = OC();
    this.errorStack = OC();
    this.byteOffset = byteOffset;
    this.dispatchState = dispatchState;
    this.memory = memory;
    this.receiver = receiver;
  }
  function use(context) {
    context.stack.Ti();
    context.stack.TK(1);
    context.stack.TB(0, 2);
    context.stack.Tb(0, 1, 2);
    context.errorStack.Ti();
    B.getOwnPropertyDescriptor(context.stack, "Ti");
    return context.stack.length;
  }
  return use(new Oz(0, 0, null, null));
})(typeof window !== "undefined" ? window : self, 0, 0, [1, 0, 2, 3]);
