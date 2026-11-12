var O = Object,
  A = Array,
  B = O.bind,
  C = O.call,
  D = C.bind(B, C),
  floor = Math.floor,
  indexOf = "".indexOf,
  charAt = "".charAt,
  lookupIndex = D(indexOf),
  lookupChar = D(charAt),
  allocator = typeof Uint8Array === "function" ? Uint8Array : A;

function decode(input, destination) {
  var alpha = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  var length = input.length;
  destination = destination || new allocator(floor(length * 3 / 4));
  var a, b, c, d, e, f, g;
  for (var inputCursor = 0, outputCursor = 0; inputCursor < length; inputCursor += 4, outputCursor += 3) {
    a = lookupIndex(alpha, lookupChar(input, inputCursor));
    b = lookupIndex(alpha, lookupChar(input, inputCursor + 1));
    c = lookupIndex(alpha, lookupChar(input, inputCursor + 2));
    d = lookupIndex(alpha, lookupChar(input, inputCursor + 3));
    e = a << 2 | b >> 4;
    f = (b & 15) << 4 | c >> 2;
    g = (c & 3) << 6 | d;
    destination[outputCursor] = e;
    if (inputCursor + 2 < length) { destination[outputCursor + 1] = f; }
    if (inputCursor + 3 < length) { destination[outputCursor + 2] = g; }
  }
  return destination;
}
