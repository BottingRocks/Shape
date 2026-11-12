var r = [0, 1, 2, 3, 255, 128];
var BD = [];
function vmContext(start) { this.P = start; }
vmContext.prototype.q = function () { return BD[this.f][r[this.P++]]; };
var operations = [function (context) {
  var first = r[context.P];
  var second = r[context.P + 1] | r[context.P + 2] << 8;
  context.P += 3;
  return first + second;
}];
