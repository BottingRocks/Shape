use shape::naming::name_threads;
#[path="naming_software_float_byte_writer_family.rs"] mod inherited;
const RULE:&str="shared_software_float_byte_writer_protocol";
const BODIES:[&str;12]=[
 include_str!("fixtures/software_float_byte_writer/computed-exponent-nested-rounding.js"),include_str!("fixtures/software_float_byte_writer/computed-exponent-nested-rounding-alternate.js"),
 include_str!("fixtures/software_float_byte_writer/computed-exponent-compact-rounding.js"),include_str!("fixtures/software_float_byte_writer/literal-exponent-compact-rounding.js"),
 include_str!("fixtures/software_float_byte_writer/computed-exponent-baseline.js"),include_str!("fixtures/software_float_byte_writer/computed-exponent-alternate.js"),
 include_str!("fixtures/software_float_byte_writer/literal-exponent-baseline.js"),include_str!("fixtures/software_float_byte_writer/literal-exponent-alternate.js"),
 include_str!("fixtures/software_float_byte_writer/exponent-first-baseline.js"),include_str!("fixtures/software_float_byte_writer/exponent-first-alternate.js"),
 include_str!("fixtures/software_float_byte_writer/literal-exponent-heldout-baseline.js"),include_str!("fixtures/software_float_byte_writer/literal-exponent-heldout-alternate.js")];
fn body(i:usize)->String {
 let b=BODIES[i]; let start=b.find('(').unwrap();
 format!("function $THREAD_5_12345{}",&b[start..])
}
fn capture(i:usize)-> &'static str { ["$KEY_125","$KEY_125","$KEY_125","$KEY_311","$KEY_88","$KEY_88","$KEY_26","$KEY_26","$KEY_200","$KEY_200","$KEY_151","$KEY_151"][i] }
fn wrapped(i:usize,b:&str,setup:&str,tail:&str)->String {
 let t=capture(i);
 format!("function main(){{let {t};{setup}{b}{tail}}}main();")
}
fn store(i:usize)->String{format!("{}={{FLOAT32:236,DOUBLE64:237}};",capture(i))}
fn outcome(s:&str)->shape::naming::ThreadName {
 name_threads(s,true).expect("source parses").report.outcomes.into_iter().find(|o|o.binding.name=="$THREAD_5_12345").expect("$THREAD_5_12345 binding")
}
#[test]fn actual_bodies_and_alpha(){
 for i in 0..12 {
  let b=body(i);
  for s in [b.clone(), b.replace("$GP","inputValue").replace("$GQ","sinkValue").replace("$KEY_5","signValue")] {
   let o=outcome(&wrapped(i,&s,&store(i),"return $THREAD_5_12345;"));
   assert_eq!(o.evaluator.as_deref(),Some(RULE),"variant {i}: {o:?}"); assert!(o.applied);
  }
 }
}
#[test]fn complete_arithmetic_mutations_refused(){
 for i in 0..12 {
  let b=body(i);
  for (a,z) in [("536870911","536870912"),(".5",".25"),("% 2","% 3"),("1 / $GP < 0","1 / $GP > 0"),("Math.LN2","Math.PI"),("Math.pow(2, 52)","Math.pow(2, 51)"),("1023","1022"),("<< 7","<< 6"),("unshift","push"),("i + 8","i + 7"),("+= 8","+= 7"),("DOUBLE64","FLOAT32"),("/ 2)","/ 3)"),("* 2","* 3")] {
   let changed=b.replace(a,z); if changed==b {continue;}
   let o=outcome(&wrapped(i,&changed,&store(i),"return $THREAD_5_12345;"));
   assert_ne!(o.evaluator.as_deref(),Some(RULE),"variant {i}, mutation {a}: {o:?}");assert!(!o.applied,"{o:?}");
  }
 }
}
#[test]fn all_capture_writers_and_early_exposure_refused(){
 for i in 0..12 {
  let b=body(i);let t=capture(i);let init=store(i);
  for (setup,tail) in [
   (String::new(),"return $THREAD_5_12345;".into()),
   (format!("let Math=foreign;{init}"),"return $THREAD_5_12345;".into()),
   (format!("register($THREAD_5_12345);{init}"),"return $THREAD_5_12345;".into()),
   (format!("if(flag){{{init}}}"),"return $THREAD_5_12345;".into()),
   (init.clone(),format!("{t}={{FLOAT32:0,DOUBLE64:237}};return $THREAD_5_12345;")),
   (init.clone(),format!("{t}.FLOAT32=0;return $THREAD_5_12345;")),
   (init.clone(),format!("escape({t});return $THREAD_5_12345;")),
   (init.clone(),format!("{t}={{FLOAT32:236,DOUBLE64:238}};return $THREAD_5_12345;")),
  ] {
   let o=outcome(&wrapped(i,&b,&setup,&tail));assert!(!o.applied,"variant {i}: {o:?}");
  }
  let o=outcome(&wrapped(i,&b,&format!("{init}{init}{init}"),"return $THREAD_5_12345;"));
  assert_eq!(o.evaluator.as_deref(),Some(RULE),"repeated stores variant {i}: {o:?}");
 }
}

