//! Op classification: what each VM handler does, in the vocabulary of plain
//! JavaScript operations and VM machinery.
//!
//! Every handler's HandlerIR is run against an abstract operand stack whose
//! entry slots are named inputs (`in[-1]` is the top). The handler's net
//! effect falls out as the number of slots it pops, the values it pushes in
//! terms of those inputs, and its ordered side effects. Each pushed value and
//! side effect is a *component* named after the JavaScript or VM operation it
//! performs (`binary.greater_than`, `unary.typeof`, `property.get`,
//! `memory.write`, `vm.call`, ...). A handler with one component has that
//! component as its kind; several components make a fused handler; anything
//! HandlerIR cannot represent yet is `unrecognized.<reason>`.
//!
//! [`classify_seeds`] then compares kinds across captures: a kind seen in both
//! the JavaScript VM and the WebAssembly VM is common, otherwise it is specific
//! to one VM variant.

use std::collections::{BTreeMap, BTreeSet};

use crate::js_number::parse_numeric_literal;
use crate::transforms::deob::pipeline::PipelineReport;

use super::operands::{js_operand_separation_from_report, OperandHandlerStatus};
use super::templates::{
    handler_templates_from_report, CallFrameField, HandlerTemplate, HandlerTemplateError,
    LiteralValue, PrimitiveOperator, ReceiverField, RecipeOperation, SemanticValue,
};

/// Where a capture's VM runs its dispatch loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VmVariant {
    /// The JavaScript VM: dispatch, operand decode and call frames are JS.
    JavaScript,
    /// The WebAssembly VM: dispatch and operand decode run in WASM and the
    /// handlers receive their operands as parameters.
    WebAssembly,
}

impl VmVariant {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JavaScript => "javascript-vm",
            Self::WebAssembly => "webassembly-vm",
        }
    }
}

/// What kind of operation a handler is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OpNature {
    /// One plain JavaScript operation moved through the stack: an operator,
    /// a literal, a property access, a call, a throw.
    JavaScript,
    /// One piece of VM machinery: memory slots, stack shuffles, jumps, call
    /// frames, thread creation.
    Vm,
    /// Several operations fused into one handler.
    Fused,
    /// HandlerIR cannot represent part of the handler yet.
    Unrecognized,
}

impl OpNature {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JavaScript => "javascript",
            Self::Vm => "vm",
            Self::Fused => "fused",
            Self::Unrecognized => "unrecognized",
        }
    }
}

/// Which VM variants a kind was seen in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OpScope {
    /// Seen in both the JavaScript VM and the WebAssembly VM.
    Common,
    JavaScriptVmOnly,
    WebAssemblyVmOnly,
}

impl OpScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Common => "common",
            Self::JavaScriptVmOnly => "javascript-vm-only",
            Self::WebAssemblyVmOnly => "webassembly-vm-only",
        }
    }
}

/// One classified handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedOp {
    pub handler_index: u32,
    /// The single component, `a + b + ...` for a fused handler, or
    /// `unrecognized.<reason>`.
    pub kind: String,
    pub nature: OpNature,
    /// Stack slots consumed below the entry top.
    pub pops: u32,
    /// Values left on the stack in their place.
    pub pushes: u32,
    /// The handler's source text.
    pub source: String,
}

/// Every handler of one capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedClassification {
    pub variant: VmVariant,
    pub ops: Vec<ClassifiedOp>,
}

/// Classify every handler of one deobfuscated capture.
///
/// A capture with a proven JavaScript dispatch read is the JavaScript VM and
/// its handlers are classified after operand separation, so inline bytecode
/// reads become operands. Otherwise it is the WebAssembly VM, whose handlers
/// already take their operands as parameters.
pub fn classify_report(
    report: &PipelineReport,
) -> Result<SeedClassification, HandlerTemplateError> {
    let mut templates = handler_templates_from_report(report)?;
    // Operand separation only proves an entry coordinate when a JavaScript
    // dispatch method reads the bytecode.
    let separation = js_operand_separation_from_report(report)
        .ok()
        .filter(|separation| separation.entry_coordinate.is_some());
    let variant = match separation {
        Some(separation) => {
            for template in &mut templates.handlers {
                if let Some(lifted) = separation
                    .handlers
                    .iter()
                    .find(|lifted| lifted.handler_index == template.handler_index)
                {
                    if lifted.status != OperandHandlerStatus::Unresolved {
                        *template = lifted.template.clone();
                    }
                }
            }
            VmVariant::JavaScript
        }
        None => VmVariant::WebAssembly,
    };
    let ops = templates
        .handlers
        .iter()
        .map(|template| {
            let source = report
                .output
                .get(template.source_span.start as usize..template.source_span.end as usize)
                .unwrap_or_default()
                .to_string();
            classify_template(template, source)
        })
        .collect();
    Ok(SeedClassification { variant, ops })
}

