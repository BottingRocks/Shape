use sha2::{Digest, Sha256};
use shape::naming::name_threads;

fn named(source: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.outcomes.iter().any(|outcome| {
            outcome.applied
                && outcome.proposed_name.as_deref() == Some("getSignalSnapshot")
                && outcome.evaluator.as_deref() == Some("signals_signal_snapshot")
        })
    })
}

const SNAPSHOT: &str = r#"function main(){
  let filters = [], callbacks = {}, values = {}, metadata = {}, descriptors = {}, errors = {toArray(){return [];}};
  function $THREAD_0_1(options){
    let opts = options === void 0 ? {} : options;
    let filter = opts.signalFilter === void 0 ? filters : opts.signalFilter;
    if (opts.notifyLastChance === void 0 || opts.notifyLastChance) {
      filter.forEach(key => {
        if (callbacks[key] != null) {
          callbacks[key].call(void 0);
        }
      });
    }
    let bundles;
    if (opts.includeDebugNames !== void 0 && opts.includeDebugNames) {
      bundles = filter.map(key => {
        let record = {value: void 0, signal: void 0};
        record.value = values[key];
        record.signal = metadata[key];
        return record;
      });
    } else {
      bundles = filter.map(key => values[key]);
    }
    let keyed = [];
    let positional = [];
    let named = [];
    let i = 0;
    for (; i < bundles.length; i++) {
      let desc = descriptors[filter[i]];
      let target, push, payload;
      if (Object.prototype.hasOwnProperty.call(desc, "name")) {
        target = typeof desc.name === "number" ? keyed : named;
        push = target.push;
        payload = [desc.name, bundles[i]];
      } else {
        push = positional.push;
        target = positional;
        payload = bundles[i];
      }
      push.call(target, payload);
    }
    return {signals: named, positionalSignals: positional, keyedSignals: keyed, errors: errors.toArray()};
  }
  return $THREAD_0_1;
}
main();"#;

#[test]
fn snapshot_accepts_generated_and_alpha_variants() {
    let renamed = SNAPSHOT
        .replace("$THREAD_0_1", "$THREAD_902_44771_3")
        .replace("opts", "$KEY_12")
        .replace("bundles", "$KEY_15")
        .replace("record", "$KEY_16")
        .replace("desc", "$KEY_21")
        .replace("target", "$KEY_22")
        .replace("payload", "$KEY_24");
    for source in [SNAPSHOT.to_string(), renamed] {
        assert!(named(&source), "{source}");
    }
}

#[test]
fn snapshot_refuses_changed_operations_arguments_and_returns() {
    for (from, to) in [
        ("filter.forEach(key =>", "filter.map(key =>"),
        (
            "callbacks[key].call(void 0)",
            "callbacks[key].apply(void 0)",
        ),
        ("target.push", "target.unshift"),
        ("positional.push", "positional.unshift"),
        ("push.call(target, payload)", "push.apply(target, payload)"),
        (
            "Object.prototype.hasOwnProperty.call(desc, \"name\")",
            "Object.hasOwn(desc, \"name\")",
        ),
        ("errors.toArray()", "errors.toJSON()"),
        ("descriptors[filter[i]]", "descriptors[bundles[i]]"),
        (
            "typeof desc.name === \"number\"",
            "typeof desc.name === \"string\"",
        ),
        ("? keyed : named", "? named : keyed"),
        ("[desc.name, bundles[i]]", "[bundles[i], desc.name]"),
        ("push.call(target, payload)", "push.call(payload, target)"),
        (
            "signals: named, positionalSignals: positional",
            "signals: positional, positionalSignals: named",
        ),
        ("keyedSignals: keyed, ", ""),
        (
            "errors: errors.toArray()",
            "errors: errors.toArray(), extra: named",
        ),
        ("signals: named, ", ""),
    ] {
        let source = SNAPSHOT.replace(from, to);
        assert!(!named(&source), "accepted {from} -> {to}: {source}");
    }
}

#[test]
fn snapshot_refuses_capture_alias_and_cursor_changes() {
    for (from, to) in [
        ("? filters :", "? values :"),
        ("if (callbacks[key] != null)", "if (values[key] != null)"),
        ("errors: errors.toArray()", "errors: values.toArray()"),
        (
            "record.signal = metadata[key]",
            "record.signal = values[key]",
        ),
        ("let i = 0;", "let i = 1;"),
        ("i < bundles.length", "i < filter.length"),
        ("i++)", "i += 1)"),
        ("descriptors[filter[i]]", "descriptors[filter[0]]"),
        ("bundles[i]]", "bundles[0]]"),
    ] {
        let source = SNAPSHOT.replace(from, to);
        assert!(!named(&source), "accepted {from} -> {to}: {source}");
    }
}

