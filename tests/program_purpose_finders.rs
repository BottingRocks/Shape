//! Purpose finders: callback registry, task queues, async rolling hashes,
//! interception wrappers and once wrappers.
//!
//! The sources below are written by hand in both representations: named
//! formals and `arguments[N]` with nested function declarations.


use shape::signals::analysis::roles::purposes::{find_program_purposes, PurposeReportEntry, PurposeStatus};

fn report(source: &str) -> Vec<PurposeReportEntry> {
    find_program_purposes(source).expect("report")
}

fn found<'r>(entries: &'r [PurposeReportEntry], role: &str) -> Vec<&'r PurposeReportEntry> {
    entries
        .iter()
        .filter(|entry| entry.role == role && entry.status == PurposeStatus::Found)
        .collect()
}

fn with_status<'r>(
    entries: &'r [PurposeReportEntry],
    role: &str,
    status: PurposeStatus,
) -> Vec<&'r PurposeReportEntry> {
    entries
        .iter()
        .filter(|entry| entry.role == role && entry.status == status)
        .collect()
}

/// The source text of the single found function for `role`.
fn found_text<'s>(source: &'s str, entries: &[PurposeReportEntry], role: &str) -> &'s str {
    let hits = found(entries, role);
    assert_eq!(hits.len(), 1, "exactly one {role}: {hits:#?}");
    let span = hits[0].function_spans[0];
    &source[span.start as usize..span.end as usize]
}

fn assert_starts(source: &str, entries: &[PurposeReportEntry], role: &str, prefix: &str) {
    let text = found_text(source, entries, role);
    assert!(
        text.starts_with(prefix),
        "{role} found `{}`",
        &text[..text.len().min(60)]
    );
}

fn found_roles(entries: &[PurposeReportEntry]) -> Vec<&str> {
    entries
        .iter()
        .filter(|entry| entry.status == PurposeStatus::Found)
        .map(|entry| entry.role.as_str())
        .collect()
}

// ===========================================================================
// (a) Callback registry
// ===========================================================================

const REGISTRY_NAMED: &str = r#"(function main() {
  function makeRegistry() {
    let registry = { list: [], notify: void 0, register: void 0, unregister: void 0 };
    function notifyAll(payload) {
      "use strict";
      let copy = slice(this.list);
      let index = copy.length;
      let result = void 0;
      let callback = void 0;
      while (index - 1 >= 0) {
        try {
          callback = copy[index - 1];
          if (callback != null) {
            callback = callback(payload, result);
            if (callback != null) {
              result = callback;
            }
          }
        } catch (e) {}
        --index;
      }
      return result;
    }
    registry.notify = notifyAll;
    function addOne(callback) {
      "use strict";
      this.list.push(callback);
    }
    registry.register = addOne;
    function removeOne(callback) {
      "use strict";
      let at = indexOf(this.list, callback);
      if (at !== -1) {
        this.list[at] = null;
      }
    }
    registry.unregister = removeOne;
    return registry;
  }
  let registryFactory = makeRegistry;
  return registryFactory();
})();"#;

const REGISTRY_TRACE: &str = r#"(function main() {
  let registryFactory;
  function makeRegistry() {
    let registry = { ["__callbacks"]: [], ["notify"]: void 0, ["register"]: void 0 };
    function notifyAll() {
      "use strict";
      let copy = bound(sliceRef, this["__callbacks"]);
      let index = copy["length"];
      let result = void 0;
      let callback = void 0;
      while (index - 1 >= 0) {
        try {
          callback = copy[index - 1];
          if (callback != null) {
            callback = callback(arguments[0], result);
            if (callback != null) {
              result = callback;
            }
          }
        } catch (e) {}
        --index;
      }
      return result;
    }
    registry["notify"] = notifyAll;
    function addOne() {
      "use strict";
      bound(pushRef, this["__callbacks"], arguments[0]);
      return void 0;
    }
    registry["register"] = addOne;
    return registry;
  }
  registryFactory = makeRegistry;
  return registryFactory();
})();"#;

