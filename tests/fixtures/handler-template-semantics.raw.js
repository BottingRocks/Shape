var operations = [
  function (context, index, value) {
    context.stack[index + 1] += value;
  },
  function (context, slot) {
    context.memory.initializeSlot(slot);
    return context.memory.readSlot(slot);
  },
  function (context, slot, value, values) {
    context.memory.writeSlot(slot, ...values);
  },
  function (context, other) {
    context = other;
    return context.stack[0];
  }
];