/// Classify one handler template; `source` is carried into the result.
pub fn classify_template(template: &HandlerTemplate, source: String) -> ClassifiedOp {
    let summary = Machine::run(template);
    let (kind, nature) = summary.kind();
    ClassifiedOp {
        handler_index: template.handler_index,
        kind,
        nature,
        pops: summary.pops,
        pushes: summary.outputs.len() as u32,
        source,
    }
}

/// One kind across captures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindSummary {
    pub kind: String,
    pub nature: OpNature,
    pub scope: OpScope,
    /// `(capture position, handler index)` for every handler of this kind.
    pub handlers: Vec<(usize, u32)>,
}

/// Group the handlers of several captures by kind and give each kind its
/// cross-VM scope. Kinds are in name order.
pub fn classify_seeds(seeds: &[SeedClassification]) -> Vec<KindSummary> {
    /// A kind's nature, the VM variants it was seen in, and its handlers.
    type Seen = (OpNature, BTreeSet<VmVariant>, Vec<(usize, u32)>);
    let mut kinds: BTreeMap<&str, Seen> = BTreeMap::new();
    for (position, seed) in seeds.iter().enumerate() {
        for op in &seed.ops {
            let entry = kinds
                .entry(op.kind.as_str())
                .or_insert_with(|| (op.nature, BTreeSet::new(), Vec::new()));
            entry.1.insert(seed.variant);
            entry.2.push((position, op.handler_index));
        }
    }
    kinds
        .into_iter()
        .map(|(kind, (nature, variants, handlers))| KindSummary {
            kind: kind.to_string(),
            nature,
            scope: match (
                variants.contains(&VmVariant::JavaScript),
                variants.contains(&VmVariant::WebAssembly),
            ) {
                (true, true) => OpScope::Common,
                (true, false) => OpScope::JavaScriptVmOnly,
                _ => OpScope::WebAssemblyVmOnly,
            },
            handlers,
        })
        .collect()
}

/// A value in terms of the handler's inputs.
#[derive(Debug, Clone, PartialEq)]
enum Expr {
    /// The entry stack slot at this offset from the entry length (`-1` is the
    /// top).
    Input(i64),
    /// The stack length, as an offset from the entry length.
    Length(i64),
    Operand(u32),
    Literal(LiteralValue),
    Memory(Box<Expr>),
    Binding(String),
    Binary(PrimitiveOperator, Box<Expr>, Box<Expr>),
    Unary(PrimitiveOperator, Box<Expr>),
    Property(Box<Expr>, Box<Expr>),
    Object,
    Array,
    /// The stack values from this offset to the top.
    Slice,
    Call {
        callee: Box<Expr>,
        /// The receiver is `undefined`: a plain function call.
        plain: bool,
        /// The arguments are a stack slice, as in
        /// `Function.prototype.apply.call(fn, this, stack.slice(...))`.
        spread: bool,
    },
    Frame(CallFrameField),
    /// A VM closure from `vmCreateThread`.
    Thread,
    /// The VM function's `this`.
    Receiver(ReceiverField),
    /// A function expression.
    Function,
    /// A popped try-frame or error-record field.
    ExceptionRecord,
    /// The popped pending exception.
    Thrown,
    /// `new callee(...)`.
    Construct,
    /// `delete object[key]`.
    Delete,
    /// An unwritten slot above the entry length: `undefined`.
    Hole,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq)]
enum Effect {
    MemoryWrite(Expr),
    MemoryInitialize,
    PropertyWrite,
    Evaluate(Expr),
    Return(Expr),
    Throw(Expr),
    PushFrame,
    PopFrame,
    Jump,
    SetState,
    /// A conditional jump on this condition.
    JumpIf(Expr),
    TryPush,
    TryPop,
    FinallyPush,
    ErrorPop,
    Rethrow,
    DropThrown,
    /// The stack length is set to a non-constant value, such as an operand.
    SetLength,
    Unknown(String),
}

/// The abstract stack machine and, after [`Machine::run`], its summary.
#[derive(Default)]
struct Machine {
    temps: BTreeMap<u32, Expr>,
    writes: BTreeMap<i64, Expr>,
    /// Current length as an offset from the entry length; `None` once it
    /// depends on a non-constant value.
    length: Option<i64>,
    /// The lowest entry slot read, written or truncated to.
    lowest: i64,
    effects: Vec<Effect>,
    /// A slice of bytecode-sized length makes the stack positions dynamic;
    /// the written values are still classified.
    variadic: bool,
    variadic_writes: Vec<Expr>,
    pops: u32,
    outputs: Vec<Expr>,
}