#[test]
fn callback_registry_in_both_representations() {
    for source in [REGISTRY_NAMED, REGISTRY_TRACE] {
        let entries = report(source);
        assert_starts(
            source,
            &entries,
            "callback_registry_factory",
            "function makeRegistry",
        );
        assert_starts(
            source,
            &entries,
            "callback_registry_notify",
            "function notifyAll",
        );
        assert_starts(
            source,
            &entries,
            "callback_registry_register",
            "function addOne",
        );
        let factory = found(&entries, "callback_registry_factory")[0];
        assert_eq!(
            factory.action_name.as_deref(),
            Some("createCallbackRegistry")
        );
        assert_eq!(
            factory.holder_alias_name.as_deref(),
            Some("callbackRegistryFactory")
        );
    }
    // The unregister method is not a register lookalike.
    let entries = report(REGISTRY_NAMED);
    assert_eq!(found(&entries, "callback_registry_register").len(), 1);
}

#[test]
fn forward_emitter_is_not_a_registry_notify() {
    // Forward order and no threaded result: an event emitter, not the notifier.
    let source = r#"(function main() {
  function makeEmitter() {
    let emitter = { list: [], notify: void 0, register: void 0 };
    function emit(payload) {
      let copy = this.list.slice();
      for (let i = 0; i < copy.length; i++) {
        try { copy[i](payload); } catch (e) {}
      }
    }
    emitter.notify = emit;
    function add(callback) { this.list.push(callback); }
    emitter.register = add;
    return emitter;
  }
  return makeEmitter();
})();"#;
    let entries = report(source);
    assert!(found_roles(&entries).is_empty(), "{entries:#?}");
    assert_eq!(
        with_status(
            &entries,
            "callback_registry_factory",
            PurposeStatus::NotFound
        )
        .len(),
        1
    );
}

#[test]
fn reassigned_registry_bindings_fail_closed() {
    // The factory's own binding is reassigned.
    let source = REGISTRY_NAMED.replace(
        "  let registryFactory = makeRegistry;",
        "  let registryFactory = makeRegistry;\n  makeRegistry = null;",
    );
    let entries = report(&source);
    assert!(found(&entries, "callback_registry_factory").is_empty());
    assert_eq!(
        with_status(
            &entries,
            "callback_registry_factory",
            PurposeStatus::Refused
        )
        .len(),
        1
    );
    // The returned registry object itself is reassigned.
    let source = REGISTRY_NAMED.replace(
        "    return registry;",
        "    registry = {};\n    return registry;",
    );
    let entries = report(&source);
    assert!(found(&entries, "callback_registry_factory").is_empty());
}

// ===========================================================================
// (e) Once wrappers
// ===========================================================================

const ONCE_NAMED: &str = r#"(function main() {
  function makeOnce(callback) {
    let target = callback;
    let called = false;
    function guarded(value) {
      if (!called) {
        called = true;
        target(value);
      }
    }
    return guarded;
  }
  let onceAlias = makeOnce;
  return onceAlias;
})();"#;

const ONCE_TRACE: &str = r#"(function main() {
  let onceAlias;
  function makeOnce() {
    let called, target;
    target = arguments[0];
    called = false;
    function guarded() {
      if (!called) {
        called = true;
        target(arguments[0]);
      }
      return void 0;
    }
    return guarded;
  }
  onceAlias = makeOnce;
  return onceAlias;
})();"#;

#[test]
fn once_wrapper_in_both_representations() {
    for source in [ONCE_NAMED, ONCE_TRACE] {
        let entries = report(source);
        assert_starts(
            source,
            &entries,
            "once_callback_factory",
            "function makeOnce",
        );
        assert_starts(source, &entries, "once_guarded_invoke", "function guarded");
        let factory = found(&entries, "once_callback_factory")[0];
        assert_eq!(factory.action_name.as_deref(), Some("createOnceCallback"));
        assert_eq!(
            factory.holder_alias_name.as_deref(),
            Some("onceCallbackFactory")
        );
    }
}

#[test]
fn throttle_and_memoize_are_not_once() {
    // A throttle resets its latch from a timer: not once-only.
    let throttle = r#"(function main() {
  function makeThrottle(callback, interval) {
    let target = callback;
    let locked = false;
    function throttled() {
      if (locked) return;
      locked = true;
      setTimeout(() => { locked = false; }, interval);
      target();
    }
    return throttled;
  }
  return makeThrottle;
})();"#;
    // A memoizer stores the callback's result.
    let memoize = r#"(function main() {
  function memoize(callback) {
    let done = false;
    let value;
    function cached(input) {
      if (!done) {
        done = true;
        value = callback(input);
      }
      return value;
    }
    return cached;
  }
  return memoize;
})();"#;
    for source in [throttle, memoize] {
        let entries = report(source);
        assert!(found_roles(&entries).is_empty(), "{entries:#?}");
    }
}