#[test]
fn snapshot_refuses_callback_guard_and_shape_changes() {
    for (from, to) in [
        ("filter.forEach(key =>", "filter.forEach((key, extra) =>"),
        ("filter.forEach(key =>", "filter.forEach((key = 0) =>"),
        ("filter.forEach(key =>", "filter.forEach((...key) =>"),
        ("filter.forEach(key =>", "filter.forEach(({key}) =>"),
        ("filter.forEach(key =>", "filter.forEach(async key =>"),
        (
            "callbacks[key].call(void 0);",
            "callbacks[key].call(void 0);extra();",
        ),
        (
            "if (opts.notifyLastChance === void 0 || opts.notifyLastChance)",
            "if (opts.notifyLastChance)",
        ),
        ("options === void 0", "options === null"),
        (
            "opts.includeDebugNames !== void 0 && opts.includeDebugNames",
            "opts.includeDebugNames !== void 0 || opts.includeDebugNames",
        ),
        (
            "{value: void 0, signal: void 0}",
            "{value: null, signal: void 0}",
        ),
        (
            "filter.map(key => values[key])",
            "filter.map(key => {return values[key];})",
        ),
        (
            "function $THREAD_0_1(options)",
            "async function $THREAD_0_1(options)",
        ),
        (
            "function $THREAD_0_1(options)",
            "function* $THREAD_0_1(options)",
        ),
        (
            "function $THREAD_0_1(options)",
            "function $THREAD_0_1(options, extra)",
        ),
        (
            "let opts = options === void 0 ? {} : options;",
            "let opts = options === void 0 ? {} : options;opts;",
        ),
        ("return {signals:", "if (i) return {};return {signals:"),
    ] {
        let source = SNAPSHOT.replace(from, to);
        assert!(!named(&source), "accepted {from} -> {to}: {source}");
    }
}

#[test]
fn readable_names_and_collisions_remain_protected() {
    let readable = SNAPSHOT.replace("$THREAD_0_1", "keepSnapshot");
    assert_eq!(name_threads(&readable, true).unwrap().source, readable);
    let collision = SNAPSHOT.replace(
        "function main(){",
        "function main(){let getSignalSnapshot = 1;",
    );
    assert!(!named(&collision));
}

// Candidate shape: the debug record is one returned object literal, the named
// branch assigns push -> payload -> sink and the unnamed branch sink -> push ->
// payload. Same option protocol, same captured tables, same four-key report.
const CANDIDATE: &str = r#"function main(){
  let filters = [], callbacks = {}, values = {}, metadata = {}, descriptors = {}, errors = {toArray(){return [];}};
  function $THREAD_0_1(options){
    let opts = options === void 0 ? {} : options;
    let filter = opts.signalFilter === void 0 ? filters : opts.signalFilter;
    if (opts.notifyLastChance === void 0 || opts.notifyLastChance) {
      filter.forEach(key => {
        if (callbacks[key] != null) {
          callbacks[key].call(void 0);
        }
      });
    }
    let bundles;
    if (opts.includeDebugNames !== void 0 && opts.includeDebugNames) {
      bundles = filter.map(key => {
        return {value: values[key], signal: metadata[key]};
      });
    } else {
      bundles = filter.map(key => values[key]);
    }
    let keyed = [];
    let positional = [];
    let named = [];
    let i = 0;
    for (; i < bundles.length; i++) {
      let desc = descriptors[filter[i]];
      let target, push, payload;
      if (Object.prototype.hasOwnProperty.call(desc, "name")) {
        push = (typeof desc.name === "number" ? keyed : named).push;
        payload = [desc.name, bundles[i]];
        target = typeof desc.name === "number" ? keyed : named;
      } else {
        target = positional;
        push = positional.push;
        payload = bundles[i];
      }
      push.call(target, payload);
    }
    return {signals: named, positionalSignals: positional, keyedSignals: keyed, errors: errors.toArray()};
  }
  return $THREAD_0_1;
}
main();"#;

// Reordered declarations plus a placeholder report record filled by field
// writes: `signals` is the first declared list, `keyedSignals` the second and
// `positionalSignals` the third, so nothing may be bound by declaration order.
const REORDERED: &str = r#"function main(){
  let filters = [], callbacks = {}, values = {}, metadata = {}, descriptors = {}, errors = {toArray(){return [];}};
  function $THREAD_0_2(options){
    let opts = options === void 0 ? {} : options;
    let filter = opts.signalFilter === void 0 ? filters : opts.signalFilter;
    if (opts.notifyLastChance === void 0 || opts.notifyLastChance) {
      filter.forEach(key => {
        if (callbacks[key] != null) {
          callbacks[key].call(void 0);
        }
      });
    }
    let bundles;
    if (opts.includeDebugNames !== void 0 && opts.includeDebugNames) {
      bundles = filter.map(key => {
        return {value: values[key], signal: metadata[key]};
      });
    } else {
      bundles = filter.map(key => values[key]);
    }
    let named = [];
    let keyed = [];
    let positional = [];
    let i = 0;
    for (; i < bundles.length; i++) {
      let desc = descriptors[filter[i]];
      let target, push, payload;
      if (Object.prototype.hasOwnProperty.call(desc, "name")) {
        push = (typeof desc.name === "number" ? keyed : named).push;
        payload = [desc.name, bundles[i]];
        target = typeof desc.name === "number" ? keyed : named;
      } else {
        target = positional;
        push = positional.push;
        payload = bundles[i];
      }
      push.call(target, payload);
    }
    let report = {signals: named, positionalSignals: void 0, keyedSignals: void 0, errors: void 0};
    report.positionalSignals = positional;
    report.keyedSignals = keyed;
    report.errors = errors.toArray();
    return report;
  }
  return $THREAD_0_2;
}
main();"#;

