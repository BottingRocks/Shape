// A portable recovered-program shape; never executed by the tests.
function $THREAD_0_0() {
  let meaningfulKey = 7;
  function keepHumanName(x) { return x + meaningfulKey; }
  function $THREAD_1_101(emitters) {
    let first = emitters[0], second = emitters[1];
    first.next([[], [1]]); first.next([[], [2]]); second.complete();
  }
  function $THREAD_2_202(emitters) { emitters[0].next([[], [3]]); }
  function $THREAD_9_900(observer) { observer.next([[], [4]]); }
  function wrap(task) {
    return function(observer, context) {
      task(observer, context, function(){}, function(){}, function(){}, function(){});
    };
  }
  let registry = [{ctor:null}, {ctor:null, name:17}, {ctor:null},
                  {ctor:wrap($THREAD_9_900)}, {ctor:null, name:"debug"}];
  let waves = [$THREAD_1_101, $THREAD_2_202];
  let rows = [[2,0],[1]];
  let flags = [true,false];
  function coordinate(emitters, factory, context, first, second, flag) {
    factory(emitters, context, first, second, function(){}, function(){});
  }
  function connect() {
    waves.forEach((factory, i) => {
      let descriptors = rows[i].map(index => registry[index]);
      if (flags[i]) coordinate(descriptors, factory, {}, function(){}, function(){}, flags[i]);
    });
    registry.forEach(descriptor => { if (descriptor.ctor) descriptor.ctor(); });
  }
  let filterDefault = [2,1,3,0,4];
  let values = [1,2,3,4,5];
  function snapshot(options) {
    let filter = options.signalFilter === void 0 ? filterDefault : options.signalFilter;
    let mapped;
    if (options.includeDebugNames) {
      mapped = filter.map(index => ({value:values[index]}));
    } else {
      mapped = filter.map(index => values[index]);
    }
    let named = [], keyed = [], positional = [];
    let i = 0;
    for (; i < mapped.length; i++) {
      let descriptor = registry[filter[i]];
      let destination, push, item;
      if (Object.prototype.hasOwnProperty.call(descriptor, "name")) {
        destination = typeof descriptor.name === "number" ? keyed : named;
        push = destination.push;
        item = [descriptor.name, mapped[i]];
      } else {
        push = positional.push;
        destination = positional;
        item = mapped[i];
      }
      push.call(destination, item);
    }
    return {signals:named, positionalSignals:positional, keyedSignals:keyed, errors:[]};
  }
  return {snapshot, connect, keepHumanName};
}
$THREAD_0_0();
