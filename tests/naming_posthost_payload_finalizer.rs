use shape::naming::name_threads;

// Inert finalizer source retains scoped identities and is parsed only. The body assembles
// seven 7-bit varint frames, two captured-sentinel marker branches, a captured value
// encoder, and the guarded entropy/checksum prefix. The row alias has a single origin that
// matching must follow.
const POSITIVE: &str = r#"function host(){
let encodeValue, options, sentinel, hasher, key, counter, other;
function $THREAD_5_9001(snapshot){
    let out = [];
    let errors = snapshot.errors.length === 0 ? [] : encodeValue(snapshot.errors);
    let f1 = [];
    if (errors.length >>> 0 < 128) {
        f1[0] = errors.length >>> 0;
    } else {
        let n1 = errors.length >>> 0 >>> 7;
        let a1 = -1;
        let b1 = errors.length >>> 0 & 127;
        for (;;) {
            a1 = a1 + 1;
            f1[a1] = b1 | 128;
            if (n1 >>> 7 <= 0) break;
            b1 = n1 & 127;
            n1 = n1 >>> 7;
        }
    }
    let f2 = [];
    if (errors.length >>> 0 < 128) {
        f2[0] = errors.length >>> 0;
    } else {
        let n2 = errors.length >>> 0 >>> 7;
        let a2 = -1;
        let b2 = errors.length >>> 0 & 127;
        for (;;) {
            a2 = a2 + 1;
            f2[a2] = b2 | 128;
            if (n2 >>> 7 <= 0) break;
            b2 = n2 & 127;
            n2 = n2 >>> 7;
        }
    }
    let f3 = [];
    if (errors.length >>> 0 < 128) {
        f3[0] = errors.length >>> 0;
    } else {
        let n3 = errors.length >>> 0 >>> 7;
        let a3 = -1;
        let b3 = errors.length >>> 0 & 127;
        for (;;) {
            a3 = a3 + 1;
            f3[a3] = b3 | 128;
            if (n3 >>> 7 <= 0) break;
            b3 = n3 & 127;
            n3 = n3 >>> 7;
        }
    }
    let f4 = [];
    if (errors.length >>> 0 < 128) {
        f4[0] = errors.length >>> 0;
    } else {
        let n4 = errors.length >>> 0 >>> 7;
        let a4 = -1;
        let b4 = errors.length >>> 0 & 127;
        for (;;) {
            a4 = a4 + 1;
            f4[a4] = b4 | 128;
            if (n4 >>> 7 <= 0) break;
            b4 = n4 & 127;
            n4 = n4 >>> 7;
        }
    }
    let f5 = [];
    if (errors.length >>> 0 < 128) {
        f5[0] = errors.length >>> 0;
    } else {
        let n5 = errors.length >>> 0 >>> 7;
        let a5 = -1;
        let b5 = errors.length >>> 0 & 127;
        for (;;) {
            a5 = a5 + 1;
            f5[a5] = b5 | 128;
            if (n5 >>> 7 <= 0) break;
            b5 = n5 & 127;
            n5 = n5 >>> 7;
        }
    }
    let f6 = [];
    if (errors.length >>> 0 < 128) {
        f6[0] = errors.length >>> 0;
    } else {
        let n6 = errors.length >>> 0 >>> 7;
        let a6 = -1;
        let b6 = errors.length >>> 0 & 127;
        for (;;) {
            a6 = a6 + 1;
            f6[a6] = b6 | 128;
            if (n6 >>> 7 <= 0) break;
            b6 = n6 & 127;
            n6 = n6 >>> 7;
        }
    }
    let f7 = [];
    if (errors.length >>> 0 < 128) {
        f7[0] = errors.length >>> 0;
    } else {
        let n7 = errors.length >>> 0 >>> 7;
        let a7 = -1;
        let b7 = errors.length >>> 0 & 127;
        for (;;) {
            a7 = a7 + 1;
            f7[a7] = b7 | 128;
            if (n7 >>> 7 <= 0) break;
            b7 = n7 & 127;
            n7 = n7 >>> 7;
        }
    }
    let i = 0;
    for (; i < errors.length; i++) {
        out.push(errors[i]);
    }
    let p = 0;
    for (; p < snapshot.positionalSignals.length; p++) {
        if (snapshot.positionalSignals[p] === sentinel) {
            out.push(0);
            out.push(0);
        } else {
            const keyBytes = snapshot.positionalSignals[p][0].length === 0 ? [] : encodeValue(snapshot.positionalSignals[p][0]);
            for (let q = 0; q < keyBytes.length; q++) {
                out.push(keyBytes[q]);
            }
            for (let r = 0; r < snapshot.positionalSignals[p][1].length; r++) {
                out.push(snapshot.positionalSignals[p][1][r]);
            }
        }
    }
    const named = snapshot.signals.length === 0 ? [] : encodeValue(snapshot.signals, options);
    for (let s = 0; s < named.length; s++) {
        out.push(named[s]);
    }
    let k = 0;
    while (k < snapshot.keyedSignals.length) {
        const row = snapshot.keyedSignals[k];
        if (row[1] === sentinel) {
            out.push(0);
            out.push(0);
        } else {
            const keyPart = row[1][0].length === 0 ? [] : encodeValue(row[1][0]);
            for (let u = 0; u < keyPart.length; u++) {
                out.push(keyPart[u]);
            }
            for (let v = 0; v < row[1][1].length; v++) {
                out.push(row[1][1][v]);
            }
        }
        k = k + 1;
    }
    if (hasher != null && key != null) {
        let a = Math.random() * 4294967296 >>> 0;
        let b = 0;
        try {
            b = performance.now() >>> 0;
        } catch (e) {}
        let c = Number(new Date()) >>> 0;
        let d = 0;
        try {
            d = crypto.getRandomValues(new Uint32Array(1))[0];
        } catch (e2) {}
        a = a ^ 1103515245;
        b = b === b ? a ^ b : a;
        c = c === c ? b ^ c : b;
        ++counter;
        d = (c ^ d ^ counter ^ a) >>> 0;
        out = hasher(out, key, d, d);
        out.unshift(d >>> 24, d >>> 16 & 255, d >>> 8 & 255, d & 255, d >>> 24, d >>> 16 & 255, d >>> 8 & 255, d & 255);
    }
    return out;
}
return $THREAD_5_9001;
}
host();"#;