#[test]
fn two_once_factories_in_one_scope_are_ambiguous() {
    let source = r#"(function main() {
  function first(callback) {
    let called = false;
    function inner(value) { if (!called) { called = true; callback(value); } }
    return inner;
  }
  function second(callback) {
    let called = false;
    function inner(value) { if (!called) { called = true; callback(value); } }
    return inner;
  }
  return [first, second];
})();"#;
    let entries = report(source);
    assert!(found(&entries, "once_callback_factory").is_empty());
    let ties = with_status(&entries, "once_callback_factory", PurposeStatus::Ambiguous);
    assert_eq!(ties.len(), 1);
    assert_eq!(ties[0].function_spans.len(), 2);
    // Each factory's inner function lives in its own scope: both are named.
    assert_eq!(found(&entries, "once_guarded_invoke").len(), 2);
}

// ===========================================================================
// (b) Task queues and their runners
// ===========================================================================

const SEQUENTIAL_QUEUE_NAMED: &str = r#"(function main() {
  function once(callback) {
    let target = callback;
    let called = false;
    function inner(value) { if (!called) { called = true; target(value); } }
    return inner;
  }
  let onceAlias = once;
  function runTask(task, done) {
    try {
      task();
      done();
    } catch (e) {
      done(e);
    }
  }
  let runner = runTask;
  function makeQueue(worker, items, schedule) {
    let timer = schedule;
    let work = worker;
    let lastError = void 0;
    let next = void 0;
    const initial = items === void 0 ? [] : items;
    if (schedule === void 0) {
      timer = window.setTimeout;
    }
    let started = false;
    let busy = false;
    let failed = false;
    let emptyListeners = [];
    let errorListeners = [];
    let pending = initial.slice();
    function step(error) {
      let current = error;
      if (error != null) {
        busy = false;
        lastError = error;
        failed = true;
        errorListeners.forEach(listener => {
          let handler = listener;
          timer(() => handler(current), 0);
        });
        return void 0;
      } else {
        if (pending.length > 0) {
          busy = true;
          let item = pending.shift();
          timer(() => {
            work(item, onceAlias(next));
          }, 0);
        } else {
          busy = false;
          emptyListeners.forEach(listener => {
            timer(listener, 0);
          });
        }
        return void 0;
      }
    }
    next = step;
    function begin() {
      if (started) {
        return void 0;
      } else {
        started = true;
        next();
        return void 0;
      }
    }
    let api = { start: begin };
    function add() {
      let count = arguments.length;
      let values = new Array(count);
      let i = 0;
      for (; i < count; i++) {
        values[i] = arguments[i];
      }
      [].push.apply(pending, values);
      if (started && !busy) {
        next();
      }
    }
    api.enqueue = add;
    function clear() {
      if (pending.length > 0) {
        pending = [];
        busy = false;
        emptyListeners.forEach(listener => {
          timer(listener, 0);
        });
      }
    }
    function whenEmpty(listener) {
      emptyListeners.push(listener);
      if (started && !busy && pending.length === 0) {
        timer(listener, 0);
      }
    }
    api.empty = clear;
    api.onEmpty = whenEmpty;
    function whenError(listener) {
      let handler = listener;
      let stored = void 0;
      if (failed) {
        stored = lastError;
        timer(() => {
          handler(stored);
        }, 0);
      } else {
        errorListeners.push(handler);
      }
    }
    api.onError = whenError;
    return api;
  }
  let queueFactory = makeQueue;
  let queue = queueFactory(runner);
  return queue;
})();"#;