impl Machine {
    fn run(template: &HandlerTemplate) -> Self {
        let mut machine = Self {
            length: Some(0),
            ..Self::default()
        };
        for step in &template.steps {
            machine.step(&step.operation);
        }
        machine.finish();
        machine
    }

    fn step(&mut self, operation: &RecipeOperation) {
        match operation {
            RecipeOperation::Let { temporary, value }
            | RecipeOperation::Assign { temporary, value } => {
                let value = self.eval(value);
                self.temps.insert(*temporary, value);
            }
            RecipeOperation::StackWrite { index, value } => {
                let index = self.eval(index);
                let value = self.eval(value);
                match index {
                    Expr::Length(offset) => self.write(offset, value),
                    _ if self.variadic => {
                        self.length = None;
                        self.variadic_writes.push(value);
                    }
                    _ => {
                        self.length = None;
                        self.effects
                            .push(Effect::Unknown("dynamic_stack_index".into()));
                    }
                }
            }
            RecipeOperation::StackLengthSet { value } => match self.eval(value) {
                Expr::Length(offset) => {
                    self.touch(offset);
                    self.writes.retain(|slot, _| *slot < offset);
                    self.length = Some(offset);
                }
                // A variadic handler drops a bytecode-sized number of slots
                // as part of its call.
                _ if self.variadic => self.length = None,
                _ => {
                    self.length = None;
                    self.writes.clear();
                    self.effects.push(Effect::SetLength);
                }
            },
            RecipeOperation::MemoryWrite { slot, value } => {
                self.eval(slot);
                let value = self.eval(value);
                self.effects.push(Effect::MemoryWrite(value));
            }
            RecipeOperation::MemoryInitialize { slot } => {
                self.eval(slot);
                self.effects.push(Effect::MemoryInitialize);
            }
            RecipeOperation::PropertyWrite { object, key, value } => {
                self.eval(object);
                self.eval(key);
                self.eval(value);
                self.effects.push(Effect::PropertyWrite);
            }
            RecipeOperation::PushCallFrame => self.effects.push(Effect::PushFrame),
            RecipeOperation::PopCallFrame => self.effects.push(Effect::PopFrame),
            RecipeOperation::SetProgramCounter { value } => {
                self.eval(value);
                self.effects.push(Effect::Jump);
            }
            RecipeOperation::SetDispatchState { value } => {
                self.eval(value);
                self.effects.push(Effect::SetState);
            }
            RecipeOperation::PushTryFrame {
                byte_offset,
                dispatch_state,
            } => {
                self.eval(byte_offset);
                self.eval(dispatch_state);
                self.effects.push(Effect::TryPush);
            }
            RecipeOperation::PopTryFrame => self.effects.push(Effect::TryPop),
            RecipeOperation::PushFinallyRecord => self.effects.push(Effect::FinallyPush),
            RecipeOperation::PopErrorRecord => self.effects.push(Effect::ErrorPop),
            RecipeOperation::RethrowIfHandled => self.effects.push(Effect::Rethrow),
            RecipeOperation::DropThrownIfHandled | RecipeOperation::DropThrownUnderIfHandled => {
                self.effects.push(Effect::DropThrown)
            }
            RecipeOperation::JumpIf {
                condition,
                byte_offset,
                dispatch_state,
            } => {
                let condition = self.eval(condition);
                self.eval(byte_offset);
                self.eval(dispatch_state);
                self.effects.push(Effect::JumpIf(condition));
            }
            RecipeOperation::Evaluate { value } => {
                let value = self.eval(value);
                self.effects.push(Effect::Evaluate(value));
            }
            RecipeOperation::Return { value } | RecipeOperation::CompleteThread { value } => {
                let value = self.eval(value);
                self.effects.push(Effect::Return(value));
            }
            RecipeOperation::Throw { value } => {
                let value = self.eval(value);
                self.effects.push(Effect::Throw(value));
            }
            RecipeOperation::Unknown { reason, .. } => {
                self.effects.push(Effect::Unknown(reason_slug(reason)));
            }
        }
    }