fn alpha(source: &str) -> String {
    // Every local spelling that is not a protocol field name changes. The
    // member names (`errors`, `positionalSignals`, `signals`, `keyedSignals`)
    // are wire vocabulary and stay; only the scoped identities change.
    source
        .replace("$THREAD_5_9001", "$THREAD_900_777")
        .replace("snapshot", "$KEY_40")
        .replace("encodeValue", "$KEY_41")
        .replace("options", "$KEY_42")
        .replace("sentinel", "$KEY_43")
        .replace("hasher", "$KEY_44")
        .replace("counter", "$KEY_48")
        .replace("keyBytes", "$KEY_49")
        .replace("keyPart", "$KEY_50")
        .replace("named", "$KEY_51")
        .replace("row", "$KEY_52")
        .replace("out", "$KEY_46")
        .replace(", key, d, d)", ", $KEY_45, d, d)")
        .replace(" key != null", " $KEY_45 != null")
        .replace(", key,", ", $KEY_45,")
}

/// Loop-form and declaration-order variants: the same owned operations in a
/// different physical order, `while`/`for` swapped, and `out` initialized after
/// the error frame. No check may depend on statement position.
fn reordered(source: &str) -> String {
    let source = source.replace(
        "    let out = [];\n    let errors =",
        "    let errors =",
    );
    let source = source.replace(
        "    let i = 0;\n    for (; i < errors.length; i++) {",
        "    let out = [];\n    let i = 0;\n    for (; i < errors.length; i++) {",
    );
    source.replace(
        "    let p = 0;\n    for (; p < snapshot.positionalSignals.length; p++) {",
        "    let p = 0;\n    while (p < snapshot.positionalSignals.length) {",
    )
}

fn named(source: &str, target: &str, name: &str) -> bool {
    name_threads(source, true).is_ok_and(|r| {
        r.report.outcomes.iter().any(|o| {
            o.applied && o.binding.name == target && o.proposed_name.as_deref() == Some(name)
        })
    })
}

