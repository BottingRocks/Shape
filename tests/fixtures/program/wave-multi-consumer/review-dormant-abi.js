function main(flag) {
  let grouped;
  function producer(emitters) { unknown(emitters); }
  function secondProducer(emitters) { unknownSecond(emitters); }
  function coordinate(emitters, factory, context, init, collect, flag) {
    if (false) { factory(emitters, context, init, collect, function(){}, function(){}); }
    factory(emitters, context, init, collect);
  }
  function start() {
    grouped.forEach((factory, index) => {
      coordinate([], factory, {}, function(){}, function(){}, false);
    });
  }
  const startAlias = start;
  if (flag) {
    grouped = [producer, secondProducer];
    function initialize() { startAlias(); }
    return {initialize};
  }
  return {};
}
main(flag);