    fn eval(&mut self, value: &SemanticValue) -> Expr {
        match value {
            SemanticValue::Context => Expr::Unknown("context".into()),
            SemanticValue::Operand { position } => Expr::Operand(*position),
            SemanticValue::Temporary { id } => self
                .temps
                .get(id)
                .cloned()
                .unwrap_or_else(|| Expr::Unknown("temporary".into())),
            SemanticValue::Literal(literal) => Expr::Literal(literal.clone()),
            SemanticValue::StackLength => self
                .length
                .map_or_else(|| Expr::Unknown("dynamic_length".into()), Expr::Length),
            SemanticValue::StackRead { index } => match self.eval(index) {
                Expr::Length(offset) => self.read(offset),
                _ => Expr::Unknown("dynamic_stack_index".into()),
            },
            SemanticValue::MemoryRead { slot } => Expr::Memory(Box::new(self.eval(slot))),
            SemanticValue::Binary {
                operator,
                left,
                right,
            } => {
                let left = self.eval(left);
                let right = self.eval(right);
                // Stack index arithmetic stays an offset.
                match (&left, number(&right)) {
                    (Expr::Length(offset), Some(constant))
                        if constant.fract() == 0.0
                            && matches!(
                                operator,
                                PrimitiveOperator::Subtract | PrimitiveOperator::JsAdd
                            ) =>
                    {
                        let constant = constant as i64;
                        Expr::Length(if *operator == PrimitiveOperator::Subtract {
                            offset - constant
                        } else {
                            offset + constant
                        })
                    }
                    _ => Expr::Binary(*operator, Box::new(left), Box::new(right)),
                }
            }
            SemanticValue::Unary { operator, value } => {
                Expr::Unary(*operator, Box::new(self.eval(value)))
            }
            SemanticValue::Binding { name } => Expr::Binding(name.clone()),
            SemanticValue::PropertyRead { object, key } => {
                let object = self.eval(object);
                let key = self.eval(key);
                Expr::Property(Box::new(object), Box::new(key))
            }
            SemanticValue::ObjectLiteral { properties } => {
                for (_, value) in properties {
                    self.eval(value);
                }
                Expr::Object
            }
            SemanticValue::ArrayLiteral { elements } => {
                for element in elements {
                    self.eval(element);
                }
                Expr::Array
            }
            SemanticValue::PoppedCallFrame { field } => Expr::Frame(*field),
            SemanticValue::StackSlice { start } => {
                match self.eval(start) {
                    Expr::Length(offset) => self.touch(offset),
                    // A bytecode-sized slice reads an unknown number of
                    // slots: the handler is variadic.
                    _ => self.variadic = true,
                }
                Expr::Slice
            }
            SemanticValue::Call {
                callee,
                this_value,
                arguments,
            } => {
                let callee = self.eval(callee);
                let this_value = self.eval(this_value);
                let arguments = self.eval(arguments);
                Expr::Call {
                    callee: Box::new(callee),
                    plain: this_value == Expr::Literal(LiteralValue::Undefined),
                    spread: arguments == Expr::Slice,
                }
            }
            SemanticValue::CreateThread {
                descriptor,
                byte_offset,
                dispatch_state,
            } => {
                self.eval(descriptor);
                self.eval(byte_offset);
                self.eval(dispatch_state);
                Expr::Thread
            }
            SemanticValue::Receiver { field } => Expr::Receiver(*field),
            SemanticValue::FunctionLiteral { .. } | SemanticValue::ClosurePlan { .. } => {
                Expr::Function
            }
            SemanticValue::PoppedTryFrame { .. } | SemanticValue::PoppedErrorRecord { .. } => {
                Expr::ExceptionRecord
            }
            SemanticValue::PopThrownValue => Expr::Thrown,
            SemanticValue::Construct { callee, arguments } => {
                self.eval(callee);
                self.eval(arguments);
                Expr::Construct
            }
            SemanticValue::Delete { object, key } => {
                self.eval(object);
                self.eval(key);
                Expr::Delete
            }
            SemanticValue::Unknown { reason, .. } => Expr::Unknown(reason_slug(reason)),
        }
    }

    fn touch(&mut self, offset: i64) {
        self.lowest = self.lowest.min(offset);
    }

    fn read(&mut self, offset: i64) -> Expr {
        if let Some(value) = self.writes.get(&offset) {
            return value.clone();
        }
        if offset < 0 {
            self.touch(offset);
            Expr::Input(offset)
        } else {
            Expr::Hole
        }
    }

    fn write(&mut self, offset: i64, value: Expr) {
        self.touch(offset);
        self.writes.insert(offset, value);
        // Assigning at or past the length extends the array.
        if let Some(length) = self.length {
            if offset >= length {
                self.length = Some(offset + 1);
            }
        }
    }

