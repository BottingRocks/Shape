var operations = [
  function (context, x) {
    var computed = x + "!";
    context.stack[0] = computed;
  },
  function (context, slot) {
    context.memory.initializeSlot(slot);
    context.memory.writeSlot(slot, context.stack[0]);
    return context.memory.readSlot(slot);
  }
];
