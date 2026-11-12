function main() {
  const registryContext = {kind: "multi-consumer"};
  function $THREAD_1_101(emitters) { unknownFirst(emitters); }
  function $THREAD_1_101_2(emitters) { unknownSecond(emitters); }
  function meaningfulPurpose(emitters) { return emitters.length; }
  function coordinate(emitters, factory, context, init, collect, flag) {
    factory(emitters, context, init, collect, function(){}, function(){});
  }
  let grouped;
  function start() {
    grouped.forEach((factory, index) => {
      coordinate([], factory, {}, function(){}, function(){}, false);
    });
  }
  const run = start;
  function dormant() {}
  grouped = [$THREAD_1_101]; let selected = start; external(selected);
  selected = dormant; grouped = [$THREAD_1_101_2];
  return registryContext;
}
main();
