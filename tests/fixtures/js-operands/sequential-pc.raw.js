var r = [0, 1, 2, 3], BD = [];
function vmContext(p) { this.P = p; }
vmContext.prototype.q = function () { return BD[this.f][r[this.P++]]; };
var operations = [function (context) {
  var first = r[context.P];
  context.P += 1;
  var saved = context.P;
  var second = r[saved];
  context.P += 1;
  var third = r[context.P];
  return first + second + third;
}];