/// The same queue in the trace representation: `arguments[N]`, computed
/// string keys, nested function declarations, and a caller-supplied
/// scheduler (the scheduled variant).
const SCHEDULED_QUEUE_TRACE: &str = r#"(function main() {
  let onceAlias, runner, queueFactory, queue;
  function once() {
    let called, target;
    target = arguments[0];
    called = false;
    function inner() { if (!called) { called = true; target(arguments[0]); } return void 0; }
    return inner;
  }
  onceAlias = once;
  function runTask() {
    try {
      arguments[0].call(void 0);
      arguments[1].call(void 0);
    } catch (e) {
      arguments[1].call(void 0, e);
    }
    return void 0;
  }
  runner = runTask;
  function makeQueue() {
    let work, lastError, timer, busy, started, failed, emptyListeners, errorListeners, pending, next;
    work = arguments[0];
    lastError = void 0;
    timer = arguments[1];
    busy = void 0;
    const initial = arguments[2] === void 0 ? [] : arguments[2];
    started = false;
    busy = false;
    failed = false;
    emptyListeners = [];
    errorListeners = [];
    pending = initial["slice"]();
    function step() {
      let item, current;
      current = arguments[0];
      if (arguments[0] != null) {
        lastError = arguments[0];
        failed = true;
        function notifyError() {
          let handler;
          handler = arguments[0];
          function deliver() { return handler(current); }
          timer(deliver, 0);
          return void 0;
        }
        errorListeners["forEach"](notifyError);
        return void 0;
      } else {
        if (pending["length"] > 0) {
          busy = true;
          item = pending["shift"]();
          function runNext() {
            work(item, onceAlias(next));
            return void 0;
          }
          timer(runNext, 0);
        } else {
          busy = false;
          function notifyEmpty() {
            timer(arguments[0], 0);
            return void 0;
          }
          emptyListeners["forEach"](notifyEmpty);
        }
        return void 0;
      }
    }
    next = step;
    function begin() {
      if (failed) {
        throw new Error("already failed");
      }
      if (started) {
        return void 0;
      } else {
        started = true;
        next();
        return void 0;
      }
    }
    let api = { ["start"]: begin };
    function add() {
      let count = arguments["length"];
      let values = new Array(count);
      let i = 0;
      for (; i < count; i++) {
        values[i] = arguments[i];
      }
      []["push"]["apply"](pending, values);
      if (started && !busy) {
        next();
      }
      return void 0;
    }
    api["enqueue"] = add;
    function clear() {
      if (pending["length"] > 0) {
        pending = [];
        busy = false;
        function notifyCleared() {
          timer(arguments[0], 0);
          return void 0;
        }
        emptyListeners["forEach"](notifyCleared);
      }
      return void 0;
    }
    function whenEmpty() {
      emptyListeners["push"](arguments[0]);
      if (started && !busy && pending["length"] === 0) {
        timer(arguments[0], 0);
      }
      return void 0;
    }
    api["empty"] = clear;
    api["onEmpty"] = whenEmpty;
    function whenError() {
      let handler, stored;
      handler = arguments[0];
      stored = void 0;
      if (failed) {
        stored = lastError;
        function deliverStored() {
          handler(stored);
          return void 0;
        }
        timer(deliverStored, 0);
      } else {
        errorListeners["push"](handler);
      }
      return void 0;
    }
    api["onError"] = whenError;
    return api;
  }
  queueFactory = makeQueue;
  queue = queueFactory(runner, setTimeout);
  return queue;
})();"#;

const TIME_BUDGETED_QUEUE_NAMED: &str = r#"(function main() {
  function makeClock(host) {
    return () => Date.now();
  }
  function makeQueue(worker, budget, items, clock) {
    let limit = budget;
    let started = void 0;
    let pending = void 0;
    let work = worker;
    let drainRef = void 0;
    let now = clock;
    const initial = items === void 0 ? [] : items;
    if (clock === void 0) {
      now = makeClock(window);
    }
    started = false;
    let busy = false;
    let failed = false;
    let emptyListeners = [];
    let errorListeners = [];
    pending = initial.slice();
    let lastError;
    function drain() {
      let begin = now();
      let average = 0;
      let count = 0;
      let end;
      function runOne() {
        if (pending.length <= 0) {
          busy = false;
          emptyListeners.forEach(listener => { setTimeout(listener, 0); });
          return 0;
        } else {
          busy = true;
          let item = pending.shift();
          let before = now();
          try {
            work(item);
          } catch (e) {
            let thrown = e;
            lastError = e;
            failed = true;
            busy = false;
            pending.length = 0;
            errorListeners.forEach(listener => {
              let handler = listener;
              setTimeout(() => handler(thrown), 0);
            });
            return 0;
          }
          end = now();
          average = (average * count + (end - before)) / (count + 1);
          ++count;
          return void 0;
        }
      }
      while (runOne() !== 0) {
        if (end + average < begin + limit) {
          continue;
        }
        break;
      }
      if (pending.length > 0) {
        setTimeout(drainRef, 0);
      }
    }
    drainRef = drain;
    function begin() {
      if (started) {
        return void 0;
      } else {
        started = true;
        setTimeout(drainRef, 0);
        return void 0;
      }
    }
    let api = { start: begin };
    function add() {
      let count = arguments.length;
      let values = new Array(count);
      let i = 0;
      for (; i < count; i++) {
        values[i] = arguments[i];
      }
      [].push.apply(pending, values);
      if (started && !busy) {
        setTimeout(drainRef, 0);
      }
    }
    api.enqueue = add;
    function clear() {
      if (pending.length > 0) {
        pending.length = 0;
        busy = false;
        emptyListeners.forEach(listener => { setTimeout(listener, 0); });
      }
    }
    function whenEmpty(listener) {
      emptyListeners.push(listener);
      if (started && pending.length === 0) {
        setTimeout(listener, 0);
      }
    }
    api.empty = clear;
    api.onEmpty = whenEmpty;
    function whenError(listener) {
      let handler = listener;
      let stored = void 0;
      if (failed) {
        stored = lastError;
        setTimeout(() => { handler(stored); }, 0);
      } else {
        errorListeners.push(handler);
      }
    }
    api.onError = whenError;
    return api;
  }
  let budgetedFactory = makeQueue;
  return budgetedFactory;
})();"#;

