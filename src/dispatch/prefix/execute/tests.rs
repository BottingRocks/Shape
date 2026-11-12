use super::*;
use crate::{deobfuscate_with_report, prepare_prefix_from_report, DispatchStart, PrefixLimits};

fn prepared_fixture() -> PrefixPreparation {
    let source = include_str!("../../../../tests/fixtures/javascript-vm-prefix.raw.js");
    let report = deobfuscate_with_report(source).expect("fixture pipeline");
    prepare_prefix_from_report(source, &report, PrefixLimits::default())
        .expect("proof-backed prefix preparation")
}

fn execute_at(preparation: &PrefixPreparation, start: DispatchStart) -> PrefixTraceReport {
    run_prefix(
        preparation,
        start,
        TraceContext::new(),
        TraceLimits::default(),
        TraceDebugOptions::disabled(),
        &CellSharing::default(),
    )
}

fn has_loop(report: &PrefixTraceReport) -> bool {
    report
        .trace
        .arena
        .statements
        .iter()
        .any(|statement| matches!(statement.kind, crate::TraceStatementKind::Loop { .. }))
}

fn last_diagnostic(report: &PrefixTraceReport) -> (&str, &str) {
    let diagnostic = report
        .trace
        .diagnostics
        .last()
        .expect("a halted step records a diagnostic");
    (diagnostic.code.as_str(), diagnostic.message.as_str())
}

#[test]
fn fetch_and_decode_faults_keep_their_stops_codes_and_messages() {
    let prepared = prepared_fixture();

    let report = execute_at(&prepared, DispatchStart { pc: 0, state: 9 });
    assert_eq!(report.stop, PrefixStopReason::MissingState { state: 9 });
    assert_eq!(
        last_diagnostic(&report),
        (
            "prefix-missing-state",
            "dispatch state 9 has no proven table row"
        )
    );

    let mut missing_opcode = prepared.clone();
    missing_opcode.initial_image[0] = 200;
    let report = execute_at(&missing_opcode, DispatchStart { pc: 0, state: 0 });
    assert_eq!(
        report.stop,
        PrefixStopReason::MissingOpcode {
            state: 0,
            opcode: 200
        }
    );
    assert_eq!(
        last_diagnostic(&report),
        (
            "prefix-missing-opcode",
            "dispatch state 0 opcode 200 has no proven cell"
        )
    );

    let mut invalid_next_state = prepared.clone();
    invalid_next_state.table[0][0].next_state = 9;
    let report = execute_at(&invalid_next_state, DispatchStart { pc: 0, state: 0 });
    assert_eq!(report.stop, PrefixStopReason::MissingState { state: 9 });
    assert_eq!(
        last_diagnostic(&report),
        (
            "prefix-invalid-next-state",
            "dispatch cell targets state 9 outside the proven table"
        )
    );

    let mut repeated = prepared.clone();
    repeated.table[0][0] = crate::DispatchCell {
        next_state: 0,
        selector: 1,
    };
    let handler = repeated
        .operands
        .handlers
        .iter_mut()
        .find(|handler| handler.handler_index == 1)
        .expect("handler 1");
    handler.pc_delta = 0;
    handler.entry_coordinate = crate::OperandEntryCoordinate::AtOpcode;
    // A handler that never moves the PC loops forever in place.
    let report = execute_at(&repeated, DispatchStart { pc: 0, state: 0 });
    assert_eq!(report.stop, PrefixStopReason::Completed);
    assert!(has_loop(&report));

    let mut truncated_image = prepared.clone();
    truncated_image.table[0][0] = crate::DispatchCell {
        next_state: 0,
        selector: 1,
    };
    truncated_image
        .operands
        .handlers
        .iter_mut()
        .find(|handler| handler.handler_index == 1)
        .expect("handler 1")
        .pc_delta = 1_000;
    let report = execute_at(&truncated_image, DispatchStart { pc: 0, state: 0 });
    assert_eq!(
        report.stop,
        PrefixStopReason::TruncatedImage {
            pc: 1001,
            image_len: 12
        }
    );
    assert_eq!(
        last_diagnostic(&report),
        (
            "prefix-truncated-image",
            "dispatch pc 1001 is outside the 12 byte image"
        )
    );

    let mut truncated_operands = prepared.clone();
    truncated_operands.patch_handler.selector = 99;
    let template = truncated_operands
        .templates
        .handlers
        .iter_mut()
        .find(|handler| handler.handler_index == 0)
        .expect("handler 0");
    template.status = TemplateStatus::Executable;
    template.unsupported.clear();
    truncated_operands
        .operands
        .handlers
        .iter_mut()
        .find(|handler| handler.handler_index == 0)
        .expect("handler 0")
        .status = OperandHandlerStatus::Lifted;
    truncated_operands.initial_image.truncate(2);
    let report = execute_at(&truncated_operands, DispatchStart { pc: 0, state: 0 });
    assert_eq!(
        report.stop,
        PrefixStopReason::TruncatedOperand {
            operand_entry_pc: 1,
            needed: 3,
            available: 1,
        }
    );
    assert_eq!(
        last_diagnostic(&report),
        (
            "prefix-truncated-operands",
            "operand image is truncated before the handler operands are complete"
        )
    );

    let mut missing_operands = prepared.clone();
    missing_operands.operands.handlers.clear();
    missing_operands.operand_index =
        crate::dispatch::step::SelectorIndex::from_handler_indices(std::iter::empty::<u32>());
    let report = execute_at(&missing_operands, DispatchStart { pc: 4, state: 0 });
    assert_eq!(
        report.stop,
        PrefixStopReason::UnsupportedHandler {
            selector: 1,
            reason: "selector has no operand-separation handler".into(),
        }
    );
    assert_eq!(
        last_diagnostic(&report),
        (
            "prefix-missing-operands",
            "selector has no operand-separation handler"
        )
    );
}