/// A negative source must still parse and analyze; only the naming proof is
/// expected to fail. This keeps refusal tests from passing vacuously.
fn refused(source: &str, target: &str, name: &str) {
    let result = name_threads(source, true)
        .unwrap_or_else(|error| panic!("negative source must parse and analyze: {error}\n{source}"));
    let outcome = result
        .report
        .outcomes
        .iter()
        .find(|o| o.binding.name == target)
        .unwrap_or_else(|| panic!("target binding {target} missing from outcomes:\n{source}"));
    assert!(
        !(outcome.applied && outcome.proposed_name.as_deref() == Some(name)),
        "accepted {target} as {name}:\n{source}"
    );
}

fn refuses(template: &str, target: &str, name: &str, changes: &[(&str, &str)]) {
    for &(from, to) in changes {
        assert!(template.contains(from), "missing replacement {from}");
        refused(&template.replace(from, to), target, name);
    }
}

#[test]
fn payload_finalizer_whole_body_is_named() {
    assert!(
        named(POSITIVE, "$THREAD_5_9001", "encodeSignalBundle"),
        "did not name the payload finalizer fixture"
    );
    assert!(
        named(&alpha(POSITIVE), "$THREAD_900_777", "encodeSignalBundle"),
        "did not name the alpha-renamed payload finalizer fixture"
    );
    assert!(
        named(&reordered(POSITIVE), "$THREAD_5_9001", "encodeSignalBundle"),
        "did not name the reordered / for-vs-while payload finalizer variant"
    );
}

#[test]
fn payload_finalizer_requires_snapshot_receiver_and_all_fields() {
    refuses(
        POSITIVE,
        "$THREAD_5_9001",
        "encodeSignalBundle",
        &[
            ("snapshot.errors", "other.errors"),
            ("snapshot.signals", "other.signals"),
            ("snapshot.positionalSignals", "other.positionalSignals"),
            ("snapshot.keyedSignals", "other.keyedSignals"),
            ("snapshot.errors", "snapshot.events"),
        ],
    );
}

#[test]
fn payload_finalizer_requires_sentinel_zero_markers() {
    refuses(
        POSITIVE,
        "$THREAD_5_9001",
        "encodeSignalBundle",
        &[
            ("out.push(0);", "out.push(1);"),
            (
                "if (row[1] === sentinel)",
                "if (row[1] === other)",
            ),
            (
                "if (snapshot.positionalSignals[p] === sentinel)",
                "if (snapshot.positionalSignals[p] == sentinel)",
            ),
        ],
    );
}

#[test]
fn payload_finalizer_requires_encoder_and_entropy_operations() {
    refuses(
        POSITIVE,
        "$THREAD_5_9001",
        "encodeSignalBundle",
        &[
            (
                "encodeValue(row[1][0])",
                "other(row[1][0])",
            ),
            (
                "encodeValue(snapshot.signals, options)",
                "encodeValue(other.signals, options)",
            ),
            (
                "out = hasher(out, key, d, d);",
                "out = hasher(other, key, d, d);",
            ),
            (
                "out.unshift(d >>> 24, d >>> 16 & 255, d >>> 8 & 255, d & 255, d >>> 24, d >>> 16 & 255, d >>> 8 & 255, d & 255);",
                "out.unshift(d >>> 24, d >>> 16 & 255, d >>> 8 & 255, d & 255);",
            ),
            (
                "Math.random() * 4294967296 >>> 0",
                "1 >>> 0",
            ),
            (
                "crypto.getRandomValues(new Uint32Array(1))[0]",
                "0",
            ),
        ],
    );
}

#[test]
fn payload_finalizer_does_not_descend_into_nested_callbacks() {
    refuses(
        POSITIVE,
        "$THREAD_5_9001",
        "encodeSignalBundle",
        &[
            (
                "let errors = snapshot.errors.length === 0 ? [] : encodeValue(snapshot.errors);",
                "let errors = (function (snapshot) { return snapshot.errors.length === 0 ? [] : encodeValue(snapshot.errors); })(snapshot);",
            ),
            (
                "const named = snapshot.signals.length === 0 ? [] : encodeValue(snapshot.signals, options);",
                "const named = (() => snapshot.signals.length === 0 ? [] : encodeValue(snapshot.signals, options))();",
            ),
        ],
    );
}

#[test]
fn payload_finalizer_does_not_rename_readable_bindings() {
    let human = POSITIVE.replace("$THREAD_5_9001", "encodeSignalBundle");
    assert_eq!(name_threads(&human, true).unwrap().source, human);
}