#[test]
fn snapshot_accepts_direct_records_reordered_lists_and_placeholder_report() {
    // Base partition order with the candidate's direct debug record.
    let base_order_with_direct_record = SNAPSHOT.replace(
        "      bundles = filter.map(key => {\n        let record = {value: void 0, signal: void 0};\n        record.value = values[key];\n        record.signal = metadata[key];\n        return record;\n      });",
        "      bundles = filter.map(key => {\n        return {value: values[key], signal: metadata[key]};\n      });",
    );
    for source in [
        CANDIDATE.to_string(),
        REORDERED.to_string(),
        base_order_with_direct_record,
    ] {
        assert!(named(&source), "{source}");
    }
    let renamed = CANDIDATE
        .replace("$THREAD_0_1", "$THREAD_902_44771_3")
        .replace("opts", "$KEY_12")
        .replace("bundles", "$KEY_15")
        .replace("desc", "$KEY_21")
        .replace("target", "$KEY_22")
        .replace("payload", "$KEY_25");
    assert!(named(&renamed), "{renamed}");
}

#[test]
fn snapshot_refuses_wrong_array_and_signal_record_roles() {
    for (base, from, to) in [
        (
            CANDIDATE,
            "signals: named, positionalSignals: positional",
            "signals: positional, positionalSignals: named",
        ),
        (CANDIDATE, "signals: named, ", "signals: keyed, "),
        (CANDIDATE, "signal: metadata[key]", "signal: values[key]"),
        (
            CANDIDATE,
            "return {value: values[key], signal: metadata[key]};",
            "return {value: values[key], meta: metadata[key]};",
        ),
        (
            REORDERED,
            "report.keyedSignals = keyed;",
            "report.keyedSignals = positional;",
        ),
        (
            REORDERED,
            "report.positionalSignals = positional;",
            "report.positionalSignals = void 0;",
        ),
        (
            REORDERED,
            "report.errors = errors.toArray();",
            "report.errors = values.toArray();",
        ),
    ] {
        let source = base.replace(from, to);
        assert!(!named(&source), "accepted {from} -> {to}: {source}");
    }
}

#[test]
fn snapshot_refuses_shadowed_capture_and_disconnected_push() {
    // A local `metadata` shadows the captured signal table read by the debug
    // record, so the owned body is no longer the proven protocol.
    let shadowed = CANDIDATE.replace("let bundles;", "let metadata = {};\n    let bundles;");
    assert!(!named(&shadowed), "{shadowed}");

    for (from, to) in [
        (
            "push = (typeof desc.name === \"number\" ? keyed : named).push;",
            "push = keyed.push;",
        ),
        (
            "push = (typeof desc.name === \"number\" ? keyed : named).push;",
            "push = target.push;",
        ),
        ("push = positional.push;", "push = named.push;"),
        (
            "push.call(target, payload)",
            "push.call(positional, payload)",
        ),
        ("payload = bundles[i];", "payload = values[i];"),
    ] {
        let source = CANDIDATE.replace(from, to);
        assert!(!named(&source), "accepted {from} -> {to}: {source}");
    }
}

#[test]
fn snapshot_refuses_callback_decoys() {
    // A debug-shaped callback in the raw branch does not build raw values.
    let swapped = CANDIDATE.replace(
        "bundles = filter.map(key => values[key]);",
        "bundles = filter.map(key => ({value: values[key], signal: metadata[key]}));",
    );
    assert!(!named(&swapped), "{swapped}");

    // The record is hidden behind an immediately invoked callback decoy.
    let decoy = CANDIDATE.replace(
        "return {value: values[key], signal: metadata[key]};",
        "return (() => ({value: values[key], signal: metadata[key]}))();",
    );
    assert!(!named(&decoy), "{decoy}");

    // A second callback in the last-chance flush is not part of the protocol.
    let extra = CANDIDATE.replace(
        "callbacks[key].call(void 0);",
        "callbacks[key].call(void 0);filter.map(other => values[other]);",
    );
    assert!(!named(&extra), "{extra}");
}

// The named branch routes through `<routing>.push.call(<the same routing>, item)` while the
// unnamed branch calls positional.push(item) directly. The report is a direct four-key
// object.
const COMPRESSED: &str = r#"function main(){
  let filters = [], callbacks = {}, values = {}, metadata = {}, descriptors = {}, errors = {toArray(){return [];}};
  function $THREAD_0_1(options){
    let opts = options === void 0 ? {} : options;
    let filter = opts.signalFilter === void 0 ? filters : opts.signalFilter;
    if (opts.notifyLastChance === void 0 || opts.notifyLastChance) {
      filter.forEach(key => {
        if (callbacks[key] != null) {
          callbacks[key].call(void 0);
        }
      });
    }
    let bundles;
    if (opts.includeDebugNames !== void 0 && opts.includeDebugNames) {
      bundles = filter.map(key => {
        let record = {value: void 0, signal: void 0};
        record.value = values[key];
        record.signal = metadata[key];
        return record;
      });
    } else {
      bundles = filter.map(key => values[key]);
    }
    let keyed = [];
    let positional = [];
    let named = [];
    let i = 0;
    for (; i < bundles.length; i++) {
      let desc = descriptors[filter[i]];
      if (Object.prototype.hasOwnProperty.call(desc, "name")) {
        let payload = [desc.name, bundles[i]];
        (typeof desc.name === "number" ? keyed : named).push.call(typeof desc.name === "number" ? keyed : named, payload);
      } else {
        positional.push(bundles[i]);
      }
    }
    return {signals: named, positionalSignals: positional, keyedSignals: keyed, errors: errors.toArray()};
  }
  return $THREAD_0_1;
}
main();"#;