/// The prefix driver applies template existence, template executability and
/// operand-status admission at each reached cell. These are the run-time
/// checks whose main-driver equivalents are preparation-time guarantees.
/// The indexes are rebuilt after the malformed list edit so they stay
/// consistent with the lists they describe.
#[test]
fn admission_faults_keep_the_prefix_codes_and_commit_nothing() {
    let prepared = prepared_fixture();
    // pc 5 selects the non-patch handler 1.
    let start = DispatchStart { pc: 4, state: 0 };

    let mut missing_template = prepared.clone();
    missing_template
        .templates
        .handlers
        .retain(|handler| handler.handler_index != 1);
    missing_template.template_index = crate::dispatch::step::SelectorIndex::from_handler_indices(
        missing_template
            .templates
            .handlers
            .iter()
            .map(|handler| handler.handler_index),
    );
    let report = execute_at(&missing_template, start);
    assert_eq!(
        report.stop,
        PrefixStopReason::UnsupportedHandler {
            selector: 1,
            reason: "selector has no proven operation-table handler".into(),
        }
    );
    assert!(report.records.is_empty());
    assert!(report.patches.is_empty());
    assert_eq!(report.trace.handlers_traced, 0);
    assert_eq!(
        last_diagnostic(&report),
        (
            "prefix-missing-handler",
            "selector has no proven operation-table handler"
        )
    );

    let mut unsupported_template = prepared.clone();
    unsupported_template
        .templates
        .handlers
        .iter_mut()
        .find(|handler| handler.handler_index == 1)
        .expect("handler 1")
        .status = TemplateStatus::Partial;
    let report = execute_at(&unsupported_template, start);
    assert_eq!(
        report.stop,
        PrefixStopReason::UnsupportedHandler {
            selector: 1,
            reason: "handler status is partial".into(),
        }
    );
    assert!(report.records.is_empty());
    assert!(report.patches.is_empty());
    assert_eq!(
        last_diagnostic(&report),
        ("prefix-unsupported-handler", "handler status is partial")
    );

    let mut unlifted = prepared.clone();
    unlifted
        .operands
        .handlers
        .iter_mut()
        .find(|handler| handler.handler_index == 1)
        .expect("handler 1")
        .status = OperandHandlerStatus::Partial;
    let report = execute_at(&unlifted, start);
    assert_eq!(
        report.stop,
        PrefixStopReason::UnsupportedHandler {
            selector: 1,
            reason: "handler operands are not fully proven (status Partial)".into(),
        }
    );
    assert!(report.records.is_empty());
    assert!(report.patches.is_empty());
    assert_eq!(
        last_diagnostic(&report),
        (
            "prefix-unlifted-handler",
            "handler operands are not fully proven (status Partial)"
        )
    );
}