fn assert_queue_methods(source: &str, entries: &[PurposeReportEntry]) {
    assert_starts(source, entries, "task_queue_start", "function begin");
    assert_starts(source, entries, "task_queue_enqueue", "function add");
    assert_starts(source, entries, "task_queue_empty", "function clear");
    assert_starts(source, entries, "task_queue_on_empty", "function whenEmpty");
    assert_starts(source, entries, "task_queue_on_error", "function whenError");
}

#[test]
fn sequential_queue_and_runner_named_formals() {
    let source = SEQUENTIAL_QUEUE_NAMED;
    let entries = report(source);
    assert_starts(
        source,
        &entries,
        "sequential_task_queue_factory",
        "function makeQueue",
    );
    assert!(found(&entries, "scheduled_task_queue_factory").is_empty());
    assert!(found(&entries, "time_budgeted_task_queue_factory").is_empty());
    assert_queue_methods(source, &entries);
    assert_starts(
        source,
        &entries,
        "sequential_task_queue_runner",
        "function runTask",
    );
    let runner = found(&entries, "sequential_task_queue_runner")[0];
    assert_eq!(
        runner.action_name.as_deref(),
        Some("runSequentialQueueTask")
    );
    assert_eq!(
        runner.holder_alias_name.as_deref(),
        Some("sequentialQueueTaskRunner")
    );
    let factory = found(&entries, "sequential_task_queue_factory")[0];
    assert!(factory
        .evidence
        .iter()
        .any(|line| line.contains("proven once factory")));
}

#[test]
fn scheduled_queue_and_runner_trace_representation() {
    let source = SCHEDULED_QUEUE_TRACE;
    let entries = report(source);
    assert_starts(
        source,
        &entries,
        "scheduled_task_queue_factory",
        "function makeQueue",
    );
    assert!(found(&entries, "sequential_task_queue_factory").is_empty());
    assert_queue_methods(source, &entries);
    assert_starts(
        source,
        &entries,
        "scheduled_task_queue_runner",
        "function runTask",
    );
    assert!(found(&entries, "invoke_and_forward_error").is_empty());
}

#[test]
fn time_budgeted_queue_is_its_own_variant() {
    let source = TIME_BUDGETED_QUEUE_NAMED;
    let entries = report(source);
    assert_starts(
        source,
        &entries,
        "time_budgeted_task_queue_factory",
        "function makeQueue",
    );
    assert!(found(&entries, "sequential_task_queue_factory").is_empty());
    assert!(found(&entries, "scheduled_task_queue_factory").is_empty());
    assert_queue_methods(source, &entries);
}

#[test]
fn parallel_queue_copies_are_each_named() {
    // Two independent copies in sibling scopes: both factories and all of their
    // same-named methods are proven, because no two names share one scope.
    let inner = SEQUENTIAL_QUEUE_NAMED
        .trim_start_matches("(function main() {")
        .trim_end_matches("})();");
    let source = format!(
        "(function main() {{\n  function left() {{{inner}}}\n  function right() {{{inner}}}\n  return [left(), right()];\n}})();"
    );
    let entries = report(&source);
    assert_eq!(found(&entries, "sequential_task_queue_factory").len(), 2);
    assert_eq!(found(&entries, "task_queue_start").len(), 2);
    assert_eq!(found(&entries, "sequential_task_queue_runner").len(), 2);
}

