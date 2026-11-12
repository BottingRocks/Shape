use super::*;

/// A trace arena built by hand, for program mode's tests.
#[derive(Default)]
struct Built {
    arena: TraceArena,
    inputs: Vec<String>,
}

impl Built {
    fn provenance() -> TraceProvenance {
        TraceProvenance {
            handler_index: 0,
            table_key: 0,
            semantic_identity: "".into(),
            step_order: 0,
            trace_step: 0,
            span: SpanRecord { start: 0, end: 0 },
        }
    }
    fn node(&mut self, kind: TraceNodeKind) -> TraceNodeId {
        let id = self.arena.nodes.len() as TraceNodeId;
        if let TraceNodeKind::External { name } = &kind {
            if !self.inputs.contains(name) {
                self.inputs.push(name.clone());
            }
        }
        self.arena.nodes.push(TraceNode {
            id,
            kind,
            provenance: Self::provenance(),
        });
        id
    }
    fn input(&mut self, name: &str) -> TraceNodeId {
        self.node(TraceNodeKind::External { name: name.into() })
    }
    fn name(&mut self, name: &str) -> TraceNodeId {
        self.node(TraceNodeKind::Identifier { name: name.into() })
    }
    fn statement(&mut self, kind: TraceStatementKind) -> TraceStatementId {
        let id = self.arena.statements.len() as TraceStatementId;
        self.arena.statements.push(TraceStatement {
            id,
            kind,
            provenance: Self::provenance(),
        });
        id
    }
    fn declare(&mut self, names: &[&str]) {
        let names = names.iter().map(|name| name.to_string()).collect();
        self.statement(TraceStatementKind::Declare { names });
    }
    fn assign(&mut self, name: &str, value: TraceNodeId) -> TraceStatementId {
        self.statement(TraceStatementKind::Assign {
            name: name.into(),
            value,
        })
    }
    /// `{ value: value }`, a new cell.
    fn cell(&mut self, value: TraceNodeId) -> TraceNodeId {
        self.node(TraceNodeKind::ObjectLiteral {
            keys: vec!["value".into()],
            values: vec![value],
        })
    }
    /// `let name = vmCreateThread(7, 100, 2, { "K": $CELL_K, … })`.
    fn create(&mut self, name: &str, cells: &[(&str, &str)]) -> TraceStatementId {
        let values = cells.iter().map(|(_, cell)| self.name(cell)).collect();
        let memory = self.node(TraceNodeKind::ObjectLiteral {
            keys: cells.iter().map(|(key, _)| key.to_string()).collect(),
            values,
        });
        let value = self.node(TraceNodeKind::VmFunction {
            descriptor: 7,
            byte_offset: 100,
            dispatch_state: 2,
            memory: Some(memory),
        });
        self.statement(TraceStatementKind::Let {
            name: name.into(),
            value,
        })
    }
    fn emit(self) -> Result<(EmittedJavaScript, ProgramFacts), EmitError> {
        let report = TraceReport {
            arena: self.arena,
            completion: TraceCompletion::Completed,
            diagnostics: Vec::new(),
            handlers_traced: 0,
            external_inputs: self.inputs,
            final_state: TraceStateSnapshot {
                stack_length: 0,
                memory_slots: 0,
            },
        };
        let options = EmitOptions {
            simplify_bindings: true,
            function_name: PROGRAM_SELF.into(),
            ..EmitOptions::default()
        };
        emit_program_function(&report, &options)
    }
    fn error(self) -> String {
        match self.emit() {
            Ok((emitted, _)) => panic!("emitted {}", emitted.code),
            Err(error) => error.to_string(),
        }
    }
}

#[test]
fn a_position_meets_a_value_as_the_position() {
    use CellMark::{Position, Unknown, Value};
    assert_eq!(Position(7).join(Position(7)), Position(7));
    assert_eq!(Position(7).join(Value), Position(7));
    assert_eq!(Value.join(Position(7)), Position(7));
    assert_eq!(Value.join(Value), Value);
    assert_eq!(Position(7).join(Position(8)), Unknown);
    assert_eq!(Position(7).join(Unknown), Unknown);
    assert_eq!(Unknown.join(Value), Unknown);
}