// Lowering variant of the compressed branch: direct debug record and a
// different list declaration order, same inline routing and direct push.
const COMPRESSED_RECORD: &str = r#"function main(){
  let filters = [], callbacks = {}, values = {}, metadata = {}, descriptors = {}, errors = {toArray(){return [];}};
  function $THREAD_0_1(options){
    let opts = options === void 0 ? {} : options;
    let filter = opts.signalFilter === void 0 ? filters : opts.signalFilter;
    if (opts.notifyLastChance === void 0 || opts.notifyLastChance) {
      filter.forEach(key => {
        if (callbacks[key] != null) {
          callbacks[key].call(void 0);
        }
      });
    }
    let bundles;
    if (opts.includeDebugNames !== void 0 && opts.includeDebugNames) {
      bundles = filter.map(key => {
        return {value: values[key], signal: metadata[key]};
      });
    } else {
      bundles = filter.map(key => values[key]);
    }
    let named = [];
    let keyed = [];
    let positional = [];
    let i = 0;
    for (; i < bundles.length; i++) {
      let desc = descriptors[filter[i]];
      if (Object.prototype.hasOwnProperty.call(desc, "name")) {
        let payload = [desc.name, bundles[i]];
        (typeof desc.name === "number" ? keyed : named).push.call(typeof desc.name === "number" ? keyed : named, payload);
      } else {
        positional.push(bundles[i]);
      }
    }
    return {signals: named, positionalSignals: positional, keyedSignals: keyed, errors: errors.toArray()};
  }
  return $THREAD_0_1;
}
main();"#;

// The four-key report object is assigned back onto the already-declared
// `named` list symbol and returned.
const BACK_ASSIGNED: &str = r#"function main(){
  let filters = [], callbacks = {}, values = {}, metadata = {}, descriptors = {}, errors = {toArray(){return [];}};
  function $THREAD_0_1(options){
    let opts = options === void 0 ? {} : options;
    let filter = opts.signalFilter === void 0 ? filters : opts.signalFilter;
    if (opts.notifyLastChance === void 0 || opts.notifyLastChance) {
      filter.forEach(key => {
        if (callbacks[key] != null) {
          callbacks[key].call(void 0);
        }
      });
    }
    let bundles;
    if (opts.includeDebugNames !== void 0 && opts.includeDebugNames) {
      bundles = filter.map(key => {
        let record = {value: void 0, signal: void 0};
        record.value = values[key];
        record.signal = metadata[key];
        return record;
      });
    } else {
      bundles = filter.map(key => values[key]);
    }
    let named = [];
    let keyed = [];
    let positional = [];
    let i = 0;
    for (; i < bundles.length; i++) {
      let desc = descriptors[filter[i]];
      let target, push, payload;
      if (Object.prototype.hasOwnProperty.call(desc, "name")) {
        target = typeof desc.name === "number" ? keyed : named;
        push = target.push;
        payload = [desc.name, bundles[i]];
      } else {
        push = positional.push;
        target = positional;
        payload = bundles[i];
      }
      push.call(target, payload);
    }
    named = {signals: named, positionalSignals: positional, keyedSignals: keyed, errors: errors.toArray()};
    return named;
  }
  return $THREAD_0_1;
}
main();"#;

// The report is assigned back onto the signals list through void 0 placeholders and later
// field writes, preserving the captured errors.toArray() sink.
const BACK_ASSIGNED_FIELDS: &str = r#"function main(){
  let filters = [], callbacks = {}, values = {}, metadata = {}, descriptors = {}, errors = {toArray(){return [];}};
  function $THREAD_0_1(options){
    let opts = options === void 0 ? {} : options;
    let filter = opts.signalFilter === void 0 ? filters : opts.signalFilter;
    if (opts.notifyLastChance === void 0 || opts.notifyLastChance) {
      filter.forEach(key => {
        if (callbacks[key] != null) {
          callbacks[key].call(void 0);
        }
      });
    }
    let bundles;
    if (opts.includeDebugNames !== void 0 && opts.includeDebugNames) {
      bundles = filter.map(key => {
        return {value: values[key], signal: metadata[key]};
      });
    } else {
      bundles = filter.map(key => values[key]);
    }
    let named = [];
    let keyed = [];
    let positional = [];
    let i = 0;
    for (; i < bundles.length; i++) {
      let desc = descriptors[filter[i]];
      let target, push, payload;
      if (Object.prototype.hasOwnProperty.call(desc, "name")) {
        target = typeof desc.name === "number" ? keyed : named;
        push = target.push;
        payload = [desc.name, bundles[i]];
      } else {
        push = positional.push;
        target = positional;
        payload = bundles[i];
      }
      push.call(target, payload);
    }
    named = {signals: named, positionalSignals: positional, keyedSignals: void 0, errors: void 0};
    named.keyedSignals = keyed;
    named.errors = errors.toArray();
    return named;
  }
  return $THREAD_0_1;
}
main();"#;