/// Replace handler 1's template with an absolute jump to `target`/`state`.
fn with_jumping_handler_one(target: &str, state: &str) -> PrefixPreparation {
    let number =
        |text: &str| crate::SemanticValue::Literal(crate::LiteralValue::Number(text.into()));
    let mut prepared = prepared_fixture();
    let template = prepared
        .templates
        .handlers
        .iter_mut()
        .find(|handler| handler.handler_index == 1)
        .expect("handler 1");
    let span = template.source_span.clone();
    template.steps = [
        crate::RecipeOperation::SetProgramCounter {
            value: number(target),
        },
        crate::RecipeOperation::SetDispatchState {
            value: number(state),
        },
    ]
    .into_iter()
    .enumerate()
    .map(|(order, operation)| crate::RecipeStep {
        order: order as u32,
        span: span.clone(),
        operation,
    })
    .collect();
    prepared
}

#[test]
fn a_read_past_the_constant_table_stops_as_evidence() {
    let number = |text: &str| {
        Box::new(crate::SemanticValue::Literal(crate::LiteralValue::Number(
            text.into(),
        )))
    };
    let mut prepared = with_jumping_handler_one("4", "0");
    prepared.constant_tables = vec![("table".into(), vec!["only".to_string()].into())];
    let template = prepared
        .templates
        .handlers
        .iter_mut()
        .find(|handler| handler.handler_index == 1)
        .expect("handler 1");
    template.steps.truncate(1);
    template.steps[0].operation = crate::RecipeOperation::Let {
        temporary: 0,
        value: crate::SemanticValue::PropertyRead {
            object: Box::new(crate::SemanticValue::Binding {
                name: "table".into(),
            }),
            key: number("1"),
        },
    };
    let report = execute_at(&prepared, DispatchStart { pc: 4, state: 0 });
    assert_eq!(
        last_diagnostic(&report),
        ("constant-table-index", "table[1] is past its 1 entries")
    );
    // Straight from a made function's entry, the same read makes it junk.
    assert!(converge::fails_at_entry(&prepared, &report));
    let looping = with_jumping_handler_one("4", "0");
    let report = execute_at(&looping, DispatchStart { pc: 4, state: 0 });
    assert!(!converge::fails_at_entry(&looping, &report));
}

/// The side of an unknown test that makes a VM function whose first
/// instruction reads a constant table past its end is no code that runs.
#[test]
fn a_function_made_reading_past_the_constant_table_is_junk() {
    let number =
        |text: &str| crate::SemanticValue::Literal(crate::LiteralValue::Number(text.into()));
    let mut prepared = prepared_fixture();
    prepared.constant_tables = vec![("table".into(), vec!["only".to_string()].into())];
    // 0: if (host.flag) goto 2; 1: make the function at 3; 2: return;
    // 3: table[1].
    prepared.initial_image = vec![1, 2, 3, 4];
    prepared.table[0] = (0..5)
        .map(|selector| crate::DispatchCell {
            next_state: 0,
            selector,
        })
        .collect();
    let template = prepared
        .templates
        .handlers
        .iter()
        .find(|handler| handler.handler_index == 1)
        .expect("handler 1")
        .clone();
    let operands = prepared
        .operands
        .handlers
        .iter()
        .find(|handler| handler.handler_index == 1)
        .expect("handler 1")
        .clone();
    prepared
        .templates
        .handlers
        .retain(|handler| handler.handler_index < 2);
    prepared
        .operands
        .handlers
        .retain(|handler| handler.handler_index < 2);
    for handler_index in 2..5 {
        let mut template = template.clone();
        template.handler_index = handler_index;
        prepared.templates.handlers.push(template);
        let mut operands = operands.clone();
        operands.handler_index = handler_index;
        prepared.operands.handlers.push(operands);
    }
    prepared.template_index = crate::dispatch::step::SelectorIndex::from_handler_indices(
        prepared
            .templates
            .handlers
            .iter()
            .map(|handler| handler.handler_index),
    );
    prepared.operand_index = crate::dispatch::step::SelectorIndex::from_handler_indices(
        prepared
            .operands
            .handlers
            .iter()
            .map(|handler| handler.handler_index),
    );
    let operations = [
        crate::RecipeOperation::JumpIf {
            condition: crate::SemanticValue::PropertyRead {
                object: Box::new(crate::SemanticValue::Binding {
                    name: "host".into(),
                }),
                key: Box::new(crate::SemanticValue::Literal(crate::LiteralValue::String(
                    "flag".into(),
                ))),
            },
            byte_offset: number("2"),
            dispatch_state: number("0"),
        },
        crate::RecipeOperation::Evaluate {
            value: crate::SemanticValue::CreateThread {
                descriptor: Box::new(number("0")),
                byte_offset: Box::new(number("3")),
                dispatch_state: Box::new(number("0")),
            },
        },
        crate::RecipeOperation::CompleteThread { value: number("0") },
        crate::RecipeOperation::Let {
            temporary: 0,
            value: crate::SemanticValue::PropertyRead {
                object: Box::new(crate::SemanticValue::Binding {
                    name: "table".into(),
                }),
                key: Box::new(number("1")),
            },
        },
    ];
    for (index, operation) in (1..).zip(operations) {
        let template = prepared
            .templates
            .handlers
            .iter_mut()
            .find(|handler| handler.handler_index == index)
            .expect("handler");
        template.steps.truncate(1);
        template.steps[0].operation = operation;
    }
    let report = execute_at(&prepared, DispatchStart { pc: 0, state: 0 });
    assert_eq!(report.stop, PrefixStopReason::Completed);
    let kind = report
        .records
        .iter()
        .find_map(|record| match &record.effect {
            PrefixStepEffect::Handler {
                branch: Some(branch),
                ..
            } => Some(branch.kind.clone()),
            _ => None,
        })
        .expect("the jump is a branch");
    assert!(
        matches!(
            &kind,
            PrefixBranchKind::FakeByContext { taken: true, reason }
                if reason.starts_with("junk-function-entry")
        ),
        "{kind:?}"
    );
}