    /// Derive pops and outputs from the final stack.
    fn finish(&mut self) {
        if self.variadic && self.length.is_none() {
            self.outputs = std::mem::take(&mut self.variadic_writes);
            return;
        }
        let Some(length) = self.length else {
            // A length set from a non-constant value is itself the effect;
            // anything else that loses the length is not modeled.
            if !self.effects.contains(&Effect::SetLength) {
                self.effects
                    .push(Effect::Unknown("dynamic_stack_length".into()));
            }
            return;
        };
        let mut lowest = self.lowest.min(0);
        let mut outputs: Vec<Expr> = (lowest..length)
            .map(|offset| {
                self.writes.get(&offset).cloned().unwrap_or(if offset < 0 {
                    Expr::Input(offset)
                } else {
                    Expr::Hole
                })
            })
            .collect();
        // A slot that is only peeked at keeps its value: it is neither
        // popped nor pushed.
        while lowest < 0 && outputs.first() == Some(&Expr::Input(lowest)) {
            outputs.remove(0);
            lowest += 1;
        }
        self.pops = (-lowest) as u32;
        self.outputs = outputs;
    }

    /// The handler's kind and nature.
    fn kind(&self) -> (String, OpNature) {
        if let Some(reason) = self.effects.iter().find_map(|effect| match effect {
            Effect::Unknown(reason) => Some(reason.clone()),
            _ => None,
        }) {
            return (format!("unrecognized.{reason}"), OpNature::Unrecognized);
        }
        if let Some(reason) = self
            .outputs
            .iter()
            .chain(self.effects.iter().filter_map(effect_value))
            .find_map(first_unknown)
        {
            return (format!("unrecognized.{reason}"), OpNature::Unrecognized);
        }

        let mut components: Vec<(String, OpNature)> = Vec::new();
        let mut effects = self.effects.as_slice();
        while let Some((component, rest)) = control_component(effects) {
            components.push(component);
            effects = rest;
        }
        for effect in effects {
            components.push(match effect {
                Effect::MemoryWrite(_) => ("memory.write".into(), OpNature::Vm),
                Effect::MemoryInitialize => ("memory.initialize".into(), OpNature::Vm),
                Effect::PropertyWrite => ("property.set".into(), OpNature::JavaScript),
                Effect::Evaluate(value) => value_component(value),
                Effect::Return(_) => ("return".into(), OpNature::JavaScript),
                Effect::Throw(_) => ("throw".into(), OpNature::JavaScript),
                Effect::PushFrame => ("vm.call_frame.push".into(), OpNature::Vm),
                Effect::PopFrame => ("vm.call_frame.pop".into(), OpNature::Vm),
                Effect::Jump => ("vm.jump".into(), OpNature::Vm),
                Effect::SetState => ("vm.dispatch_state".into(), OpNature::Vm),
                // `if (!x) goto` is how `if`/`while` skip a body.
                Effect::JumpIf(Expr::Unary(PrimitiveOperator::LogicalNot, _)) => {
                    ("vm.jump_unless".into(), OpNature::Vm)
                }
                Effect::JumpIf(_) => ("vm.jump_if".into(), OpNature::Vm),
                Effect::SetLength => ("stack.set_length".into(), OpNature::Vm),
                Effect::TryPush
                | Effect::TryPop
                | Effect::FinallyPush
                | Effect::ErrorPop
                | Effect::Rethrow
                | Effect::DropThrown => ("vm.exception".into(), OpNature::Vm),
                Effect::Unknown(_) => unreachable!("unknown effects returned above"),
            });
        }
        components.extend(self.output_components());

        match components.len() {
            0 if self.pops > 0 => ("stack.pop".into(), OpNature::Vm),
            0 => ("nop".into(), OpNature::Vm),
            1 => components.remove(0),
            _ => (
                components
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(" + "),
                OpNature::Fused,
            ),
        }
    }

    fn output_components(&self) -> Vec<(String, OpNature)> {
        // Stack shuffles: every output is an input.
        if !self.outputs.is_empty()
            && self
                .outputs
                .iter()
                .all(|output| matches!(output, Expr::Input(_)))
        {
            let name = match (self.pops, self.outputs.as_slice()) {
                (0, [Expr::Input(-1)]) | (1, [Expr::Input(-1), Expr::Input(-1)]) => "stack.dup",
                (2, [Expr::Input(-1), Expr::Input(-2)]) => "stack.swap",
                (_, outputs) if outputs.len() as u32 > self.pops => "stack.copy",
                _ => "stack.shuffle",
            };
            return vec![(name.into(), OpNature::Vm)];
        }
        self.outputs.iter().map(value_component).collect()
    }
}

