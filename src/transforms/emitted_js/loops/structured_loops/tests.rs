use super::*;
use std::process::Command;

#[test]
#[ignore = "requires SHAPE_LOOP_AUDIT_SOURCE and SHAPE_LOOP_AUDIT_OUT"]
fn inventory_remaining_loops() {
    struct Audit<'s> {
        source: &'s str,
        out: std::path::PathBuf,
        functions: Vec<String>,
        count: usize,
        index: String,
    }
    impl Audit<'_> {
        fn outline(&self, statement: &Statement<'_>, indent: usize, lines: &mut String) {
            let text =
                &self.source[statement.span().start as usize..statement.span().end as usize];
            let line = self.source[..statement.span().start as usize]
                .bytes()
                .filter(|b| *b == b'\n')
                .count()
                + 1;
            let header = text.lines().next().unwrap_or("");
            lines.push_str(&format!(
                "{}L{} {}\n",
                "  ".repeat(indent),
                line,
                header.chars().take(155).collect::<String>()
            ));
            match statement {
                Statement::BlockStatement(block) => {
                    for s in &block.body {
                        self.outline(s, indent + 1, lines);
                    }
                }
                Statement::IfStatement(branch) => {
                    self.outline(&branch.consequent, indent + 1, lines);
                    if let Some(other) = &branch.alternate {
                        lines.push_str(&format!("{}ELSE\n", "  ".repeat(indent)));
                        self.outline(other, indent + 1, lines);
                    }
                }
                Statement::TryStatement(stmt) => {
                    for s in &stmt.block.body {
                        self.outline(s, indent + 1, lines);
                    }
                    if let Some(catch) = &stmt.handler {
                        lines.push_str("CATCH\n");
                        for s in &catch.body.body {
                            self.outline(s, indent + 1, lines);
                        }
                    }
                    if let Some(finally) = &stmt.finalizer {
                        lines.push_str("FINALLY\n");
                        for s in &finally.body {
                            self.outline(s, indent + 1, lines);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    impl<'a> Visit<'a> for Audit<'_> {
        fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
            self.functions.push(
                f.id.as_ref()
                    .map_or("anonymous".into(), |id| id.name.to_string()),
            );
            walk::walk_function(self, f, flags);
            self.functions.pop();
        }
        fn visit_labeled_statement(&mut self, labeled: &LabeledStatement<'a>) {
            if let Statement::ForStatement(loop_) = &labeled.body {
                if labeled.label.name.starts_with("vm_loop_")
                    && loop_.test.is_none()
                    && loop_.init.is_none()
                    && loop_.update.is_none()
                {
                    self.count += 1;
                    let line = self.source[..labeled.span.start as usize]
                        .bytes()
                        .filter(|b| *b == b'\n')
                        .count()
                        + 1;
                    let name = format!(
                        "{:02}-{}-{}",
                        self.count,
                        self.functions.last().unwrap(),
                        labeled.label.name
                    );
                    self.index.push_str(&format!(
                        "{}\t{}\t{}\t{}\n",
                        self.count,
                        line,
                        self.functions.last().unwrap(),
                        labeled.label.name
                    ));
                    std::fs::write(
                        self.out.join(format!("{name}.js")),
                        &self.source[labeled.span.start as usize..labeled.span.end as usize],
                    )
                    .unwrap();
                    let mut outline = String::new();
                    self.outline(&loop_.body, 0, &mut outline);
                    std::fs::write(self.out.join(format!("{name}.txt")), outline).unwrap();
                }
            }
            walk::walk_labeled_statement(self, labeled);
        }
    }
    let source =
        std::fs::read_to_string(std::env::var_os("SHAPE_LOOP_AUDIT_SOURCE").unwrap()).unwrap();
    let out = std::path::PathBuf::from(std::env::var_os("SHAPE_LOOP_AUDIT_OUT").unwrap());
    std::fs::create_dir_all(&out).unwrap();
    let allocator = Allocator::default();
    let program = analysis::parse_program(&allocator, &source).unwrap();
    let mut audit = Audit {
        source: &source,
        out,
        functions: Vec::new(),
        count: 0,
        index: String::new(),
    };
    audit.visit_program(&program);
    std::fs::write(audit.out.join("index.tsv"), &audit.index).unwrap();
    eprintln!("{}", audit.index);
}

fn execute(source: &str) -> String {
    let result = Command::new("node")
        .args(["-e", &format!("{source};console.log(JSON.stringify(f()));")])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{source}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

fn equivalent(source: &str, count: usize) -> String {
    let (code, changed) = reconstruct_loops(source, &BTreeSet::new()).unwrap();
    assert_eq!(changed, count, "{code}");
    assert_eq!(execute(source), execute(&code), "{code}");
    code
}

#[test]
fn counted_loop_folds_var_and_local_let_initializers() {
    for kind in ["var", "let"] {
        let source = format!("function f(){{const out=[]; {kind} i=0;vm_loop_0:for(;;){{if(i<4){{out.push(i);i=i+1;continue vm_loop_0;}}else{{out.push('done');}}break vm_loop_0;}}return out;}}");
        let code = equivalent(&source, 1);
        assert!(code.contains(&format!("for ({kind} i=0;")), "{code}");
        assert!(!code.contains("vm_loop_0:"));
    }
}

#[test]
fn condition_getters_and_exit_scope_keep_their_order() {
    let source = "function f(){const log=[];let i=0;const a={get length(){log.push('test'+i);return 3;}};vm_loop_1:for(;;){let $SSA_1=a.length;if(i<$SSA_1){log.push(i);i=i+1;continue vm_loop_1;}else{let value=$SSA_1;log.push(value);}break vm_loop_1;}return [i,log];}";
    let code = equivalent(source, 1);
    assert!(code.contains("$SSA_1 = (a.length)"), "{code}");
    // i is observed after the loop, so its declaration stays outside.
    assert!(code.contains("for (;"));
    equivalent("function f(){let $SSA_1=2;var i=$SSA_1;vm_loop_0:for(;;){let $SSA_1=4;if(i<$SSA_1){i++;continue vm_loop_0;}else{return i;}}}", 1);
}

#[test]
fn while_loop_reverses_the_continue_arm() {
    let source = "function f(){let i=0;const log=[];vm_loop_2:for(;;){if(i>=3){log.push('end');return log;}else{log.push(i++);continue vm_loop_2;}}}";
    let code = equivalent(source, 1);
    assert!(code.contains("while (!"), "{code}");
}

#[test]
fn search_loops_keep_early_return_and_drop_local_continue_labels() {
    let source = "function f(){let i=0;vm_loop_0:for(;;){if(i<5){if(i===3){return i;}else{i++;continue vm_loop_0;}}else{return null;}}}";
    let code = equivalent(source, 1);
    assert!(code.contains("while"), "{code}");
    assert!(!code.contains("vm_loop_0"), "{code}");
    equivalent("function f(){let i=0;const log=[];vm_loop_0:for(;;){if(i<3){if(i===1){i++;continue vm_loop_0;}else{log.push(i++);continue vm_loop_0;}}else{return log;}}}", 1);
}

#[test]
fn loop_body_break_must_skip_exit_continuation() {
    equivalent("function f(){let i=0;const log=[];vm_loop_0:for(;;){if(i<4){if(i===2)break vm_loop_0;log.push(i++);continue vm_loop_0;}else{log.push('exit');}break vm_loop_0;}return log;}", 0);
    equivalent("function f(){let i=0;vm_loop_0:for(;;){if(i<4){if(i++===2)break;continue vm_loop_0;}else{return 9;}break vm_loop_0;}return i;}", 0);
}

#[test]
fn nested_jumps_keep_targets_and_early_continue_skips_update() {
    let source = "function f(){let i=0;const log=[];vm_loop_0:for(;;){if(i<4){i++;for(let k=0;k<2;k++){if(k===1)continue vm_loop_0;log.push(i);}i=i+1;continue vm_loop_0;}else{log.push('end');}break vm_loop_0;}return log;}";
    let code = equivalent(source, 1);
    assert!(code.contains("vm_loop_0: while"), "{code}");
    assert!(!code.contains("for (;"));
    equivalent("function f(){let i=0;const log=[];vm_loop_0:for(;;){if(i<3){switch(i){case 1:break;default:log.push(i);}for(let j=0;j<2;j++){if(j)break;}i++;continue vm_loop_0;}else{}break vm_loop_0;}return log;}", 1);
}

#[test]
fn captured_prefix_and_tdz_remain_per_iteration() {
    equivalent("function f(){let i=0;const saved=[];vm_loop_0:for(;;){let $SSA_1=i;if(i<3){saved.push(()=>$SSA_1);i++;continue vm_loop_0;}else{return saved.map(f=>f());}}}", 0);
    equivalent("function f(){try{vm_loop_0:for(;;){let $SSA_1=$SSA_1;if(false){continue vm_loop_0;}else{return 1;}}}catch(e){return e.name;}}", 0);
    equivalent("function f(){try{vm_loop_0:for(;;){let $SSA_1=$SSA_2;let $SSA_2=1;if(false){continue vm_loop_0;}else{return 1;}}}catch(e){return e.name;}}", 0);
}

#[test]
fn captured_counter_keeps_single_original_cell() {
    let source = "function f(){const saved=[];let i=0;vm_loop_0:for(;;){if(i<3){saved.push(()=>i);i++;continue vm_loop_0;}else{}break vm_loop_0;}return saved.map(f=>f());}";
    let code = equivalent(source, 1);
    assert!(!code.contains("for (let"), "{code}");
    let code=equivalent("function f(){const saved=[];let i=0;vm_loop_0:for(;;){if(i<3){saved.push(class{x=i;});i++;continue vm_loop_0;}else{}break vm_loop_0;}return saved.map(C=>new C().x);}",1);
    assert!(!code.contains("for (let"), "{code}");
}

#[test]
fn prefix_closures_try_finally_and_body_local_updates() {
    equivalent("function f(){let i=0;const log=[];vm_loop_0:for(;;){if(i<3){try{log.push(i);}finally{log.push('finally');}i++;continue vm_loop_0;}else{return log;}}}", 1);
    let code = equivalent("function f(){let i=0;vm_loop_0:for(;;){if(i<3){let step=1;i=i+step;continue vm_loop_0;}else{return i;}}}", 1);
    assert!(code.contains("while"), "{code}");
}

#[test]
fn nested_loops_reconstruct_inside_out() {
    equivalent("function f(){const log=[];let i=0;vm_loop_0:for(;;){if(i<2){let j=0;vm_loop_1:for(;;){if(j<2){log.push([i,j]);j++;continue vm_loop_1;}else{}break vm_loop_1;}i++;continue vm_loop_0;}else{return log;}}}", 2);
    equivalent("function f(){const log=[];let i=0;vm_loop_0:for(;;){if(i<2){log.push(i++);continue vm_loop_0;}else{let j=0;vm_loop_1:for(;;){if(j<2){log.push(j++);continue vm_loop_1;}else{return log;}}}}}", 2);
    equivalent("function f(){let i=0;vm_loop_0:for(;;){if(i++<1){continue vm_loop_0;}else{vm_loop_1:for(;;){break vm_loop_1;}if(i>3)return i;}}}", 0);
    equivalent("function f(){const log=[];let i=0;outer:while(i<2){let j=0;vm_loop_0:for(;;){if(j<2){log.push([i,j++]);continue vm_loop_0;}else{i++;continue outer;}}}return log;}", 1);
}

#[test]
fn shared_lexical_counter_keeps_its_owner_declaration() {
    let source = "function f(){let $KEY_7=0;vm_loop_0:for(;;){if($KEY_7<3){$KEY_7++;continue vm_loop_0;}else{}break vm_loop_0;}}";
    let (code, count) = reconstruct_loops(source, &BTreeSet::from(["$KEY_7".into()])).unwrap();
    assert_eq!(count, 1);
    assert!(code.contains("let $KEY_7=0;"), "{code}");
    assert!(!code.contains("for (let"), "{code}");
}

#[test]
fn suffix_after_guard_runs_only_when_the_loop_finishes() {
    let code=equivalent("function f(){let i=0;const log=[];vm_loop_0:for(;;){if(i<3){log.push(i);i++;continue vm_loop_0;}log.push(['exit',i]);break vm_loop_0;}return log;}",1);
    assert!(code.contains("for (;"), "{code}");
    assert!(!code.contains("vm_loop_0"));
}

#[test]
fn side_effecting_prefix_preserves_getter_and_slot_order() {
    equivalent("function f(){let i=0;const log=[];const obj={get test(){log.push(['test',i]);return i<3;}};vm_loop_0:for(;;){let $SSA_1=i;log.push(['prefix',$SSA_1]);let $SSA_2=obj.test;if($SSA_2){i++;continue vm_loop_0;}else{return [log,$SSA_1];}}}",1);
    let code=equivalent("function f(){let i=0;const log=[];vm_loop_0:for(;;){if(i<1){log.push('fill');i++;}let $SSA_1=i++;if($SSA_1<4){continue vm_loop_0;}else{return [i,log,$SSA_1];}}}",1);
    assert!(code.contains("while"), "{code}");
}

#[test]
fn search_join_preserves_found_and_exhausted_values() {
    equivalent("function f(){const out=[];for(const target of [2,8]){let i=0;vm_loop_0:for(;;){let $SSA_1=i<4;let $CARRY_1;if($SSA_1){if(i===target){$CARRY_1=i;}else{i++;continue vm_loop_0;}}else{$CARRY_1=-1;}out.push([$CARRY_1,i]);break vm_loop_0;}}return out;}",1);
    // The exhausted default is read at exit, after every mutation of obj.
    equivalent("function f(){let i=0,obj={value:0};vm_loop_0:for(;;){let $CARRY_1;if(i<3){if(i===9){$CARRY_1=obj;}else{i++;obj={value:i};continue vm_loop_0;}}else{$CARRY_1=obj;}return [$CARRY_1.value,i];}}",1);
}

#[test]
fn shared_exit_does_not_change_conditional_continue_paths() {
    equivalent("function f(){let i=0;const log=[];vm_loop_0:for(;;){if(i++<5){if(i<3){continue vm_loop_0;}}if(i>0){log.push(i);}return log;}}",1);
    equivalent("function f(){let i=0,j=0;const log=[];outer:while(i<3){j=0;vm_loop_0:for(;;){if(j<3){if(i+j<3){log.push([i,j++]);continue vm_loop_0;}}log.push('next');i++;continue outer;}}return log;}",1);
}

#[test]
fn process_then_test_encoders_use_do_while_without_extra_coercions() {
    let code=equivalent("function f(){const result=[];for(const input of [0,1,127,128,129,16384,4294967295]){let value=input,index=-1,low=value&127;let $CARRY_out;const bytes=[];vm_loop_0:for(;;){let $SSA_1=index+1;let $SSA_2=low|128;bytes[$SSA_1]=$SSA_2;let $SSA_3=value&127;let $SSA_4=value>>>7;if($SSA_4>0){index=$SSA_1;value=$SSA_4;low=$SSA_3;continue vm_loop_0;}else{bytes[$SSA_1+1]=$SSA_3;}$CARRY_out=bytes;break vm_loop_0;}result.push($CARRY_out);}return result;}",1);
    assert!(code.contains("do {"), "{code}");
    assert!(!code.contains("for (;;"), "{code}");
}

#[test]
fn conditional_prefix_initializes_join_cells_each_iteration() {
    equivalent("function f(){let i=0;const log=[];vm_loop_0:for(;;){let $SSA_1=i;let $CARRY_1;if(i%2){let $SSA_2=i*2;$CARRY_1=$SSA_2;}else{$CARRY_1=i+1;}log.push([$SSA_1,$CARRY_1]);if(i<3){i++;continue vm_loop_0;}else{return log;}}}",1);
    equivalent("function f(){let i=0;const log=[];vm_loop_0:for(;;){let $CARRY_1;if(i===0)$CARRY_1=9;log.push($CARRY_1===undefined?'undefined':$CARRY_1);if(i++<2){continue vm_loop_0;}else{return log;}}}",1);
}

#[test]
fn catch_backedges_keep_their_finally_order() {
    equivalent("function f(){let i=0;const log=[];vm_loop_0:for(;;){if(i<4){try{if(i%2)throw i;log.push('try');i++;continue vm_loop_0;}catch(e){log.push(['catch',e]);i++;continue vm_loop_0;}finally{log.push(['finally',i]);}}else{return log;}}}",1);
}

#[test]
fn decoder_validation_guards_keep_inner_and_outer_continue_targets() {
    equivalent("function f(){const log=[];let i=0;outer:while(i<3){let j=0;vm_loop_0:for(;;){let $SSA_1=j;let $CARRY_1;if(j%2){$CARRY_1=j+1;}else{$CARRY_1=j;}if($CARRY_1>4){throw Error('bad');}else{if(j>=i){log.push([i,j,$SSA_1]);i++;continue outer;}else{j++;continue vm_loop_0;}}}}return log;}",1);
}

#[test]
fn conditional_declaration_lifting_rejects_capture_and_shadowing() {
    equivalent("function f(){let i=0;const saved=[];vm_loop_0:for(;;){let $SSA_1=i;if(i<3){saved.push(class{x=$SSA_1;});i++;continue vm_loop_0;}else{return saved.map(C=>new C().x);}}}",0);
    equivalent("function f(){let i=0;const log=[];vm_loop_0:for(;;){let $CARRY_1;if(i<2){let $SSA_1=i;log.push(()=>$SSA_1);$CARRY_1=i;}else{$CARRY_1=0;}if(i++<2){continue vm_loop_0;}else{return log.map(f=>f());}}}",0);
    equivalent("function f(){let $SSA_1=7,i=0;const log=[];vm_loop_0:for(;;){if(i<1){let $SSA_1=2;log.push($SSA_1);}log.push($SSA_1);if(i++<1){continue vm_loop_0;}else{return log;}}}",0);
    equivalent("function f(){let i=0;try{vm_loop_0:for(;;){let $SSA_1=$SSA_2;let $SSA_2=1;if(i++<1){continue vm_loop_0;}else{return 0;}}}catch(e){return e.name;}}",0);
    equivalent("function f(){let i=0;try{vm_loop_0:for(;;){const $SSA_1=1;if(i<2){$SSA_1=2;i++;continue vm_loop_0;}else{return 0;}}}catch(e){return e.name;}}",0);
    equivalent("function f(){let i=0;const saved=[],log=[];vm_loop_0:for(;;){log.push(i);if(i++<3){let $SSA_1=i;saved.push(class{x=$SSA_1;});continue vm_loop_0;}else{return [saved.map(C=>new C().x),log];}}}",1);
    equivalent("function f(){const $SSA_1=7;let i=0;const log=[];vm_loop_0:for(;;){log.push($SSA_1);if(i++<2){let $SSA_1=i;log.push($SSA_1);continue vm_loop_0;}else{return log;}}}",1);
}