#[test]
fn program_mode_reads_inputs_in_place_and_every_slot_as_its_key() {
    let mut built = Built::default();
    built.declare(&["$CELL_3", "$CELL_4"]);
    let captured = built.input("captured3");
    built.assign("$CELL_3", captured);
    let argument = built.input("arg0");
    let cell = built.cell(argument);
    built.assign("$CELL_4", cell);
    let shared = built.name("$CELL_3");
    let key = built.node(TraceNodeKind::String("value".into()));
    let read = built.node(TraceNodeKind::PropertyRead {
        object: shared,
        key,
    });
    built.statement(TraceStatementKind::Let {
        name: "trace_value_1".into(),
        value: read,
    });
    let own = built.name("$CELL_4");
    let receiver = built.input("receiver");
    built.statement(TraceStatementKind::PropertyWrite {
        object: own,
        key,
        value: receiver,
    });
    built.create("trace_value_2", &[("4", "$CELL_4"), ("3", "$CELL_3")]);
    let read = built.name("trace_value_1");
    let created = built.name("trace_value_2");
    let callee = built.input("callee");
    let arguments = built.input("args");
    let all = built.node(TraceNodeKind::ArrayLiteral {
        elements: vec![read, created, callee, arguments],
    });
    built.statement(TraceStatementKind::Return { value: all });
    let (emitted, facts) = built.emit().unwrap();
    assert_eq!(
            emitted.code,
            "function $THREAD_SELF() {\n\t\"use strict\";\n\tlet $KEY_4;\n\t$KEY_4 = arguments[0];\n\tlet $SSA_1 = $KEY_3;\n\t$KEY_4 = $THIS;\n\tfunction $THREAD_2_100() {}\n\treturn [\n\t\t$SSA_1,\n\t\t$THREAD_2_100,\n\t\t$THREAD_SELF,\n\t\targuments\n\t];\n}\n"
        );
    assert_eq!(facts.captured, BTreeSet::from([3]));
    assert_eq!((facts.self_names, facts.this_names), (1, 1));
    assert_eq!(
        facts.threads,
        [ProgramThread {
            name: "$THREAD_2_100".into(),
            descriptor: 7,
            pc: 100,
            state: 2,
            cells: vec![4, 3],
        }]
    );
}

#[test]
fn program_mode_fails_a_new_cell_after_a_creation_captures_the_old() {
    let mut built = Built::default();
    built.declare(&["$CELL_4"]);
    let one = built.node(TraceNodeKind::Number("1".into()));
    let cell = built.cell(one);
    built.assign("$CELL_4", cell);
    built.create("trace_value_2", &[("4", "$CELL_4")]);
    let cell = built.cell(one);
    built.assign("$CELL_4", cell);
    let error = built.error();
    assert!(error.contains("slot 4 gets a new cell after"), "{error}");
}

#[test]
fn program_mode_fails_a_new_cell_in_a_loop_around_a_creation() {
    let mut built = Built::default();
    built.declare(&["$CELL_4"]);
    // A block owns statements after it.
    let around = built.statement(TraceStatementKind::Loop {
        label: "vm_loop_1".into(),
        body: Vec::new(),
    });
    let one = built.node(TraceNodeKind::Number("1".into()));
    let cell = built.cell(one);
    let renew = built.assign("$CELL_4", cell);
    let create = built.create("trace_value_2", &[("4", "$CELL_4")]);
    built.arena.statements[around as usize].kind = TraceStatementKind::Loop {
        label: "vm_loop_1".into(),
        body: vec![renew, create],
    };
    let error = built.error();
    assert!(error.contains("slot 4 gets a new cell after"), "{error}");
}

#[test]
fn program_mode_fails_a_cell_declared_inside_a_block() {
    let mut built = Built::default();
    let around = built.statement(TraceStatementKind::Loop {
        label: "vm_loop_1".into(),
        body: Vec::new(),
    });
    built.declare(&["$CELL_4"]);
    built.arena.statements[around as usize].kind = TraceStatementKind::Loop {
        label: "vm_loop_1".into(),
        body: vec![around + 1],
    };
    let error = built.error();
    assert!(error.contains("declared inside a block"), "{error}");
}

#[test]
fn program_mode_fails_a_new_cell_for_a_shared_slot() {
    let mut built = Built::default();
    built.declare(&["$CELL_3"]);
    let captured = built.input("captured3");
    built.assign("$CELL_3", captured);
    let one = built.node(TraceNodeKind::Number("1".into()));
    let cell = built.cell(one);
    built.assign("$CELL_3", cell);
    let error = built.error();
    assert!(error.contains("gives slot 3"), "{error}");
}

#[test]
fn program_mode_fails_a_creation_passing_another_slot() {
    let mut built = Built::default();
    built.declare(&["$CELL_3"]);
    let one = built.node(TraceNodeKind::Number("1".into()));
    let cell = built.cell(one);
    built.assign("$CELL_3", cell);
    built.create("trace_value_2", &[("4", "$CELL_3")]);
    let error = built.error();
    assert!(error.contains("as slot 4"), "{error}");
}

