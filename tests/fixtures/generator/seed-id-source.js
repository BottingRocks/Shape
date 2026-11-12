// An unrelated c literal is not a header context.
let noise = {}; noise.c = "decoy";
function header(descriptor) {
  let options = Object.create(null);
  options["c"] = "source-seed-id";
  options.d = descriptor === void 0 ? "source-descriptor" : descriptor;
  options['z'] = "q";
  return options;
}
