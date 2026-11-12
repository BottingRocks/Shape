//! Whole-owner production recognition tests. This fixture is the unmodified
//! saved cleaned wrapper; offsets/hash are provenance only, never match keys.
use shape::naming::name_threads;
const BODY: &str = include_str!("fixtures/worker-environment-two-argument-owner.js");

fn alpha(s: &str) -> String {
    s.replace("$KEY_", "$LOCAL_").replace("$THREAD_", "$THREAD_88")
}
fn mutate(s: &str, from: &str, to: &str) -> String {
    assert_eq!(s.matches(from).count(), 1, "unique mutation: {from}");
    s.replacen(from, to, 1)
}
fn check(body: &str, accepted: bool) {
    let source = format!("function main(){{return 1;}} main();\n{body}");
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, &source, oxc_span::SourceType::unambiguous()).parse();
    assert!(!parsed.panicked && parsed.errors.is_empty(), "invalid fixture: {:?}", parsed.errors);
    let result = name_threads(&source, true).expect("production matcher");
    for (target, role) in [("$THREAD_6_134455", "createSeededWorkerEnvironmentBundle"), ("$THREAD_4_143124", "emitSeededWorkerEnvironmentBundle")] {
        let target = if body.contains("$THREAD_886_134455") {target.replace("$THREAD_", "$THREAD_88")} else {target.to_string()};
        let o = result.report.outcomes.iter().find(|o| o.binding.name == target).expect("target present");
        assert_eq!(o.applied && o.role_name.as_deref() == Some(role) && o.evaluator.as_deref() == Some("worker_environment_bundle"), accepted, "{target}: {body}");
        let r = result.report.function_roles.iter().find(|r| r.current_name.as_deref() == Some(target.as_str())).expect("role present");
        assert_eq!(r.evaluators.iter().any(|s| s == "worker_environment_bundle"), accepted);
    }
}
fn both(s: &str, accepted: bool) {check(s, accepted); check(&alpha(s), accepted);}

#[test]
fn whole_saved_two_argument_owner_and_alpha() { both(BODY, true); }

#[test]
fn equivalent_complete_source_forms() {
    for body in [
        mutate(BODY, "$KEY_8 = [[], $KEY_8];", "let postedPair = [[], $KEY_8];").replace("$G.b = $KEY_8;", "$G.b = postedPair;"),
        mutate(BODY, "$KEY_8[4] = $KEY_7() ^ 130;", "$KEY_8[3 + 1] = 130 ^ $KEY_7();"),
        mutate(BODY, "$KEY_8[4] = $KEY_7() ^ 130;", "$KEY_8[(4)] = ($KEY_7()) ^ (130);"),
    ] {both(&body, true);}
}

#[test]
fn publication_epochs_cursor_and_capture_refusals() {
    for (from,to) in [
        ("$KEY_8[4] =", "$KEY_8[3] ="),
        ("$E = 4;", "$E = 3;"),
        ("$E = $KEY_0($KEY_7, $E, $KEY_8, 3);", "$KEY_7 = $KEY_0($KEY_7, $E, $KEY_8, 3);"),
        ("$KEY_8 = [[], $KEY_8];", "$KEY_8 = [[], []];"),
        ("$KEY_8 = [[], $KEY_8];", "$KEY_8 = [[], $KEY_8]; $KEY_8 = [[], $KEY_8];"),
        ("$KEY_8 = [[], $KEY_8];", "$KEY_8 = [[], $KEY_8]; $KEY_8[1] = [];"),
        ("$G.b = $KEY_8;", "$G.b = $KEY_8; $G.b = [[], []];"),
        ("$F($G);", "extraEffect(); $F($G);"),
        ("$F($G);", "$F($G); extraEffect();"),
        ("$F($G);", "$F($G); $F($G);"),
        ("$F($G);", "$F({a: $KEY_12, b: [[], []]});"),
        ("$KEY_0 = $THREAD_6_21303;", "$KEY_0 = otherNumber;"),
        ("$KEY_18 = $THREAD_6_121017;", "$KEY_18 = otherStream;"),
        ("let $KEY_4 = $THREAD_1_1366;", "let $KEY_4 = otherWriter;"),
        ("$KEY_13 = Array.call.bind(String.prototype.charCodeAt);", "$KEY_13 = Array.call.bind(String.prototype.charAt);"),
        ("$KEY_0 = $KEY_16;", "$KEY_0 = 0;"),
        ("$KEY_9 = $KEY_7 && typeof Float64Array === \"function\" ? new Float64Array(1) : null;", "$KEY_9 = null;"),
        ("$KEY_11 = $KEY_9 ? null : [0, 0, 0, 0, 0, 0, 248, 127];", "$KEY_11 = $KEY_9 ? null : [0, 0, 0, 0, 0, 0, 0, 0];"),
        ("$KEY_16 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;", "$KEY_16 = 0;"),
        ("$KEY_7 = $KEY_0($KEY_7, $KEY_6(self), $KEY_8, $E);", "$KEY_7 = $KEY_0($KEY_7, $KEY_6(self), $KEY_8, 3);"),
        ("$KEY_5(location.origin, $KEY_8, $KEY_2);", "$KEY_5(location.origin, $KEY_8, $KEY_7);"),
        ("$KEY_8[4] = $KEY_7() ^ 130;", "$KEY_8[4] = otherStream() ^ 130;"),
    ] {both(&mutate(BODY, from, to), false);}
    for init in [
        "$KEY_16 = $C !== $B.a.length ? ($D << 5) - $D + ($B.a[$C].charCodeAt() << 16) | 0 : $D;",
        "$KEY_9 = $KEY_7 && typeof Float64Array === \"function\" ? new Float64Array(1) : null;",
        "$KEY_0 = $THREAD_6_21303;",
    ] {
        let late=mutate(BODY,init,"");
        both(&mutate(&late,"$THREAD_4_143124($B.b);",&format!("$THREAD_4_143124($B.b);{init}")),false);
    }
    let early=mutate(BODY,"$THREAD_4_143124($B.b);","");
    both(&mutate(&early,"let $KEY_4 = $THREAD_1_1366;","$THREAD_4_143124($B.b);let $KEY_4 = $THREAD_1_1366;"),false);
}