#[test]
fn snapshot_accepts_compressed_push_and_back_assigned_report() {
    for source in [
        COMPRESSED.to_string(),
        COMPRESSED_RECORD.to_string(),
        BACK_ASSIGNED.to_string(),
        BACK_ASSIGNED_FIELDS.to_string(),
    ] {
        assert!(named(&source), "{source}");
    }
    let renamed_compressed = COMPRESSED
        .replace("$THREAD_0_1", "$THREAD_902_44771_3")
        .replace("opts", "$KEY_12")
        .replace("bundles", "$KEY_15")
        .replace("desc", "$KEY_21")
        .replace("payload", "$KEY_25");
    assert!(named(&renamed_compressed), "{renamed_compressed}");
    let renamed_back = BACK_ASSIGNED
        .replace("$THREAD_0_1", "$THREAD_902_44771_3")
        .replace("opts", "$KEY_12")
        .replace("bundles", "$KEY_15")
        .replace("desc", "$KEY_21")
        .replace("target", "$KEY_22")
        .replace("payload", "$KEY_25");
    assert!(named(&renamed_back), "{renamed_back}");
}

#[test]
fn snapshot_refuses_compressed_push_routing_changes() {
    for (from, to) in [
        (
            "(typeof desc.name === \"number\" ? keyed : named).push.call(typeof desc.name === \"number\" ? keyed : named, payload)",
            "(typeof desc.name === \"number\" ? named : keyed).push.call(typeof desc.name === \"number\" ? named : keyed, payload)",
        ),
        (
            ").push.call(typeof desc.name === \"number\" ? keyed : named, payload)",
            ").push.call(typeof desc.name === \"number\" ? named : keyed, payload)",
        ),
        (
            "typeof desc.name === \"number\" ? keyed : named, payload)",
            "keyed, payload)",
        ),
        (
            "(typeof desc.name === \"number\" ? keyed : named).push.call(typeof desc.name === \"number\" ? keyed : named, payload);",
            "keyed.push.call(keyed, payload);",
        ),
        (
            "let payload = [desc.name, bundles[i]];",
            "let payload = [bundles[i], desc.name];",
        ),
        (
            "let payload = [desc.name, bundles[i]];",
            "let payload = [desc.name, bundles[i], desc.name];",
        ),
        ("positional.push(bundles[i]);", "named.push(bundles[i]);"),
        (
            "positional.push(bundles[i]);",
            "positional.push(values[i]);",
        ),
        (".push.call(", ".push.apply("),
    ] {
        let source = COMPRESSED.replace(from, to);
        assert!(!named(&source), "accepted {from} -> {to}: {source}");
    }
}

#[test]
fn snapshot_refuses_back_assigned_report_changes() {
    for (base, from, to) in [
        (
            BACK_ASSIGNED,
            "signals: named, positionalSignals: positional",
            "signals: positional, positionalSignals: positional",
        ),
        (
            BACK_ASSIGNED,
            "named = {signals: named",
            "keyed = {signals: named",
        ),
        (BACK_ASSIGNED, "return named;", "return keyed;"),
        (
            BACK_ASSIGNED,
            "errors: errors.toArray()",
            "errors: values.toArray()",
        ),
        (
            BACK_ASSIGNED,
            "named = {signals:",
            "named = named;named = {signals:",
        ),
        (BACK_ASSIGNED_FIELDS, "named.keyedSignals = keyed;", ""),
        (
            BACK_ASSIGNED_FIELDS,
            "named.keyedSignals = keyed;",
            "named.keyedSignals = positional;",
        ),
        (
            BACK_ASSIGNED_FIELDS,
            "named.errors = errors.toArray();",
            "named.errors = errors.toArray(extra);",
        ),
        (
            BACK_ASSIGNED_FIELDS,
            "named.errors = errors.toArray();",
            "named.errors = values.toArray();",
        ),
        (
            BACK_ASSIGNED_FIELDS,
            "named = {signals: named, positionalSignals: positional, keyedSignals: void 0, errors: void 0};",
            "named = {signals: named, positionalSignals: positional, keyedSignals: void 0, errors: void 0, extra: named};",
        ),
    ] {
        let source = base.replace(from, to);
        assert!(!named(&source), "accepted {from} -> {to}: {source}");
    }
}

// The split-filter body implements the complete snapshot protocol. The active filter is
// declared bare and assigned in both signalFilter arms rather than a conditional
// initializer. The unchanged body is wrapped only with its captured default filter,
// last-chance, value, signal, descriptor, and errors bindings.
const SPLIT_FILTER_BODY: &str = include_str!("fixtures/signal-snapshot-explicit-default-guard.js");
const SPLIT_FILTER_HEAD: &str = r#"function main(){
  let $KEY_6 = [], $KEY_15 = {}, $KEY_38 = {}, $KEY_313 = {}, $KEY_459 = {}, $KEY_387 = {toArray(){return [];}};