/// A fork whose fall-through makes a VM function, as bytecode: opcode 1
/// jumps past it on `host.flag`, 2 makes the function entered at pc 3, 3
/// returns; that function's first instruction, opcode 4, reads
/// `table[index]`.
fn with_function_made_on_a_side(index: &str) -> PrefixPreparation {
    let number =
        |text: &str| crate::SemanticValue::Literal(crate::LiteralValue::Number(text.into()));
    let source = include_str!("../../../../tests/fixtures/javascript-vm-prefix.raw.js")
        .replace(
            "context.G.push(1);\n    }\n  ];",
            "context.G.push(1);\n    },\n    function (context) {\n      return;\n    }\n  ];",
        )
        .replace(
            "[[0, 0], [0, 1]]",
            "[[0, 0], [0, 1], [0, 2], [0, 3], [0, 4]]",
        );
    let report = deobfuscate_with_report(&source).expect("fixture pipeline");
    let mut prepared = prepare_prefix_from_report(&source, &report, PrefixLimits::default())
        .expect("proof-backed prefix preparation");
    prepared.initial_image = vec![1, 2, 3, 4, 3];
    prepared.constant_tables = vec![("table".into(), vec!["only".to_string()].into())];
    let flag = crate::SemanticValue::PropertyRead {
        object: Box::new(crate::SemanticValue::Binding {
            name: "host".into(),
        }),
        key: Box::new(crate::SemanticValue::Literal(crate::LiteralValue::String(
            "flag".into(),
        ))),
    };
    let handlers = [
        crate::RecipeOperation::JumpIf {
            condition: flag,
            byte_offset: number("2"),
            dispatch_state: number("0"),
        },
        crate::RecipeOperation::Evaluate {
            value: crate::SemanticValue::CreateThread {
                descriptor: Box::new(number("0")),
                byte_offset: Box::new(number("3")),
                dispatch_state: Box::new(number("0")),
            },
        },
        crate::RecipeOperation::CompleteThread { value: number("7") },
        crate::RecipeOperation::Let {
            temporary: 0,
            value: crate::SemanticValue::PropertyRead {
                object: Box::new(crate::SemanticValue::Binding {
                    name: "table".into(),
                }),
                key: Box::new(number(index)),
            },
        },
    ];
    let operands = prepared
        .operands
        .handlers
        .iter()
        .find(|handler| handler.handler_index == 1)
        .expect("handler 1")
        .status
        .clone();
    for (selector, operation) in (1..).zip(handlers) {
        prepared
            .operands
            .handlers
            .iter_mut()
            .find(|handler| handler.handler_index == selector)
            .expect("handler")
            .status = operands.clone();
        let template = prepared
            .templates
            .handlers
            .iter_mut()
            .find(|handler| handler.handler_index == selector)
            .expect("handler");
        template.status = TemplateStatus::Executable;
        template.unsupported.clear();
        template.steps = vec![crate::RecipeStep {
            order: 0,
            span: template.source_span.clone(),
            operation,
        }];
    }
    prepared
}