/// `push frame; jump; set state` is a VM call, `pop frame; jump; set state`
/// a VM return, and `jump [; set state]` a jump.
fn control_component(effects: &[Effect]) -> Option<((String, OpNature), &[Effect])> {
    let (name, rest) = match effects {
        [Effect::PushFrame, Effect::Jump, Effect::SetState, rest @ ..] => ("vm.call", rest),
        [Effect::PopFrame, Effect::Jump, Effect::SetState, rest @ ..] => ("vm.return", rest),
        [Effect::Jump, Effect::SetState, rest @ ..] => ("vm.jump", rest),
        [Effect::TryPush, rest @ ..] => ("vm.try.enter", rest),
        [Effect::TryPop, rest @ ..] => ("vm.try.exit", rest),
        [Effect::FinallyPush, Effect::TryPop, Effect::Jump, Effect::SetState, rest @ ..] => {
            ("vm.finally.enter", rest)
        }
        [Effect::ErrorPop, Effect::Rethrow, Effect::Jump, Effect::SetState, rest @ ..] => {
            ("vm.finally.end", rest)
        }
        [Effect::ErrorPop, Effect::DropThrown, rest @ ..] => ("vm.catch", rest),
        [Effect::ErrorPop, Effect::MemoryWrite(Expr::Thrown), rest @ ..] => ("vm.catch.bind", rest),
        _ => return None,
    };
    Some(((name.into(), OpNature::Vm), rest))
}

/// The component a pushed or evaluated value performs.
fn value_component(value: &Expr) -> (String, OpNature) {
    let (name, nature): (String, OpNature) = match value {
        Expr::Binary(operator, ..) => (
            format!("binary.{}", operator_name(*operator)),
            OpNature::JavaScript,
        ),
        Expr::Unary(operator, _) => (
            format!("unary.{}", operator_name(*operator)),
            OpNature::JavaScript,
        ),
        Expr::Literal(_) | Expr::Hole => ("push.literal".into(), OpNature::JavaScript),
        Expr::Operand(_) => ("push.operand".into(), OpNature::Vm),
        Expr::Memory(_) => ("memory.read".into(), OpNature::Vm),
        Expr::Binding(_) => ("push.binding".into(), OpNature::JavaScript),
        Expr::Property(object, key) => match (object.as_ref(), key.as_ref()) {
            // A constant table read at a bytecode index is a string decode.
            (Expr::Binding(_), Expr::Operand(_)) => ("push.string_table".into(), OpNature::Vm),
            (Expr::Binding(_), _) => ("global.get".into(), OpNature::JavaScript),
            _ => ("property.get".into(), OpNature::JavaScript),
        },
        Expr::Object => ("object.new".into(), OpNature::JavaScript),
        Expr::Array => ("array.new".into(), OpNature::JavaScript),
        Expr::Slice => ("stack.slice".into(), OpNature::Vm),
        Expr::Call {
            callee,
            plain,
            spread,
        } => call_component(callee, *plain, *spread),
        Expr::Frame(_) => ("vm.call_frame.read".into(), OpNature::Vm),
        Expr::Thread => ("thread.create".into(), OpNature::Vm),
        // `receiverObject` is the sloppy-mode `this` (null and undefined
        // become the global object); `receiverValue` is the raw one.
        Expr::Receiver(ReceiverField::Object) => ("push.this".into(), OpNature::JavaScript),
        Expr::Receiver(ReceiverField::Value) => ("push.this_value".into(), OpNature::JavaScript),
        Expr::Function => ("push.function".into(), OpNature::JavaScript),
        Expr::ExceptionRecord => ("vm.exception.read".into(), OpNature::Vm),
        Expr::Thrown => ("push.thrown".into(), OpNature::Vm),
        Expr::Construct => ("call.new".into(), OpNature::JavaScript),
        Expr::Delete => ("property.delete".into(), OpNature::JavaScript),
        Expr::Input(_) => ("stack.copy".into(), OpNature::Vm),
        Expr::Length(_) => ("push.stack_length".into(), OpNature::Vm),
        Expr::Unknown(reason) => (format!("unrecognized.{reason}"), OpNature::Unrecognized),
    };
    (name, nature)
}

/// Calls are named by their callee shape, as the legacy effect labels did.
fn call_component(callee: &Expr, plain: bool, spread: bool) -> (String, OpNature) {
    let name = match callee {
        Expr::Binding(name) if name == STRING_DECODER => {
            return ("string.decode".into(), OpNature::Vm);
        }
        Expr::Property(object, key)
            if matches!(object.as_ref(), Expr::Binding(name) if name == "Object")
                && matches!(key.as_ref(), Expr::Literal(LiteralValue::String(key)) if key == "defineProperty") =>
        {
            "property.define"
        }
        _ if spread => "call.apply",
        Expr::Binding(_) if plain => "call.global",
        _ if plain => "call.function",
        _ => "call.method",
    };
    (name.into(), OpNature::JavaScript)
}

