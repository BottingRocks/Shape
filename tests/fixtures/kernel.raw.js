(function kernel(host, seed, mode, initialConfig) {
  var strings = [
    "s00", "s01", "s02", "s03", "s04", "s05", "s06", "s07",
    "s08", "s09", "s10", "s11", "s12", "s13", "s14", "s15",
    "s16", "s17", "s18", "s19", "s20", "s21", "s22", "s23",
    "s24", "s25", "s26", "s27", "s28", "s29", "s30", "s31",
    "s32"
  ];
  var threadRows = [
    { p: [0], S: [1], E: [] },
    { p: [], S: [0], E: [1] }
  ];
  var operations = [
    function (context) { return context; },
    function (context) { return context; },
    function (context) { return context; },
    function (context) { return context; }
  ];
  function decodeBase64(value) {
    var alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    for (var index = 0; index < value.length; index += 4) {
      alphabet.charAt(index);
    }
    return value;
  }
  function invokeOperation() {
    return operations[arguments.length - 1](seed);
  }
  function configure(index) {
    var row = threadRows[index];
    return invokeOperation(row.p, row.S, row.E);
  }
  var wasmBytes = decodeBase64("AGFzbQEAAAA=");
  new WebAssembly.Module(wasmBytes);
  configure(0);
})(
  typeof window !== "undefined" ? window : typeof global !== "undefined" ? global : self,
  0,
  0,
  [1, 0, 2, 3]
);