/// The kind of the walk's only fork.
fn fork_kind(report: &PrefixTraceReport) -> &PrefixBranchKind {
    report
        .records
        .iter()
        .find_map(|record| match &record.effect {
            PrefixStepEffect::Handler {
                branch: Some(branch),
                ..
            } => Some(&branch.kind),
            _ => None,
        })
        .expect("the fork's record")
}

/// A made function that starts by reading past the constant table has an
/// operand no code the VM runs has: the side that made it is fake.
#[test]
fn a_function_made_reading_past_the_constant_table_rules_out_its_side() {
    let prepared = with_function_made_on_a_side("9");
    let report = execute_at(&prepared, DispatchStart { pc: 0, state: 0 });
    assert_eq!(report.stop, PrefixStopReason::Completed);
    let fork = fork_kind(&report);
    assert!(
        matches!(
            fork,
            PrefixBranchKind::FakeByContext { taken: true, reason }
                if reason.starts_with("junk-function-entry")
        ),
        "{fork:?}"
    );
}

/// One that reads inside the table is no evidence: the fork stays real.
#[test]
fn a_function_made_reading_the_constant_table_leaves_its_side_real() {
    let prepared = with_function_made_on_a_side("0");
    let report = execute_at(&prepared, DispatchStart { pc: 0, state: 0 });
    assert_eq!(report.stop, PrefixStopReason::Completed);
    assert_eq!(fork_kind(&report), &PrefixBranchKind::Real);
}

/// The prefix driver follows handler jumps like the main driver: a jump
/// back to the starting position under the same call stack is a loop,
/// here one that never ends.
#[test]
fn a_handler_jump_back_to_its_own_position_is_a_loop() {
    let prepared = with_jumping_handler_one("4", "0");
    let report = execute_at(&prepared, DispatchStart { pc: 4, state: 0 });
    assert_eq!(report.stop, PrefixStopReason::Completed);
    assert_eq!(report.records.len(), 1);
    assert!(has_loop(&report));
    let code = crate::emit_javascript(&report.trace, &crate::EmitOptions::default())
        .expect("a complete walk emits")
        .code;
    assert!(code.contains("vm_loop_0: for (;;)"), "{code}");
    assert!(code.contains("continue vm_loop_0;"), "{code}");
}

#[test]
fn a_handler_jump_outside_the_image_stops_at_the_next_fetch() {
    let prepared = with_jumping_handler_one("99", "0");
    let report = execute_at(&prepared, DispatchStart { pc: 4, state: 0 });
    assert_eq!(
        report.stop,
        PrefixStopReason::TruncatedImage {
            pc: 99,
            image_len: 12
        }
    );
    assert_eq!(report.records.len(), 1);
}

#[test]
fn image_patch_decoding_is_width_driven_and_bounded() {
    let selector = 222;
    let mut image = vec![0u8; 8];
    image[..PATCH_OPERAND_WIDTH].copy_from_slice(&[7, 0, 0, 0xab]);
    let (patch, actual_next_pc) = apply_image_patch(&mut image, 0, PATCH_OPERAND_WIDTH, selector)
        .expect("last image byte is a valid patch target");
    assert_eq!(
        patch,
        ImagePatchRecord {
            address: 7,
            previous: 0,
            value: 0xab,
        }
    );
    assert_eq!(actual_next_pc, PATCH_OPERAND_WIDTH);
    assert_eq!(image[7], 0xab);

    let mut one_past_end = vec![0u8; 8];
    one_past_end[..PATCH_OPERAND_WIDTH].copy_from_slice(&[8, 0, 0, 0xab]);
    assert_eq!(
        apply_image_patch(&mut one_past_end, 0, PATCH_OPERAND_WIDTH, selector),
        Err(PrefixStopReason::AddressOutOfBounds {
            address: 8,
            image_len: 8,
        })
    );

    let mut truncated = vec![0u8; PATCH_OPERAND_WIDTH - 1];
    assert_eq!(
        apply_image_patch(&mut truncated, 0, PATCH_OPERAND_WIDTH, selector),
        Err(PrefixStopReason::TruncatedOperand {
            operand_entry_pc: 0,
            needed: PATCH_OPERAND_WIDTH,
            available: PATCH_OPERAND_WIDTH - 1,
        })
    );

    let mut short_layout = vec![0u8; 8];
    short_layout[..PATCH_OPERAND_WIDTH].copy_from_slice(&[0, 0, 0, 1]);
    assert_eq!(
            apply_image_patch(
                &mut short_layout,
                0,
                PATCH_OPERAND_WIDTH - 1,
                selector
            ),
            Err(PrefixStopReason::UnsupportedHandler {
                selector,
                reason: format!(
                    "image-patch handler operands are {} bytes; the proven layout is {PATCH_OPERAND_WIDTH} bytes",
                    PATCH_OPERAND_WIDTH - 1
                ),
            })
        );
}