#[test]
fn program_mode_fails_a_cell_used_other_than_through_its_value() {
    let mut built = Built::default();
    built.declare(&["$CELL_4"]);
    let one = built.node(TraceNodeKind::Number("1".into()));
    let cell = built.cell(one);
    built.assign("$CELL_4", cell);
    let box_ = built.name("$CELL_4");
    built.statement(TraceStatementKind::Return { value: box_ });
    let error = built.error();
    assert!(error.contains("other than through its value"), "{error}");
}

#[test]
fn program_mode_fails_a_shared_slot_that_is_also_a_plain_variable() {
    let mut built = Built::default();
    built.declare(&["$CELL_3"]);
    let captured = built.input("captured3");
    built.assign("$CELL_3", captured);
    let other = built.input("captured5");
    built.assign("$CELL_3", other);
    let error = built.error();
    assert!(error.contains("is given what is not a cell"), "{error}");
}

#[test]
fn a_wasm_catch_drop_removes_the_thrown_value_under_the_top() {
    let span = SpanRecord { start: 0, end: 0 };
    let template = HandlerTemplate {
        handler_index: 0,
        table_key: 0,
        semantic_identity: "catch drop".into(),
        source_span: span.clone(),
        status: crate::TemplateStatus::Executable,
        steps: [
            RecipeOperation::PopErrorRecord,
            RecipeOperation::DropThrownUnderIfHandled,
        ]
        .into_iter()
        .enumerate()
        .map(|(order, operation)| RecipeStep {
            order: order as u32,
            span: span.clone(),
            operation,
        })
        .collect(),
        unsupported: Vec::new(),
    };
    let thrown = |name: &str| StoredValue {
        value: SymbolicValue::String(name.into()),
        fact: Some(SymbolicValue::String(name.into())),
        node: None,
    };
    let handled = ErrorRecord {
        handled: true,
        saved: None,
        finally: None,
        thrown_in: None,
    };
    let remaining = |values: Vec<StoredValue>| {
        let mut session =
            TraceSession::new_continuing(TraceContext::default(), TraceLimits::default())
                .with_thread_control();
        session.executor.context.error_records = vec![handled.clone(), handled.clone()];
        session.executor.context.thrown = values;
        let result =
            session.execute_handler_at(Some(&template), &DecodedInstruction::new(0, Vec::new()));
        assert!(
            matches!(result, TraceHandlerResult::Continued),
            "{result:?}"
        );
        session
            .executor
            .context
            .thrown
            .iter()
            .map(|stored| stored.value.clone())
            .collect::<Vec<_>>()
    };
    // `TJ(1)` keeps the top: the outer `finally` rethrows the inner value.
    assert_eq!(
        remaining(vec![thrown("outer"), thrown("inner")]),
        [SymbolicValue::String("inner".into())]
    );
    // Alone, the top goes.
    assert_eq!(remaining(vec![thrown("only")]), []);
}

#[test]
fn program_mode_keeps_the_function_its_result_slot_holds() {
    let mut built = Built::default();
    built.declare(&["$CELL_0"]);
    let callee = built.input("callee");
    built.assign("$CELL_0", callee);
    let own = built.name("$CELL_0");
    let key = built.node(TraceNodeKind::String("value".into()));
    let read = built.node(TraceNodeKind::PropertyRead { object: own, key });
    built.statement(TraceStatementKind::Return { value: read });
    let (emitted, facts) = built.emit().unwrap();
    assert_eq!(
        emitted.code,
        "function $THREAD_SELF() {\n\tlet $KEY_0;\n\t$KEY_0 = $THREAD_SELF;\n\treturn $KEY_0;\n}\n"
    );
    assert_eq!(facts.self_names, 1);
}