#[test]
fn debounce_and_unconsumed_runner_are_not_queue_roles() {
    let source = r#"(function main() {
  function debounce(callback, wait) {
    let timer = null;
    function debounced() {
      clearTimeout(timer);
      timer = setTimeout(callback, wait);
    }
    return debounced;
  }
  function runTask(task, done) {
    try { task(); done(); } catch (e) { done(e); }
  }
  function cleanup(task, done) {
    try { task(); } finally { done(); }
  }
  return [debounce, runTask, cleanup];
})();"#;
    let entries = report(source);
    // A runner shape without a proven queue consumer proves no role: the
    // generic fallback was removed as dead (it never fired on a capture).
    assert!(found_roles(&entries).is_empty(), "{:?}", found_roles(&entries));
}

#[test]
fn reassigned_queue_alias_breaks_the_runner_proof() {
    // The factory alias the runner is passed through is reassigned: the
    // deferred-use proof fails closed and the runner keeps the generic role.
    let source = SEQUENTIAL_QUEUE_NAMED.replace(
        "  let queue = queueFactory(runner);",
        "  queueFactory = makeQueue;\n  let queue = queueFactory(runner);",
    );
    let entries = report(&source);
    assert!(found(&entries, "sequential_task_queue_runner").is_empty());
    // No fallback role is emitted when the consumer proof fails.
    assert!(
        !found_roles(&entries)
            .iter()
            .any(|role| *role == "invoke_and_forward_error"),
        "{:?}",
        found_roles(&entries)
    );
    // The factory itself is still proven by its own structure.
    assert_eq!(found(&entries, "sequential_task_queue_factory").len(), 1);
}

#[test]
fn queue_with_written_worker_slot_is_not_proven() {
    // Writing the worker formal breaks the slot proof (`worker` is no longer
    // provably the caller's slot 0).
    let source = SEQUENTIAL_QUEUE_NAMED.replace(
        "    let work = worker;",
        "    worker = worker || null;\n    let work = worker;",
    );
    let entries = report(&source);
    assert!(found(&entries, "sequential_task_queue_factory").is_empty());
}

// ---------------------------------------------------------------------------
// (b2) The initial-items default written as a leading if/else statement
// ---------------------------------------------------------------------------

/// The queue defaults initial items with a leading if (items === void 0) branch over a
/// hoisted local, then takes a slice() copy of that local for pending items.
fn sequential_queue_if_else() -> String {
    SEQUENTIAL_QUEUE_NAMED.replace(
        "    const initial = items === void 0 ? [] : items;",
        "    let initial;\n    if (items === void 0) {\n      initial = [];\n    } else {\n      initial = items;\n    }",
    )
}

#[test]
fn sequential_queue_accepts_if_else_initial_default() {
    let source = sequential_queue_if_else();
    assert!(
        source.contains("if (items === void 0) {"),
        "the if/else default fixture did not build"
    );
    let entries = report(&source);
    assert_starts(
        &source,
        &entries,
        "sequential_task_queue_factory",
        "function makeQueue",
    );
    // The factory and its five methods are the existing queue vocabulary; the
    // advance step is claimed by the queue-helper finder, and the caller's
    // worker runner is not re-claimed here.
    assert_queue_methods(&source, &entries);
    assert!(found(&entries, "scheduled_task_queue_factory").is_empty());
    assert!(found(&entries, "time_budgeted_task_queue_factory").is_empty());
}

#[test]
fn if_else_initial_default_survives_alpha_renaming() {
    // Generated spellings are not a gate: an alpha-renamed copy of the if/else
    // default still proves the same queue roles.
    let source = sequential_queue_if_else()
        .replace("items", "$BS")
        .replace("initial", "$KEY_9");
    let entries = report(&source);
    assert_starts(
        &source,
        &entries,
        "sequential_task_queue_factory",
        "function makeQueue",
    );
    assert_queue_methods(&source, &entries);
}