"#;
const SPLIT_FILTER_TAIL: &str = r#"
  return $THREAD_2_8552;
}
main();"#;

// The compact-conditional donor must remain accepted by the shared variant.
const CONDITIONAL_FILTER_BODY: &str = include_str!("fixtures/signal-snapshot-ternary-default.js");
const CONDITIONAL_FILTER_HEAD: &str = r#"function main(){
  let $KEY_225 = [], $KEY_14 = {}, $KEY_324 = {}, $KEY_411 = {}, $KEY_165 = {}, $KEY_416 = {toArray(){return [];}};
"#;
const CONDITIONAL_FILTER_TAIL: &str = r#"
  return $THREAD_8_57294;
}
main();"#;

fn sha(bytes: impl AsRef<[u8]>) -> String {
    Sha256::digest(bytes.as_ref())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn split_filter_positive() -> String {
    format!("{SPLIT_FILTER_HEAD}{SPLIT_FILTER_BODY}{SPLIT_FILTER_TAIL}")
}

fn conditional_filter_positive() -> String {
    format!("{CONDITIONAL_FILTER_HEAD}{CONDITIONAL_FILTER_BODY}{CONDITIONAL_FILTER_TAIL}")
}

fn snapshot_named(source: &str, target: &str) -> bool {
    name_threads(source, true).is_ok_and(|result| {
        result.report.outcomes.iter().any(|outcome| {
            outcome.applied
                && outcome.binding.name == target
                && outcome.proposed_name.as_deref() == Some("getSignalSnapshot")
                && outcome.evaluator.as_deref() == Some("signals_signal_snapshot")
        })
    })
}

fn snapshot_refused(source: &str, target: &str) {
    let result = name_threads(source, true).unwrap_or_else(|error| {
        panic!("negative source must parse and analyze: {error}\n{source}")
    });
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|outcome| outcome.binding.name == target)
        .unwrap_or_else(|| panic!("missing target {target}\n{source}"));
    assert!(
        !(outcome.applied
            && outcome.proposed_name.as_deref() == Some("getSignalSnapshot")
            && outcome.evaluator.as_deref() == Some("signals_signal_snapshot")),
        "negative was still named: {target}\n{source}"
    );
}

fn changed(source: &str, from: &str, to: &str) -> String {
    assert!(source.contains(from), "missing anchor {from}");
    source.replace(from, to)
}

#[test]
fn snapshot_accepts_split_filter_split_filter_variant_and_applied_donor() {
    assert_eq!(
        SPLIT_FILTER_BODY.len(),
        1325,
        "split-filter body must be the exact 1325-byte slice"
    );
    assert_eq!(
        sha(SPLIT_FILTER_BODY.as_bytes()),
        "f1ba3b89d2453ef35b4b5e9ff39b686b169f9b4bf0257e011896fc1eb23e86f4"
    );
    let split_filter = split_filter_positive();
    assert!(split_filter.contains(SPLIT_FILTER_BODY));
    assert!(snapshot_named(&split_filter, "$THREAD_2_8552"), "{split_filter}");

    assert_eq!(
        CONDITIONAL_FILTER_BODY.len(),
        1221,
        "donor body must be the exact 1221-byte slice"
    );
    assert_eq!(
        sha(CONDITIONAL_FILTER_BODY.as_bytes()),
        "220e6e0651fb3fdbea4e3fdb6e0cc56f77e2b8851075e13f55a5112740e793ce"
    );
    let conditional_filter = conditional_filter_positive();
    assert!(conditional_filter.contains(CONDITIONAL_FILTER_BODY));
    assert!(snapshot_named(&conditional_filter, "$THREAD_8_57294"), "{conditional_filter}");

    let alpha = split_filter
        .replace("$THREAD_2_8552", "$THREAD_902_44771_3")
        .replace("$KEY_5", "$KEY_91")
        .replace("$BZ", "$KEY_92")
        .replace("$CA", "$KEY_93")
        .replace("$CJ", "$KEY_94")
        .replace("$CE", "$KEY_95")
        .replace("$CF", "$KEY_96")
        .replace("$CG", "$KEY_97")
        .replace("$CH", "$KEY_98")
        .replace("$CB", "$KEY_81")
        .replace("$CC", "$KEY_82")
        .replace("$CD", "$KEY_83");
    assert!(snapshot_named(&alpha, "$THREAD_902_44771_3"), "{alpha}");
}

