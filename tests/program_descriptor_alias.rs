use shape::signals::extract::scanner::extract_program_properties;

#[test]
fn registry_captures_array_before_scratch_is_reused() {
    let source = r#"function main(){
      let registry; let scratch = 0;
      scratch = [{ctor: null, priority: false}, {ctor: null, priority: true}];
      registry = scratch;
      scratch = {unrelated: true};
      registry.forEach(entry => { let task = entry.ctor; if(task) task(); });
      return registry.length;
    } main();"#;
    let report = extract_program_properties(source).unwrap();
    let registry = report.descriptors.unwrap();
    assert_eq!(registry.length, 2);
    assert_eq!(registry.binding.name, "registry");
    assert_eq!(registry.elements.len(), 2);
}

#[test]
fn branch_dependent_registry_capture_is_not_guessed() {
    let source = r#"function main(){
      let registry; let scratch = 0;
      if(unknown) scratch = [{ctor: null, priority: false}];
      registry = scratch;
      scratch = {unrelated: true};
      registry.forEach(entry => { let task = entry.ctor; if(task) task(); });
      return registry.length;
    } main();"#;
    let report = extract_program_properties(source).unwrap();
    assert!(report.descriptors.is_none());
}

#[test]
fn two_aliases_of_the_same_literal_are_one_registry() {
    let source = r#"function main(){
      let scratch = [{ctor: null, priority: false}];
      let registry = scratch;
      registry.forEach(entry => { let task = entry.ctor; if(task) task(); });
      return registry.length;
    } main();"#;
    let report = extract_program_properties(source).unwrap();
    assert_eq!(report.descriptors.unwrap().length, 1);
}

#[test]
fn captured_registry_retains_population_before_scratch_reuse() {
    let source = r#"function main(){
      let registry;let scratch=0;
      scratch=[{ctor:null,priority:false}];
      scratch[1]={ctor:null,name:"extra",priority:false};
      registry=scratch;
      scratch={unrelated:true};scratch[9]={ctor:null,name:"neighbor"};
      registry.forEach(entry=>{let task=entry.ctor;if(task)task();});
      return registry.length;
    }main();"#;
    let registry=extract_program_properties(source).unwrap().descriptors.unwrap();
    assert_eq!(registry.length,2);
    assert_eq!(registry.elements[1].name,Some(shape::signals::schema::LiteralValue::String("extra".into())));
}

#[test]
fn constructor_value_survives_an_unrelated_loop() {
    let source = r#"function main(){
      function wrap(task){return function(obs){task(obs);};}
      let held=wrap(function task(obs){obs.next([[],[1,2,3,4]]);});
      for(let i=0;i<3;i++){unknown(i);}
      let registry=[{ctor:held,priority:false}];
      registry.forEach(entry=>{let task=entry.ctor;if(task)task();});
      return registry.length;
    }main();"#;
    let wiring=shape::signals::extract::wiring::extract_program_wiring(source).unwrap();
    assert_eq!(wiring.direct_routes.len(),1);
    assert!(wiring.direct_routes[0].ctor.callback.is_some());
}

#[test]
fn constructor_changed_in_a_loop_is_not_guessed() {
    let source = r#"function main(){
      function wrap(task){return function(obs){task(obs);};}
      let held=wrap(function task(obs){obs.next([[],[1,2,3,4]]);});
      for(let i=0;i<3;i++){held=other(i);}
      let registry=[{ctor:held,priority:false}];
      registry.forEach(entry=>{let task=entry.ctor;if(task)task();});
      return registry.length;
    }main();"#;
    let wiring=shape::signals::extract::wiring::extract_program_wiring(source).unwrap();
    assert!(wiring.direct_routes[0].ctor.callback.is_none());
}

#[test]
fn population_reads_each_scratch_object_before_reuse() {
    let source = r#"function main(){
      let registry;let scratch=[{ctor:null}];let entry;
      entry={ctor:function first(){},name:"first"};scratch[0]=entry;
      entry={ctor:null,name:"second"};scratch[1]=entry;
      registry=scratch;
      registry.forEach(entry=>{let task=entry.ctor;if(task)task();});
      return registry.length;
    }main();"#;
    let registry=extract_program_properties(source).unwrap().descriptors.unwrap();
    assert_eq!(registry.elements.len(),2);
    assert_eq!(registry.elements[0].name,Some(shape::signals::schema::LiteralValue::String("first".into())));
    assert_eq!(registry.elements[1].name,Some(shape::signals::schema::LiteralValue::String("second".into())));
}

#[test]
fn nested_registry_read_retains_outer_constructor_definition() {
    let source = r#"function main(){
      function wrap(task){return function(obs){task(obs);};}
      let held=wrap(function task(obs){obs.next([[],[1,2,3,4]]);});
      let registry;
      if(unknown){registry=[{ctor:held,priority:false}];}
      registry.forEach(entry=>{let task=entry.ctor;if(task)task();});
      return registry.length;
    }main();"#;
    let wiring=shape::signals::extract::wiring::extract_program_wiring(source).unwrap();
    assert_eq!(wiring.direct_routes.len(),1);
    assert!(wiring.direct_routes[0].ctor.callback.is_some());
}

#[test]
fn returning_setup_keeps_its_selector_before_later_scratch_writes() {
    let source=r#"function main(){
      function wave(obs){obs[0].next([[],[1,2,3,4]]);}
      let descriptors=[{ctor:null,priority:false}];
      let functions=[wave];let rows=[[0]];let flags;
      if(unknown){flags=[false];function run(){for(let i=0;i<rows.length;i++){functions[i](rows[i],flags[i]);}}return run;}
      flags=2;return flags;
    }main();"#;
    let report=extract_program_properties(source).unwrap();
    assert!(report.waves.is_some());
}

#[test]
fn nonreturning_setup_does_not_hide_a_later_selector_write() {
    let source=r#"function main(){
      function wave(obs){obs[0].next([[],[1,2,3,4]]);}
      let descriptors=[{ctor:null,priority:false}];
      let functions=[wave];let rows=[[0]];let flags;
      if(unknown){flags=[false];function run(){for(let i=0;i<rows.length;i++){functions[i](rows[i],flags[i]);}}}
      flags=2;return flags;
    }main();"#;
    let report=extract_program_properties(source).unwrap();
    assert!(report.waves.is_none());
}