// Whole-source and production routing must exercise the arithmetic protocol so isolated
// bodies cannot claim support absent from their actual lowering.
#[test]
fn float_encoding_whole_source_production_and_nonvacuous_alpha() {
 let original=include_str!("fixtures/software_float_byte_writer/whole-scope.js");
 let alpha=original.replace("$AW","$RENAMED_INPUT").replace("$AX","$RENAMED_OUTPUT").replace("$KEY_88","$RENAMED_TAGS");
 assert_ne!(alpha,original);
 for source in [original,alpha.as_str()] {
  let report=name_threads(source,true).expect("whole source parses").report;
  for name in ["$THREAD_0_122622","$THREAD_0_122622_2"] {
   let o=report.outcomes.iter().find(|o|o.binding.name==name).expect("actual binding");
   assert_eq!(o.evaluator.as_deref(),Some(RULE),"{name}: {o:?}");
   assert!(o.applied,"{o:?}");
  }
 }
}
#[test]
fn float_encoding_rounding_overflow_byteorder_buffer_and_cursor_refusals() {
 let original=body(4);
 for (from,to) in [
  ("let $AY = Math.abs($AW);","let $AY = Math.abs($AW); $AY=0;"),
  ("1 / $AW < 0","1 / $AW > 0"),
  (".5",".25"),("% 2","% 3"),("2047","2046"),
  ("$AZ / 536870911","$AZ / 536870912"),
  ("<< 7","<< 6"),("$AX.push($BG)","foreign.push($BG)"),
  ("$AX.push($BK[","foreign.push($BK["),("i + 8","i + 7"),
 ] {
  assert!(original.contains(from),"nonvacuous mutation {from}");
  let changed=original.replace(from,to);
  for b in [changed.clone(),changed.replace("$AW","$INPUT").replace("$AX","$OUTPUT")] {
   let o=outcome(&wrapped(4,&b,&store(4),"return $THREAD_5_12345;"));
   assert!(!o.applied,"mutation {from}: {o:?}");
  }
 }
}
#[test]
fn float_encoding_table_epochs_cannot_borrow_foreign_or_later_stores() {
 let b=body(4);let t=capture(4);let good=store(4);
 for (setup,tail) in [
  (format!("register($THREAD_5_12345);{good}"),"return $THREAD_5_12345;".to_string()),
  (format!("let alias={t};{good}"),"return $THREAD_5_12345;".into()),
  (format!("{t}={{FLOAT32:0,DOUBLE64:237}};let foreign={{FLOAT32:236,DOUBLE64:237}};"),"return $THREAD_5_12345;".into()),
  (good.clone(),format!("let alias={t};alias.FLOAT32=0;return $THREAD_5_12345;")),
  (format!("let alias=$THREAD_5_12345;register(alias);{good}"),"return $THREAD_5_12345;".into()),
  (format!("{good}function reenter(){{{t}={{FLOAT32:236,DOUBLE64:238}};}}"),"return $THREAD_5_12345;".into()),
 ] {
  let o=outcome(&wrapped(4,&b,&setup,&tail));assert!(!o.applied,"{o:?}");
 }
 let o=outcome(&wrapped(4,&b,&format!("{good}{good}"),"return $THREAD_5_12345;"));
 assert_eq!(o.evaluator.as_deref(),Some(RULE));assert!(o.applied);
}
