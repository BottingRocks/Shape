function main() {
  //            Parameter-only helper shared by the named transport protocols.
  function $B($PARAMA, $PARAMB, $PARAMC, $PARAMD) {
    $PARAMA[$PARAMB] = $PARAMC;
    return $PARAMD;
  }
  function $THREAD_1_101(emitters) { unknown(emitters); }
  function semanticPurpose(emitters) { return emitters.length; }
  function hookPurpose(context) { unknownHook(context); }
  function coordinate(emitters, factory, context, init, collect, flag) {
    factory(emitters, context, init, collect, function(){}, function(){});
  }
  let grouped = [$THREAD_1_101];
  grouped[2] = $B(grouped, 1,
    (emitters) => { return this === arguments[0] ? emitters : null; },
    (emitters) => { return emitters; });
  grouped[3] = semanticPurpose;
  const publicGroups = grouped;
  function start() {
    publicGroups.forEach((factory, index) => {
      coordinate([], factory, {}, function(){}, function(){}, false);
    });
  }
  let hooks = [hookPurpose];
  Array.prototype.map.call(hooks, hook => hook({}, {}, function(){}, function(){}));
  start();
  return {start, hooks};
}
main();
