(function kernel(host, seed, mode, initialConfig) {
  var Ok = Object.bind, OH = Object.call, Ov = OH.bind(Ok, OH);
  var nativeArrayPop = [].pop, nativeArrayPush = [].push, nativeArraySlice = [].slice, nativeArraySplice = [].splice;
  var popValue = Ov(nativeArrayPop), pushValue = Ov(nativeArrayPush), sliceValues = Ov(nativeArraySlice), spliceValues = Ov(nativeArraySplice);
  var B = Object;
  function OC() {
    var s = [];
    B.defineProperty(s, "Ti", { value: nativeArrayPush });
    B.defineProperty(s, "TK", { value: nativeArraySplice });
    B.defineProperty(s, "TB", { value: nativeArrayPop });
    B.defineProperty(s, "Tb", { value: nativeArraySlice });
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
    context.stack.TB();
    context.stack.Ti(1);
    context.stack.Tb(0, 2);
    context.stack.TK(0, 1, 2);
    context.errorStack.TB();
    B.getOwnPropertyDescriptor(context.stack, "TB");
    return context.stack.length;
  }
  return use(new Oz(0, 0, null, null));
})(typeof window !== "undefined" ? window : self, 0, 0, [1, 0, 2, 3]);