#[test]
fn scheduled_queue_accepts_if_else_initial_default_in_trace_form() {
    // The same default through the `arguments[N]` slot representation: the
    // undefined test and both arms read slot 2 directly.
    let source = SCHEDULED_QUEUE_TRACE.replace(
        "    const initial = arguments[2] === void 0 ? [] : arguments[2];",
        "    let initial;\n    if (arguments[2] === void 0) {\n      initial = [];\n    } else {\n      initial = arguments[2];\n    }",
    );
    assert!(
        source.contains("if (arguments[2] === void 0) {"),
        "the trace if/else default fixture did not build"
    );
    let entries = report(&source);
    assert_starts(
        &source,
        &entries,
        "scheduled_task_queue_factory",
        "function makeQueue",
    );
    assert_queue_methods(&source, &entries);
    assert!(found(&entries, "sequential_task_queue_factory").is_empty());
}

#[test]
fn if_else_initial_default_refuses_near_misses() {
    let base = sequential_queue_if_else();
    for (from, to) in [
        // wrong branch value: the non-empty arm assigns another formal
        ("      initial = items;", "      initial = schedule;"),
        // wrong key: the undefined test does not test the arm's slot
        (
            "    if (items === void 0) {",
            "    if (schedule === void 0) {",
        ),
        // empty alternate: both arms assign the empty array
        ("      initial = items;", "      initial = [];"),
        // the default arm is not the zero-element array
        ("      initial = [];", "      initial = [0];"),
        // branch side effect: the arm runs more than its one assignment
        (
            "      initial = [];\n    } else {",
            "      initial = [];\n      busy = false;\n    } else {",
        ),
        // extra write: a third write to the hoisted local
        (
            "      initial = items;\n    }\n    if (schedule === void 0) {",
            "      initial = items;\n    }\n    initial = items;\n    if (schedule === void 0) {",
        ),
        // donor if: the copy reads another local
        (
            "    let pending = initial.slice();",
            "    let pending = items.slice();",
        ),
        // missing copy: the pending list is not a slice
        (
            "    let pending = initial.slice();",
            "    let pending = initial;",
        ),
        // zero-argument copy: the slice takes an argument
        (
            "    let pending = initial.slice();",
            "    let pending = initial.slice(0);",
        ),
    ] {
        let source = base.replace(from, to);
        assert_ne!(source, base, "near miss {from:?} did not apply");
        let entries = report(&source);
        assert!(
            found(&entries, "sequential_task_queue_factory").is_empty(),
            "accepted near miss {from:?} -> {to:?}"
        );
    }
}

#[test]
fn if_else_initial_default_refuses_a_foreign_local() {
    // The default writes a local declared outside the factory, so the two
    // assignments are not the factory's own dominating definitions.
    let source = sequential_queue_if_else()
        .replace(
            "  function makeQueue(worker, items, schedule) {",
            "  let initial;\n  function makeQueue(worker, items, schedule) {",
        )
        .replace("    let initial;\n", "");
    assert!(
        source.contains("  let initial;\n  function makeQueue"),
        "the foreign-local fixture did not build"
    );
    let entries = report(&source);
    assert!(found(&entries, "sequential_task_queue_factory").is_empty());
}

// ===========================================================================
// (c) Asynchronous rolling hashes
// ===========================================================================

const HASHES_NAMED: &str = r#"(function main() {
  let code = s => s.charCodeAt();
  function hashSync(input) {
    if (typeof input === "string") {
      let end = input.length >> 1 << 1;
      let h = 0;
      for (let i = 0; i < end; i = i + 2) {
        h = (h << 5) - h + (code(input[i]) << 16 | code(input[i + 1])) | 0;
      }
      return h;
    }
    let words = new Int32Array(input.buffer);
    let w = 0;
    for (let j = 0; j < words.length; j++) {
      w = (w << 5) - w + words[j] | 0;
    }
    return w;
  }
  function hashViewAsync(view, budget, onDone, onError) {
    let fail = onError;
    let h = void 0;
    let words = void 0;
    let done = onDone;
    let padded;
    if (view.byteLength % 4 === 0) {
      padded = view.buffer;
    } else {
      padded = new ArrayBuffer(view.byteLength + (4 - view.byteLength % 4));
      new Int8Array(padded).set(new Int8Array(view.buffer));
    }
    words = new Int32Array(padded);
    let count = words.length;
    let cursor = 0;
    h = 0;
    new Scheduler(budget).schedule(() => {
      try {
        let stop = cursor + 400 < count ? cursor + 400 : count;
        while (cursor < stop) {
          h = (h << 5) - h + words[cursor] | 0;
          ++cursor;
        }
        if (cursor < count) {
          return true;
        } else {
          done(h);
          return false;
        }
      } catch (e) {
        fail(e);
        return false;
      }
    });
  }
  function hashStringAsync(text, budget, onDone, onError) {
    let done = onDone;
    let cursor = void 0;
    let input = text;
    let fail = onError;
    let end = text.length >> 1 << 1;
    cursor = 0;
    let h = 0;
    new Scheduler(budget).schedule(() => {
      try {
        let stop = cursor + 400 < end ? cursor + 400 : end;
        for (; cursor < stop; cursor = cursor + 2) {
          h = (h << 5) - h + (code(input[cursor]) << 16 | code(input[cursor + 1])) | 0;
        }
        if (cursor < end) {
          return true;
        } else {
          done(h);
          return false;
        }
      } catch (e) {
        fail(e);
        return false;
      }
    });
  }
  let viewHashAlias = hashViewAsync;
  let stringHashAlias = hashStringAsync;
  return [hashSync, viewHashAlias, stringHashAlias];
})();"#;