#[test]
fn step_faults_keep_the_prefix_codes_and_messages() {
    let cases = [
        (
            StepFault::Repeated { pc: 3, state: 4 },
            PrefixStopReason::RepeatedPcState { pc: 3, state: 4 },
            "prefix-repeated-pc-state",
            "prefix revisited pc 3 state 4 without progress",
        ),
        (
            StepFault::TruncatedImage {
                pc: 5,
                image_len: 7,
            },
            PrefixStopReason::TruncatedImage {
                pc: 5,
                image_len: 7,
            },
            "prefix-truncated-image",
            "dispatch pc 5 is outside the 7 byte image",
        ),
        (
            StepFault::MissingState { state: 2 },
            PrefixStopReason::MissingState { state: 2 },
            "prefix-missing-state",
            "dispatch state 2 has no proven table row",
        ),
        (
            StepFault::MissingOpcode {
                state: 2,
                opcode: 9,
            },
            PrefixStopReason::MissingOpcode {
                state: 2,
                opcode: 9,
            },
            "prefix-missing-opcode",
            "dispatch state 2 opcode 9 has no proven cell",
        ),
        (
            StepFault::InvalidNextState { next_state: 6 },
            PrefixStopReason::MissingState { state: 6 },
            "prefix-invalid-next-state",
            "dispatch cell targets state 6 outside the proven table",
        ),
        (
            StepFault::PcOverflow {
                pc: 8,
                image_len: 9,
            },
            PrefixStopReason::TruncatedImage {
                pc: 8,
                image_len: 9,
            },
            "prefix-pc-overflow",
            "dispatch PC increment overflowed the address space",
        ),
        (
            StepFault::TruncatedOperands {
                position: 2,
                operand_entry_pc: 1,
                required_end: 5,
                image_len: 4,
            },
            PrefixStopReason::TruncatedOperand {
                operand_entry_pc: 1,
                needed: 4,
                available: 3,
            },
            "prefix-truncated-operands",
            "operand image is truncated before the handler operands are complete",
        ),
        (
            StepFault::UnsupportedOperands {
                selector: 7,
                reason: "unsupported operand recipe: reason".into(),
            },
            PrefixStopReason::UnsupportedHandler {
                selector: 7,
                reason: "unsupported operand recipe: reason".into(),
            },
            "prefix-unsupported-operands",
            "unsupported operand recipe: reason",
        ),
        (
            StepFault::NextPcOverflow {
                pc: 10,
                image_len: 11,
            },
            PrefixStopReason::TruncatedImage {
                pc: 10,
                image_len: 11,
            },
            "prefix-next-pc-overflow",
            "handler PC delta overflowed the address space",
        ),
        (
            StepFault::MissingOperandHandler { selector: 7 },
            PrefixStopReason::UnsupportedHandler {
                selector: 7,
                reason: "selector has no operand-separation handler".into(),
            },
            "prefix-missing-operands",
            "selector has no operand-separation handler",
        ),
    ];
    for (fault, expected_stop, expected_code, expected_message) in cases {
        let (stop, code, message) = prefix_fault(&fault);
        assert_eq!(stop, expected_stop, "stop for {fault:?}");
        assert_eq!(code.as_str(), expected_code, "code for {fault:?}");
        assert_eq!(message, expected_message, "message for {fault:?}");
    }
}