/// The pipeline's canonical name for the VM string decoder.
const STRING_DECODER: &str = "vmDecodeString";

fn effect_value(effect: &Effect) -> Option<&Expr> {
    match effect {
        Effect::MemoryWrite(value)
        | Effect::Evaluate(value)
        | Effect::Return(value)
        | Effect::Throw(value)
        | Effect::JumpIf(value) => Some(value),
        _ => None,
    }
}

fn first_unknown(value: &Expr) -> Option<String> {
    match value {
        Expr::Unknown(reason) => Some(reason.clone()),
        Expr::Memory(inner) | Expr::Unary(_, inner) => first_unknown(inner),
        Expr::Binary(_, left, right) | Expr::Property(left, right) => {
            first_unknown(left).or_else(|| first_unknown(right))
        }
        _ => None,
    }
}

fn number(value: &Expr) -> Option<f64> {
    match value {
        Expr::Literal(LiteralValue::DecodedNumber { bits }) => Some(f64::from_bits(*bits)),
        Expr::Literal(LiteralValue::Number(raw)) => parse_numeric_literal(raw),
        _ => None,
    }
}

/// Snake-case names in the vocabulary of the legacy effect labels.
fn operator_name(operator: PrimitiveOperator) -> &'static str {
    match operator {
        PrimitiveOperator::JsAdd => "add",
        PrimitiveOperator::Subtract => "subtract",
        PrimitiveOperator::Multiply => "multiply",
        PrimitiveOperator::Divide => "divide",
        PrimitiveOperator::Remainder => "remainder",
        PrimitiveOperator::Exponent => "exponent",
        PrimitiveOperator::ShiftLeft => "left_shift",
        PrimitiveOperator::ShiftRight => "right_shift",
        PrimitiveOperator::ShiftRightZeroFill => "unsigned_right_shift",
        PrimitiveOperator::BitwiseOr => "bitwise_or",
        PrimitiveOperator::BitwiseXor => "bitwise_xor",
        PrimitiveOperator::BitwiseAnd => "bitwise_and",
        PrimitiveOperator::Equal => "equality",
        PrimitiveOperator::NotEqual => "inequality",
        PrimitiveOperator::StrictEqual => "strict_equality",
        PrimitiveOperator::StrictNotEqual => "strict_inequality",
        PrimitiveOperator::LessThan => "less_than",
        PrimitiveOperator::LessThanOrEqual => "less_than_or_equal",
        PrimitiveOperator::GreaterThan => "greater_than",
        PrimitiveOperator::GreaterThanOrEqual => "greater_than_or_equal",
        PrimitiveOperator::In => "in",
        PrimitiveOperator::InstanceOf => "instanceof",
        PrimitiveOperator::UnaryPlus => "plus",
        PrimitiveOperator::UnaryMinus => "negate",
        PrimitiveOperator::BitwiseNot => "bitwise_not",
        PrimitiveOperator::LogicalNot => "not",
        PrimitiveOperator::Void => "void",
        PrimitiveOperator::Typeof => "typeof",
    }
}

/// A short stable slug for an unknown fragment's reason.
fn reason_slug(reason: &str) -> String {
    const SLUGS: &[(&str, &str)] = &[
        ("call expression", "call"),
        ("call target", "call"),
        ("control flow", "control_flow"),
        ("memory helper", "memory_helper"),
        ("unary operator", "unary_operator"),
        ("binary operator", "binary_operator"),
        ("computed read receiver", "vm_internal_read"),
        ("computed member assignment", "vm_internal_write"),
        ("member assignment", "vm_internal_write"),
        ("static member", "vm_internal_read"),
        ("assignment", "assignment"),
        ("identifier binding", "binding"),
        ("expression is outside", "expression"),
    ];
    SLUGS
        .iter()
        .find(|(prefix, _)| reason.starts_with(prefix) || reason.contains(prefix))
        .map_or("other", |(_, slug)| slug)
        .into()
}

/// A classified capture with its display name, for rendering.
#[derive(Debug, Clone, Copy)]
pub struct NamedSeed<'a> {
    pub name: &'a str,
    pub classification: &'a SeedClassification,
}

#[derive(serde::Serialize)]
struct ReportJson<'a> {
    seeds: Vec<SeedJson<'a>>,
    kinds: Vec<KindJson<'a>>,
}

#[derive(serde::Serialize)]
struct SeedJson<'a> {
    name: &'a str,
    variant: &'static str,
    ops: Vec<OpJson<'a>>,
}

#[derive(serde::Serialize)]
struct OpJson<'a> {
    key: u32,
    kind: &'a str,
    nature: &'static str,
    pops: u32,
    pushes: u32,
    source: &'a str,
}