const HASHES_TRACE: &str = r#"(function main() {
  let viewHashAlias, stringHashAlias;
  function code() { return arguments[0].charCodeAt(); }
  function hashViewAsync() {
    let count, cursor, fail, h, words, done;
    fail = arguments[3];
    h = void 0;
    words = void 0;
    done = arguments[2];
    let ctor;
    let padded;
    if (arguments[0]["byteLength"] % 4 === 0) {
      ctor = Int32Array;
      padded = arguments[0]["buffer"];
    } else {
      padded = new ArrayBuffer(arguments[0]["byteLength"] + (4 - arguments[0]["byteLength"] % 4));
      new Int8Array(padded)["set"](new Int8Array(arguments[0]["buffer"]));
      ctor = Int32Array;
    }
    words = new ctor(padded);
    count = words["length"];
    cursor = 0;
    h = 0;
    function batch() {
      try {
        let stop = cursor + 400 < count ? cursor + 400 : count;
        while (cursor < stop) {
          h = (h << 5) - h + words[cursor] | 0;
          ++cursor;
        }
        if (cursor < count) {
          return true;
        } else {
          done(h);
          return false;
        }
      } catch (e) {
        fail(e);
        return false;
      }
    }
    new Scheduler(arguments[1])["schedule"](batch);
    return void 0;
  }
  function hashStringAsync() {
    let h, done, cursor, input, fail, end;
    done = arguments[2];
    cursor = void 0;
    input = arguments[0];
    fail = arguments[3];
    end = void 0;
    end = arguments[0]["length"] >> 1 << 1;
    cursor = 0;
    h = 0;
    function batch() {
      try {
        let stop = cursor + 400 < end ? cursor + 400 : end;
        for (; cursor < stop; cursor = cursor + 2) {
          h = (h << 5) - h + (code(input[cursor]) << 16 | code(input[cursor + 1])) | 0;
        }
        if (cursor < end) {
          return true;
        } else {
          done(h);
          return false;
        }
      } catch (e) {
        fail(e);
        return false;
      }
    }
    new Scheduler(arguments[1])["schedule"](batch);
    return void 0;
  }
  stringHashAlias = hashStringAsync;
  viewHashAlias = hashViewAsync;
  return [viewHashAlias, stringHashAlias];
})();"#;

#[test]
fn async_hashes_in_both_representations() {
    for source in [HASHES_NAMED, HASHES_TRACE] {
        let entries = report(source);
        assert_starts(
            source,
            &entries,
            "typed_array_hash_async",
            "function hashViewAsync",
        );
        assert_starts(
            source,
            &entries,
            "string_hash_async",
            "function hashStringAsync",
        );
        let view = found(&entries, "typed_array_hash_async")[0];
        assert_eq!(
            view.action_name.as_deref(),
            Some("hashArrayBufferViewAsync")
        );
        assert_eq!(
            view.holder_alias_name.as_deref(),
            Some("arrayBufferViewHashAsync")
        );
        assert_eq!(found(&entries, "typed_array_hash_batch_step").len(), 1);
        assert_eq!(found(&entries, "string_hash_batch_step").len(), 1);
    }
    // The synchronous sibling is the existing `string_buffer_hash` role and is
    // not reported again here.
    let entries = report(HASHES_NAMED);
    for entry in found(&entries, "typed_array_hash_async")
        .into_iter()
        .chain(found(&entries, "string_hash_async"))
    {
        let span = entry.function_spans[0];
        assert!(!HASHES_NAMED[span.start as usize..].starts_with("function hashSync"));
    }
}