#[test]
fn a_walk_traces_the_same_when_the_cells_it_read_have_the_same_effect() {
    let content = |addresses: &[usize], data: bool| CellContent {
        addresses: addresses.iter().copied().collect(),
        data,
    };
    let contents = |entries: &[(u32, CellContent)]| -> BTreeMap<u32, CellContent> {
        entries.iter().cloned().collect()
    };
    let reads = CellReads {
        cells: BTreeSet::from([1, 2, 4]),
        data: BTreeSet::from([2]),
        sets: BTreeSet::from([1]),
    };
    let none = contents(&[]);
    // A cell the walk did not read may change freely.
    assert!(reads.same(&none, &contents(&[(3, content(&[7], true))])));
    // Data where the walk never asked whether a read was data.
    assert!(reads.same(&none, &contents(&[(1, content(&[], true))])));
    // Data where it asked.
    assert!(!reads.same(&none, &contents(&[(2, content(&[], true))])));
    // An empty content is no content.
    assert!(reads.same(&none, &contents(&[(2, content(&[], false))])));
    // A value set, new or changed, changes how a read asked it traces.
    assert!(!reads.same(&none, &contents(&[(1, content(&[7], false))])));
    // One never asked for changes nothing.
    assert!(reads.same(&none, &contents(&[(4, content(&[7], true))])));
    // Data turned value set: the read is no longer data.
    assert!(!reads.same(
        &contents(&[(2, content(&[], true))]),
        &contents(&[(2, content(&[7], true))])
    ));
    assert!(!reads.same(
        &contents(&[(1, content(&[7], false))]),
        &contents(&[(1, content(&[8], false))])
    ));
    assert!(!reads.same(
        &contents(&[(1, content(&[7], false))]),
        &contents(&[(1, content(&[7], true))])
    ));
    // Too many addresses for a value set: the read takes nothing.
    let many: Vec<usize> = (0..=MAX_VALUE_SET).collect();
    assert!(reads.same(&none, &contents(&[(1, content(&many, false))])));
}

fn assert_literal_roundtrip(value: f64) {
    let text = number_text(value);
    let parsed = parse_numeric_literal(&text)
        .unwrap_or_else(|| panic!("number_text({value:?}) emitted unparseable {text:?}"));
    if value.is_nan() {
        assert!(parsed.is_nan(), "number_text({value:?}) emitted {text:?}");
    } else {
        assert_eq!(
            parsed.to_bits(),
            value.to_bits(),
            "number_text({value:?}) emitted {text:?}"
        );
    }
}

#[test]
fn number_text_roundtrips_literal_edge_values() {
    for value in [
        0.0,
        -0.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        2_147_483_647.0,
        2_147_483_648.0,
        -2_147_483_648.0,
        -2_147_483_649.0,
        4_294_967_296.0,
        4_294_967_297.0,
        -4_294_967_296.0,
        9_007_199_254_740_992.0,
        9_007_199_254_740_993.0,
        f64::from_bits(0x4340_0000_0000_0001),
        1e21,
        -1e21,
        4_294_967_295.5,
        -0.5,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::from_bits(0x000f_ffff_ffff_ffff),
    ] {
        assert_literal_roundtrip(value);
    }
    assert_eq!(number_text(-0.0), "-0");
    assert_eq!(
        number_text(-0.0).parse::<f64>().unwrap().to_bits(),
        (-0.0f64).to_bits()
    );
    assert_eq!(number_text(f64::NAN), "NaN");
    assert_eq!(number_text(f64::INFINITY), "Infinity");
    assert_eq!(number_text(f64::NEG_INFINITY), "-Infinity");
}

#[test]
fn number_text_is_literal_rendering_not_js_to_string() {
    // Emitted literals only need to parse back to the same value. The
    // JavaScript string-conversion primitive deliberately differs: it
    // renders -0 as "0" and uses the ECMA-262 exponent window.
    assert_eq!(crate::js_number::number_to_js_string(-0.0), "0");
    assert_eq!(number_text(-0.0), "-0");
    assert_eq!(crate::js_number::number_to_js_string(1e21), "1e+21");
    assert_eq!(number_text(1e21), "1000000000000000000000");
}

#[test]
fn an_incomplete_trace_emits_only_when_allowed() {
    let mut built = Built::default();
    let value = built.input("input");
    built.declare(&["x"]);
    built.assign("x", value);
    let report = TraceReport {
        arena: built.arena,
        completion: TraceCompletion::BudgetStopped,
        diagnostics: Vec::new(),
        handlers_traced: 0,
        external_inputs: built.inputs,
        final_state: TraceStateSnapshot {
            stack_length: 0,
            memory_slots: 0,
        },
    };
    let options = EmitOptions {
        external_parameters: report.external_inputs.clone(),
        ..EmitOptions::default()
    };
    assert!(matches!(
        emit::emit_javascript(&report, &options),
        Err(EmitError::IncompleteTrace(_))
    ));
    let options = EmitOptions {
        allow_incomplete: true,
        ..options
    };
    let emitted = emit::emit_javascript(&report, &options).expect("allowed");
    assert!(emitted.code.contains("x = input"), "{}", emitted.code);
}