#[test]
fn snapshot_refuses_split_filter_split_filter_flow_receiver_and_partition_changes() {
    let base = split_filter_positive();
    let target = "$THREAD_2_8552";
    for (from, to) in [
        // Changed flow: wrong split operator and an emptied alternate arm.
        (
            "if ($KEY_5.signalFilter === void 0) {",
            "if ($KEY_5.signalFilter !== void 0) {",
        ),
        ("$BZ = $KEY_5.signalFilter;", ""),
        // Changed flow: a second write in the default arm.
        ("$BZ = $KEY_6;", "$BZ = $KEY_6; $BZ = $KEY_6;"),
        // Options: wrong default, wrong field, wrong absent test.
        (
            "let $KEY_5 = $BY === void 0 ? {} : $BY;",
            "let $KEY_5 = $BY === void 0 ? {} : $KEY_6;",
        ),
        ("$KEY_5.signalFilter === void 0", "$KEY_5.signal === void 0"),
        (
            "$KEY_5.notifyLastChance === void 0",
            "$KEY_5.notifyLastChance === null",
        ),
        (
            "$KEY_5.includeDebugNames !== void 0 && $KEY_5.includeDebugNames",
            "$KEY_5.includeDebugNames !== null && $KEY_5.includeDebugNames",
        ),
        // Receiver: wrong dispatch receiver and wrong push sink.
        ("$CG.call($CF, $CH);", "$CG.call($CD, $CH);"),
        ("$CG = $CF.push;", "$CG = $CB.push;"),
        // Partition: wrong numeric test, swapped routing, wrong ownership test.
        (
            "typeof $CE.name === \"number\"",
            "typeof $CE.name === \"string\"",
        ),
        ("$CC : $CB", "$CB : $CC"),
        (
            "Object.prototype.hasOwnProperty.call($CE, \"name\")",
            "Object.hasOwn($CE, \"name\")",
        ),
        ("$CH = [$CE.name, $CA[i]];", "$CH = [$CA[i], $CE.name];"),
        ("$KEY_459[$BZ[i]]", "$KEY_459[$CA[i]]"),
        // Report role: the keyed list is replaced by the positional list.
        ("keyedSignals: $CC,", "keyedSignals: $CD,"),
        // Aliases: value and signal read the same capture.
        ("signal: $KEY_313[$CJ]", "signal: $KEY_38[$CJ]"),
        // Aliases: the descriptor table aliases the captured default filter.
        ("let $CE = $KEY_459[$BZ[i]];", "let $CE = $KEY_6[$BZ[i]];"),
    ] {
        snapshot_refused(&changed(&base, from, to), target);
    }

    // Report/partition: the signals and positional roles are exchanged.
    let swapped_roles = changed(&base, "signals: $CB,", "signals: $TMP_SIGNALS,");
    let swapped_roles = changed(
        &swapped_roles,
        "positionalSignals: $CD,",
        "positionalSignals: $CB,",
    );
    let swapped_roles = changed(&swapped_roles, "signals: $TMP_SIGNALS,", "signals: $CD,");
    snapshot_refused(&swapped_roles, target);

    // Changed flow: the two filter arms are exchanged.
    let swapped_arms = changed(&base, "$BZ = $KEY_6;", "$BZ = $TMP_SWAP;");
    let swapped_arms = changed(&swapped_arms, "$BZ = $KEY_5.signalFilter;", "$BZ = $KEY_6;");
    let swapped_arms = changed(
        &swapped_arms,
        "$BZ = $TMP_SWAP;",
        "$BZ = $KEY_5.signalFilter;",
    );
    snapshot_refused(&swapped_arms, target);

    // Aliases: a local `$KEY_6` shadows the captured default filter.
    let shadowed = changed(&base, "let $BZ;", "let $KEY_6 = []; let $BZ;");
    snapshot_refused(&shadowed, target);

    // Dead borrowed helper: the default filter comes from an immediately
    // invoked helper that only reads the capture, not from the captured
    // binding itself.
    let borrowed = changed(&base, "$BZ = $KEY_6;", "$BZ = (() => $KEY_6)();");
    snapshot_refused(&borrowed, target);

    // Dead borrowed helper: a never-called nested helper mentions the capture
    // but is not part of the owned protocol statements.
    let dead_helper = changed(
        &base,
        "let $KEY_5 = $BY === void 0 ? {} : $BY;",
        "function $THREAD_DEAD() { return $KEY_6; } let $KEY_5 = $BY === void 0 ? {} : $BY;",
    );
    snapshot_refused(&dead_helper, target);
}

// Excerpts retain runtime, installation into a callable alias, table initialization, and an
// actual encoder caller. Only main and consumer wrappers are synthetic. The JavaScript is
// never executed.
const SHARED_SNAPSHOT_SOURCE: &str = include_str!("fixtures/signal-snapshot-owned-alternate.js");
const SHARED_SNAPSHOT_ALTERNATE: &str = include_str!("fixtures/signal-snapshot-owned-baseline.js");

#[test]
fn shared_snapshot_owned_full_table_debug_and_alias_installation() {
    for (source, target, install, invocation) in [
        (
            SHARED_SNAPSHOT_SOURCE,
            "$THREAD_3_111589",
            "$KEY_183 = $THREAD_3_111589;",
            "$KEY_183($CA)",
        ),
        (
            SHARED_SNAPSHOT_ALTERNATE,
            "$THREAD_8_86926",
            "$KEY_105 = $THREAD_8_86926;",
            "$KEY_105($AS)",
        ),
    ] {
        assert!(source.contains(install) && source.contains(invocation));
        assert!(snapshot_named(source, target), "{target}");
        // The containing runtime installs/initializes the protocol. It does
        // not inherit the snapshot name from its child.
        let result = name_threads(source, true).expect("valid owned source model");
        assert!(!result.report.outcomes.iter().any(|o| o.applied
            && ["$THREAD_3_93415", "$THREAD_6_80458"].contains(&o.binding.name.as_str())
            && o.proposed_name.as_deref() == Some("getSignalSnapshot")));
    }
}