#[derive(serde::Serialize)]
struct KindJson<'a> {
    kind: &'a str,
    nature: &'static str,
    scope: &'static str,
    handlers: Vec<HandlerRefJson<'a>>,
}

#[derive(serde::Serialize)]
struct HandlerRefJson<'a> {
    seed: &'a str,
    key: u32,
}

/// Every capture's handlers and the cross-capture kind table, as JSON.
pub fn classification_json(seeds: &[NamedSeed<'_>]) -> String {
    let classifications: Vec<SeedClassification> = seeds
        .iter()
        .map(|seed| seed.classification.clone())
        .collect();
    let kinds = classify_seeds(&classifications);
    let report = ReportJson {
        seeds: seeds
            .iter()
            .map(|seed| SeedJson {
                name: seed.name,
                variant: seed.classification.variant.as_str(),
                ops: seed
                    .classification
                    .ops
                    .iter()
                    .map(|op| OpJson {
                        key: op.handler_index,
                        kind: &op.kind,
                        nature: op.nature.as_str(),
                        pops: op.pops,
                        pushes: op.pushes,
                        source: &op.source,
                    })
                    .collect(),
            })
            .collect(),
        kinds: kinds
            .iter()
            .map(|kind| KindJson {
                kind: &kind.kind,
                nature: kind.nature.as_str(),
                scope: kind.scope.as_str(),
                handlers: kind
                    .handlers
                    .iter()
                    .map(|(position, key)| HandlerRefJson {
                        seed: seeds[*position].name,
                        key: *key,
                    })
                    .collect(),
            })
            .collect(),
    };
    serde_json::to_string_pretty(&report).expect("classification serializes")
}

/// A column header: capture file names are long and differ early.
fn short_name(name: &str) -> String {
    let short: String = name.chars().take(16).collect();
    if short.len() < name.len() {
        format!("{short}…")
    } else {
        short
    }
}

/// A markdown summary: per-capture counts, then the kinds grouped by nature
/// with their cross-VM scope and per-capture counts. With `include_ops`, every
/// handler is listed by capture.
pub fn classification_markdown(seeds: &[NamedSeed<'_>], include_ops: bool) -> String {
    let classifications: Vec<SeedClassification> = seeds
        .iter()
        .map(|seed| seed.classification.clone())
        .collect();
    let kinds = classify_seeds(&classifications);
    let mut out = String::from("# Op classes\n\n");
    out.push_str("| Capture | VM | Handlers | JavaScript | VM | Fused | Unrecognized |\n");
    out.push_str("|---|---|---:|---:|---:|---:|---:|\n");
    for seed in seeds {
        let count = |nature: OpNature| {
            seed.classification
                .ops
                .iter()
                .filter(|op| op.nature == nature)
                .count()
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            seed.name,
            seed.classification.variant.as_str(),
            seed.classification.ops.len(),
            count(OpNature::JavaScript),
            count(OpNature::Vm),
            count(OpNature::Fused),
            count(OpNature::Unrecognized),
        ));
    }
    for (nature, title) in [
        (OpNature::JavaScript, "JavaScript operations"),
        (OpNature::Vm, "VM machinery"),
        (OpNature::Fused, "Fused handlers"),
        (OpNature::Unrecognized, "Unrecognized"),
    ] {
        let rows: Vec<&KindSummary> = kinds.iter().filter(|kind| kind.nature == nature).collect();
        if rows.is_empty() {
            continue;
        }
        out.push_str(&format!("\n## {title}\n\n| Kind | Scope |"));
        for seed in seeds {
            out.push_str(&format!(" {} |", short_name(seed.name)));
        }
        out.push_str("\n|---|---|");
        for _ in seeds {
            out.push_str("---:|");
        }
        out.push('\n');
        for kind in rows {
            out.push_str(&format!("| `{}` | {} |", kind.kind, kind.scope.as_str()));
            for position in 0..seeds.len() {
                let count = kind
                    .handlers
                    .iter()
                    .filter(|(seed, _)| *seed == position)
                    .count();
                out.push_str(&format!(" {count} |"));
            }
            out.push('\n');
        }
    }
    if include_ops {
        for seed in seeds {
            out.push_str(&format!(
                "\n## Handlers: {}\n\n| Key | Kind | Nature | Pops | Pushes |\n|---:|---|---|---:|---:|\n",
                seed.name
            ));
            for op in &seed.classification.ops {
                out.push_str(&format!(
                    "| {} | `{}` | {} | {} | {} |\n",
                    op.handler_index,
                    op.kind,
                    op.nature.as_str(),
                    op.pops,
                    op.pushes
                ));
            }
        }
    }
    out
}
