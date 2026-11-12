use shape::naming::name_threads;

const KEYDOWN: &str = "recordKeydownTimingSample";
const KEYUP: &str = "recordKeyupHoldStatistics";

const BASE_KEYDOWN: &str = r#"function main(){let state=Object.create(null);let first=0;let previous=0;let total=0;let overflow=0;let samples=0;
function $THREAD_1_10(event){let clock=Date.now();state[event.keyCode]=clock;if(first===0){first=clock;}else{clock=previous?clock-previous:0;if(clock<2e3){total=total+clock;++samples;}else{overflow=overflow+clock;}}}
return $THREAD_1_10;}main();"#;

const BASE_KEYUP: &str = r#"function main(){let store=Object.create(null);let count=0;let mean=0;let moment=0;let first=0;let keys=[];let counts=[];let seen=0;let active=false;
function $THREAD_1_11(event){let clock=Date.now();let press=store[event.keyCode];++count;if(press){let delta=mean;mean=mean+(clock-press-delta)/count;moment=moment+(clock-press-delta)*(clock-press-mean);}first=clock;let index=keys.indexOf(event.keyCode);if(index!==-1){++seen;counts[index]=Number(counts[index])+1;}store[event.keyCode]=null;active=true;}
return $THREAD_1_11;}main();"#;

fn named(source: &str, role: &str) -> bool {
    name_threads(source, true)
        .unwrap_or_else(|e| panic!("{e}: {source}"))
        .report
        .outcomes
        .iter()
        .any(|o| o.applied && o.role_name.as_deref() == Some(role))
}

fn replay_ok(source: &str) {
    let result = name_threads(source, true).unwrap();
    let mut replay = source.to_owned();
    for edit in result.report.edits.iter().rev() {
        replay.replace_range(edit.start as usize..edit.end as usize, &edit.replacement);
    }
    assert_eq!(replay, result.source);
}

#[test]
fn keydown_and_keyup_positive_variants() {
    // `let`/`var`/`const` declarations and an extra copy of the same shape in
    // another thread both keep the complete owned-body proof.
    for source in [
        BASE_KEYDOWN.to_owned(),
        BASE_KEYDOWN.replace("let state", "var state").replace("let clock", "var clock"),
        BASE_KEYDOWN.replace("2e3", "2000"),
        BASE_KEYUP.to_owned(),
        BASE_KEYUP.replace("let store", "const store"),
        BASE_KEYUP.replace("index!==-1", "index !== -1"),
    ] {
        let role = if source.contains("$THREAD_1_10") { KEYDOWN } else { KEYUP };
        assert!(named(&source, role), "{source}");
        replay_ok(&source);
    }
}

#[test]
fn exact_owned_and_order_rejections() {
    for (from, to) in [
        // keydown: threshold, comparison, clock origin, key spelling, counter
        ("2e3", "2e4"),
        ("clock<2e3", "clock<=2e3"),
        ("Date.now", "Date.UTC"),
        ("event.keyCode]=clock", "event.which]=clock"),
        ("++samples", "--samples"),
        ("if(first===0)", "if(first!==0)"),
        ("total=total+clock", "total=total-clock"),
        ("overflow=overflow+clock", "overflow=overflow-clock"),
        ("clock=previous?clock-previous:0", "clock=previous?clock+previous:0"),
        // keyup: mean/second-moment arithmetic, lookup comparison, clear value,
        // active flag and tracked-key source
        ("mean+(clock-press-delta)/count", "mean+(clock-press-delta)*count"),
        ("moment=moment+", "first=moment+"),
        ("index!==-1", "index!==0"),
        ("store[event.keyCode]=null", "store[event.keyCode]=0"),
        ("active=true", "active=false"),
        ("++count", "++seen"),
        ("keys.indexOf(event.keyCode)", "counts.indexOf(event.keyCode)"),
        ("if(press)", "if(!press)"),
    ] {
        let (base, role) = if BASE_KEYDOWN.contains(from) {
            (BASE_KEYDOWN, KEYDOWN)
        } else {
            assert!(BASE_KEYUP.contains(from), "mutation source not found: {from}");
            (BASE_KEYUP, KEYUP)
        };
        assert!(!named(&base.replace(from, to), role), "accepted {from} -> {to}");
    }
}

#[test]
fn captured_participants_must_be_outside_the_owned_body() {
    // Moving a captured participant into the owned body removes the
    // plain-origin-before-use proof.
    let keydown = BASE_KEYDOWN.replace(
        "function $THREAD_1_10(event){",
        "function $THREAD_1_10(event){let previous=0;",
    );
    assert!(!named(&keydown, KEYDOWN));
    let keyup = BASE_KEYUP.replace(
        "function $THREAD_1_11(event){",
        "function $THREAD_1_11(event){let keys=[];",
    );
    assert!(!named(&keyup, KEYUP));
}
