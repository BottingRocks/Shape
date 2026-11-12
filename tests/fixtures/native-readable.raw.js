var guardBlob = typeof Blob === "function" ? Blob : null,
  guardWorker = typeof Worker === "function" ? Worker : null,
  Oo = ReferenceError,
  Ot = TypeError,
  OQ = Object,
  Oq = RegExp,
  OE = Number,
  OP = String,
  Os = Array,
  Ok = OQ.bind,
  OH = OQ.call,
  Ov = OH.bind(Ok, OH),
  z = OQ.apply,
  OA = Ov(z),
  nativeArrayPush = [].push,
  nativeArrayPop = [].pop,
  nativeArraySlice = [].slice,
  nativeArraySplice = [].splice,
  nativeArrayJoin = [].join,
  nativeArrayMap = [].map,
  r = Ov(nativeArrayPush),
  s = Ov(nativeArraySlice),
  c = Ov(nativeArrayJoin),
  B = Ov(nativeArrayMap),
  A = {}.hasOwnProperty,
  Og = Ov(A),
  jsonStringify = JSON.stringify,
  Ox = OQ.getOwnPropertyDescriptor,
  OF = OQ.defineProperty,
  Oh = OP.fromCharCode,
  u = typeof URL === "function" ? URL.createObjectURL : null,
  OT = guardBlob,
  OS = guardWorker,
  mathMin = Math.min,
  mathFloor = Math.floor,
  Oa = OQ.create,
  stringIndexOf = "".indexOf,
  stringCharAt = "".charAt,
  Ol = Ov(stringIndexOf),
  OM = Ov(stringCharAt),
  OZ = typeof Uint8Array === "function" ? Uint8Array : Os;

use(
  Oo, Ot, OQ, Oq, OE, OP, Os, Ok, OH, Ov, z, OA, nativeArrayPush,
  nativeArrayPop, nativeArraySlice, nativeArraySplice, nativeArrayJoin,
  nativeArrayMap, A, Og, jsonStringify, Ox, OF, Oh, u, OT, OS, mathMin,
  mathFloor, Oa, stringIndexOf, stringCharAt, Ol, OM, OZ
);
r(receiver, value);
s(receiver, start, end);
c(receiver, separator);
B(receiver, callback);
Og(receiver, key);
OA(fn, receiver, args);
new OZ(length);