fn shared_split_wasm() -> String {
    // Derived lowering of the compact-conditional donor. Both arms are explicitly owned and
    // undefined-only.
    changed(
        &conditional_filter_positive(),
        "let $KEY_2 = $CH === void 0 ? {} : $CH;",
        "let $KEY_2; if ($CH === void 0) { $KEY_2 = {}; } else { $KEY_2 = $CH; }",
    )
}

#[test]
fn shared_snapshot_wasm_split_options_and_debug_temporary() {
    let split = shared_split_wasm();
    assert!(snapshot_named(&split, "$THREAD_8_57294"));
    let flag = changed(
        &split,
        "if ($KEY_2.notifyLastChance === void 0 || $KEY_2.notifyLastChance)",
        "let debugFlag; if ($KEY_2.includeDebugNames === void 0) { debugFlag = false; } else { debugFlag = $KEY_2.includeDebugNames; } if ($KEY_2.notifyLastChance === void 0 || $KEY_2.notifyLastChance)",
    );
    let flag = changed(
        &flag,
        "if ($KEY_2.includeDebugNames !== void 0 && $KEY_2.includeDebugNames)",
        "if (debugFlag)",
    );
    assert!(snapshot_named(&flag, "$THREAD_8_57294"));
    for (from, to) in [
        ("$CH === void 0", "$CH == null"),
        ("$KEY_2 = $CH;", "$KEY_2 = $KEY_225;"),
        ("debugFlag = false;", "debugFlag = true;"),
        (
            "debugFlag = false;",
            "debugFlag = false; debugFlag = false;",
        ),
        (
            "$KEY_2.includeDebugNames === void 0",
            "$KEY_2.includeDebugNames == null",
        ),
        (
            "debugFlag = $KEY_2.includeDebugNames;",
            "debugFlag = $KEY_2.notifyLastChance;",
        ),
    ] {
        snapshot_refused(&changed(&flag, from, to), "$THREAD_8_57294");
    }
}

#[test]
fn shared_snapshot_refuses_foreign_routes_donors_and_mutable_origins() {
    for (from, to) in [
        ("$BG.call($BF, $BH);", "$BG.call($BB, $BH);"),
        ("$BG = $BB.push;", "$BG = $BD.push;"),
        (
            "$BG = (typeof $BE.name === \"number\" ? $BC : $BD).push;",
            "$BG = (typeof $BE.name === \"number\" ? $BD : $BC).push;",
        ),
        ("keyedSignals: $BC,", "keyedSignals: $BB,"),
        (
            "keyedSignals: $BC,",
            "keyedSignals: $BC, keyedSignals: $BC,",
        ),
        ("errors: $KEY_283.toArray()", "errors: $KEY_225.toArray()"),
        (
            "errors: $KEY_283.toArray()",
            "errors: $KEY_283.toArray(extra)",
        ),
        ("$AZ.signalFilter.forEach", "$KEY_210.forEach"),
        ("$AZ.signalFilter.map", "$KEY_210.map"),
        ("$KEY_31[$AZ.signalFilter[i]]", "$KEY_31[$KEY_210[i]]"),
        ("$KEY_340[$BK]", "$KEY_340[$BJ]"),
        ("value: $BJ,", "value: $KEY_225[$BJ],"),
        ("$BA = $KEY_225.map", "$BA = $KEY_340.map"),
        (
            "$AZ.notifyLastChance == null",
            "$AZ.notifyLastChance === void 0",
        ),
        ("includeDebugNames: false,", "includeDebugNames: true,"),
        (
            "$KEY_225 = $KEY_31.map(() => $KEY_56);",
            "$KEY_225 = $KEY_31.map(() => $KEY_56); $KEY_225 = foreignValues;",
        ),
        (
            "$KEY_0 = Object.create(null);",
            "$KEY_0 = Object.create(null); $KEY_0 = foreignCallbacks;",
        ),
        // Extra nested donor statements cannot supply the enclosing protocol.
        (
            "let $BA;",
            "function donor(){ return {signals: [], positionalSignals: [], keyedSignals: [], errors: []}; } let $BA;",
        ),
        // Removing the positional route still leaves valid JS and a Model.
        ("$BH = $BA[i];", ""),
    ] {
        snapshot_refused(&changed(SHARED_SNAPSHOT_ALTERNATE, from, to), "$THREAD_8_86926");
    }
    // Move the entire last-chance block after sampling, preserving valid syntax.
    let start = SHARED_SNAPSHOT_ALTERNATE
        .find("\t\t\t\t\t\tif ($AZ.notifyLastChance")
        .unwrap();
    let end = SHARED_SNAPSHOT_ALTERNATE[start..].find("\t\t\t\t\t\tlet $BA;").unwrap() + start;
    let notify = &SHARED_SNAPSHOT_ALTERNATE[start..end];
    let reordered = SHARED_SNAPSHOT_ALTERNATE.replacen(notify, "", 1);
    let reordered = changed(
        &reordered,
        "let $BB = [];",
        &format!("{notify}let $BB = [];"),
    );
    snapshot_refused(&reordered, "$THREAD_8_86926");
}