#[test]
fn exact_numeric_protocol_all_reset_arithmetic_and_port_mutants_refuse() {
    for (from,to) in [
        ("let $M = Math.abs($F);", "let $M = Math.abs($F); $M = 0;"),
        ("let $M = Math.abs($F);", "let $M = Math.abs($F) + 1;"),
        ("let $M = Math.abs($F);", "let $M = Math.abs($H);"),
        ("$Q * $S - $T < .5", "$Q * $S - $T < .25"),
        ("$Q * $S - $T > .5", "$Q * $S - $T > .25"),
        ("$T % 2", "$T % 3"),
        ("Math.min(Math.floor(Math.log($M) / Math.LN2), 1023)", "Math.min(Math.floor(Math.log($M) / Math.LN2), 1022)"),
        ("$R = ($R ? $P + 1 : $P) + 1023;", "$R = ($R ? $P + 1 : $P) + 1022;"),
        ("$O = ($T - $S) / $S >= 1;", "$O = ($T + $S) / $S >= 1;"),
        ("$O = $O ? 0 : $T - $S;", "$O = 0;"),
        ("$N = $O ? $R + 1 : $R;", "$N = 0;"),
        ("if ($N > 2046)", "if ($N > 2045)"),
        ("$N = 2047;", "$N = 2046;"),
        ("$O & 1", "$O & 2"),
        ("Number($U[j]) << 7", "Number($U[j]) << 6"),
        ("Math.pow(2, -1074)", "Math.pow(2, -1073)"),
        ("Math.pow(2, -1022)", "Math.pow(2, -1021)"),
        ("Math.pow(2, 52)", "Math.pow(2, 51)"),
        ("$G[$KEY_12] = $E() ^ 128;", "$G[$KEY_12] = $E() ^ 127;"),
        ("$KEY_9[0] = $F;", "$KEY_9[1] = $F;"),
        ("new $KEY_7($KEY_9.buffer)", "new $KEY_9($KEY_9.buffer)"),
        ("$L = $KEY_11;", "$L = $KEY_9;"),
        ("$L = new $KEY_7($KEY_9.buffer);", "$L = new $KEY_7($KEY_11.buffer);"),
        ("$G[++$KEY_12] = $L[i_2] ^ $E();", "$G[$KEY_12] = $L[i_2] ^ $E();"),
        ("for (; i_2 < 8; i_2++)", "for (; i_2 < 7; i_2++)"),
        ("$J = 64;", "$J = 65;"),
        ("$K < 4096", "$K < 4095"),
        ("$G[++$KEY_12] = $K >>> 12 ^ $E();", "$G[++$KEY_12] = $K >>> 13 ^ $E();"),
        ("let $M = Math.abs($F);", "let Math = {}; let $M = Math.abs($F);"),
    ] {both(&mutate(BODY, from, to), false);}
}
