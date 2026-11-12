//! Lift a WASM interpreter's dispatch paths into VM operations, once.
//!
//! The interpreter's exported run function is a loop: it reads the opcode at
//! `pc`, reads its dispatch cell `base + state * stride + opcode * 2` from
//! linear memory (selector, then next state), sets the state, and branches on
//! the selector. Each selector's path decodes operands from memory after the
//! opcode, then calls a JavaScript operation or does VM bookkeeping in the
//! function's locals: try frames, `finally`/`catch` records and one call's
//! return address.
//!
//! [`lift`] evaluates every path once over abstract values (the context,
//! operand loads relative to `pc`, call results), for every try and record
//! depth the locals can hold, and names what it does as a [`SelectorOp`]: the
//! same operations a JavaScript VM's handlers do. A path it cannot name is an
//! error, never a guess. The bank layout (which locals hold which frame) is
//! that of the admitted interpreter; [`lift`] checks every path against it.

use std::collections::BTreeMap;

use wasmparser::{ExternalKind, Imports, Operator, Parser, Payload, TypeRef, ValType};

/// The most try frames the interpreter's locals hold.
pub(crate) const TRY_FRAMES: u32 = 3;
/// The most `finally`/`catch` records its locals hold.
pub(crate) const ERROR_RECORDS: u32 = 2;

/// Locals of the admitted interpreter.
const PC: u32 = 0;
const STATE: u32 = 1;
const CONTEXT: u32 = 2;
const SELECTOR: u32 = 3;
const TRY_DEPTH: u32 = 8;
/// Try frame `d` (1-based): handler pc, handler state, records at entry,
/// calls made inside.
const TRY_FRAME: [[u32; 4]; 3] = [[9, 10, 11, 12], [13, 14, 15, 16], [17, 18, 19, 20]];
const RECORD_DEPTH: u32 = 21;
/// Record `d` (1-based): saved pc, saved state, handled.
const RECORD: [[u32; 3]; 2] = [[22, 23, 24], [25, 26, 27]];
const CALL_DEPTH: u32 = 28;
const RETURN_PC: u32 = 29;
const RETURN_STATE: u32 = 30;

/// One operand: `width` bytes little-endian at `pc + 1 + offset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Operand {
    pub offset: u32,
    pub width: u8,
}

/// What one selector does, as a JavaScript VM handler would.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SelectorOp {
    /// Call JavaScript operation `operation` with these operands, then go on
    /// at `pc + 1 + length`.
    Operation {
        operation: u32,
        operands: Vec<Operand>,
        length: u32,
    },
    /// Push a literal: `{}`, `[]`, `true`, `false`, `undefined`, `null` or
    /// `Infinity` by the helper's kind.
    PushLiteral {
        kind: u32,
    },
    /// Push an operand as a number.
    PushNumber {
        operand: Operand,
    },
    /// Push entry `operand` of the f64 table at `table` in memory.
    PushTableNumber {
        table: u32,
        operand: Operand,
    },
    /// `image[address] = value`.
    ImageWrite {
        address: Operand,
        value: Operand,
    },
    Jump {
        pc: Operand,
        state: Operand,
    },
    /// Pop a value; jump when it is truthy (or falsy when `negated`).
    JumpIf {
        negated: bool,
        pc: Operand,
        state: Operand,
        length: u32,
    },
    /// Pop the pc, then the state, and go there.
    PopJump,
    /// Return from the VM function: status 0 is `undefined`, 1 pops it.
    Return {
        status: i32,
    },
    /// Enter a `try` whose handler is at these operands.
    TryEnter {
        pc: Operand,
        state: Operand,
        length: u32,
    },
    TryLeave,
    /// Leave a `try` normally into its `finally`: push a record resuming
    /// after this instruction and go to the handler.
    FinallyEnter,
    /// End a `finally`: pop its record; a handled one rethrows, another
    /// resumes where it was saved.
    FinallyLeave,
    /// A `catch` without a binding: pop its record and, if handled, a
    /// thrown value.
    CatchDrop,
    /// A `catch` binding: pop its record and write the thrown value to slot
    /// `slot`.
    CatchBind {
        slot: Operand,
    },
    /// Call a VM subroutine, returning after this instruction.
    Call {
        pc: Operand,
        state: Operand,
        length: u32,
    },
    CallReturn,
    /// `unreachable` (or an unmapped selector): the run function traps.
    Trap,
}

/// What one of the interpreter's helper imports does, as its JavaScript
/// function shows (see the WASM preparation, which reads them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Helper {
    /// `return !!context.stack.pop()`.
    Truth,
    /// Push a literal by kind.
    Literal,
    /// Push its number.
    Number,
    /// `return context.stack.pop()`.
    Pop,
    /// `context.memory.writeSlot(slot, context.errorStack.pop())`.
    CatchBind,
    /// `throw context.errorStack.pop()`.
    Rethrow,
    /// Drop `n` thrown values under the top one.
    DropUnder,
    /// Call `vmOperations[selector]` guarded: a throw sets the flag.
    Invoke,
}

/// The interpreter, lifted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lifted {
    pub base: u32,
    pub stride: u32,
    pub memory_bytes: usize,
    /// Indexed by selector; selectors past the branch table trap.
    pub selectors: Vec<SelectorOp>,
    /// The run function's and the flag setter's export names.
    pub run_export: String,
    pub flag_export: String,
}

impl Lifted {
    /// What `selector` does; any byte past the branch table traps.
    pub(crate) fn op(&self, selector: u8) -> &SelectorOp {
        self.selectors
            .get(selector as usize)
            .unwrap_or(&SelectorOp::Trap)
    }
}

/// Instructions of the run function, as the evaluator reads them.
#[derive(Debug, Clone, PartialEq)]
enum Op {
    Block,
    Loop,
    If,
    Else,
    End,
    Br(u32),
    BrIf(u32),
    BrTable(Vec<u32>, u32),
    Return,
    Unreachable,
    Drop,
    Call(u32),
    CallIndirect { type_index: u32, table: u32 },
    LocalGet(u32),
    LocalSet(u32),
    LocalTee(u32),
    GlobalGet(u32),
    GlobalSet(u32),
    Const(i64),
    Add,
    Sub,
    Mul,
    Shl,
    And,
    Ne,
    Load { width: u8, offset: u32 },
    F64Load(u32),
    Store8(u32),
    F64ConvertU,
    Other(String),
}

/// An abstract value.
#[derive(Debug, Clone, PartialEq)]
enum V {
    Context,
    /// `pc + k` at the path's start.
    Pc(i64),
    Selector,
    /// The next state the dispatch cell gave.
    State,
    Const(i64),
    /// An operand: `width` bytes at `pc + offset` (offset counts the opcode).
    Load(u32, u8),
    /// A full four-byte load, masked to an operand only by `& 0xFFFFFF`.
    Wide(u32),
    Shl(Box<V>, i64),
    Number(Box<V>),
    TableNumber {
        table: u32,
        index: Box<V>,
    },
    /// A local's value when the path began.
    Reg(u32),
    /// The result of the `n`th call on the path.
    Result(usize),
    /// `value + k`.
    Plus(Box<V>, i64),
    /// Arithmetic on values the path does not know.
    Bin(&'static str, Box<V>, Box<V>),
}

#[derive(Debug, Clone, PartialEq)]
enum Effect {
    Call { function: u32, args: Vec<V> },
    CallIndirect { index: V, args: Vec<V> },
    Store8 { address: V, value: V },
    SetFlag,
    ClearFlag,
}

#[derive(Debug, Clone, PartialEq)]
enum End {
    /// Back to the dispatch loop.
    Next,
    /// To the check of the exception flag after an operation call.
    Checked,
    Return(i64),
    Trap,
}

/// One way through a path.
#[derive(Debug, Clone)]
struct Outcome {
    /// The symbolic tests taken, with their outcomes.
    tests: Vec<(V, bool)>,
    effects: Vec<Effect>,
    locals: BTreeMap<u32, V>,
    end: End,
}

impl Outcome {
    fn local(&self, local: u32) -> &V {
        self.locals.get(&local).expect("every local is initialized")
    }
}

struct Module<'a> {
    types: Vec<(usize, usize)>,
    /// (namespace, name, type) of each imported function, in index order.
    imports: Vec<(String, String, u32)>,
    defined_types: Vec<u32>,
    memory_pages: Option<u64>,
    /// Exported functions: (name, function index).
    exports: Vec<(String, u32)>,
    bodies: Vec<wasmparser::FunctionBody<'a>>,
}

impl Module<'_> {
    /// The instructions of function `index`, when the module defines it.
    fn body(&self, index: u32) -> Option<Result<Vec<Op>, String>> {
        let imported = self.imports.len() as u32;
        let body = self.bodies.get(index.checked_sub(imported)? as usize)?;
        Some(operators(body))
    }
}

/// Lift `interpreter`, its helper imports doing what `roles` says by name.
/// Its run function is the export that runs the dispatch loop; its flag
/// setter is the export that only sets global 0 to 1.
pub(crate) fn lift(interpreter: &[u8], roles: &BTreeMap<String, Helper>) -> Result<Lifted, String> {
    let module = parse(interpreter)?;
    let memory_bytes = module
        .memory_pages
        .ok_or("the interpreter imports no memory")? as usize
        * 65_536;
    let helpers: BTreeMap<u32, Helper> = module
        .imports
        .iter()
        .enumerate()
        .filter_map(|(index, (_, name, _))| Some((index as u32, *roles.get(name)?)))
        .collect();
    let mut flag_export = None;
    let mut run = None;
    for (name, index) in &module.exports {
        let Some(ops) = module.body(*index) else {
            continue;
        };
        let ops = ops?;
        if ops == [Op::Const(1), Op::GlobalSet(0), Op::End] {
            if flag_export.replace(name.clone()).is_some() {
                return Err("two exports set the exception flag".into());
            }
        } else if ops.first() == Some(&Op::Loop) && run.replace((name.clone(), ops)).is_some() {
            return Err("two exports run a loop".into());
        }
    }
    let flag_export = flag_export.ok_or("no export sets the exception flag")?;
    let (run_export, ops) = run.ok_or("no export runs the dispatch loop")?;
    let structure = Structure::of(&ops)?;

    // The dispatch: `local3 = mem[cell]; local1 = mem[cell + 1]` for
    // `cell = base + local1 * stride + mem[local0] * 2`, then `br_table` on
    // local 3.
    let prologue = [
        Op::Loop,
        Op::Const(0),
        Op::LocalGet(STATE),
        Op::Const(0),
        Op::Mul,
        Op::LocalGet(PC),
        Op::Load {
            width: 1,
            offset: 0,
        },
        Op::LocalTee(SELECTOR),
        Op::Const(1),
        Op::Shl,
        Op::Add,
        Op::Add,
        Op::LocalTee(31),
        Op::Load {
            width: 1,
            offset: 0,
        },
        Op::LocalSet(SELECTOR),
        Op::LocalGet(31),
        Op::Load {
            width: 1,
            offset: 1,
        },
        Op::LocalSet(STATE),
    ];
    let shape = |op: &Op| match op {
        Op::Const(_) => Op::Const(0),
        other => other.clone(),
    };
    if ops.len() < prologue.len()
        || ops[..prologue.len()]
            .iter()
            .map(shape)
            .ne(prologue.iter().map(shape))
    {
        return Err("the run function does not begin with the dispatch read".into());
    }
    let (Op::Const(base), Op::Const(stride)) = (&ops[1], &ops[3]) else {
        unreachable!("checked by the prologue");
    };
    let (base, stride) = (*base as u32, *stride as u32);
    let mut at = prologue.len();
    while ops.get(at) == Some(&Op::Block) {
        at += 1;
    }
    let (Some(Op::LocalGet(SELECTOR)), Some(Op::BrTable(targets, default))) =
        (ops.get(at), ops.get(at + 1))
    else {
        return Err("the dispatch read is not followed by a branch on the selector".into());
    };
    let branch = at + 1;
    let open = structure.open_at(branch);
    let label = |depth: u32| -> Result<usize, String> {
        open.iter()
            .rev()
            .nth(depth as usize)
            .copied()
            .ok_or_else(|| format!("branch depth {depth} leaves the function"))
    };
    let check = ops
        .iter()
        .position(|op| *op == Op::GlobalGet(0))
        .ok_or("the run function never checks its exception flag")?;
    if ops.iter().filter(|op| **op == Op::GlobalGet(0)).count() != 1 {
        return Err("the run function reads its exception flag more than once".into());
    }
    let machine = Machine {
        ops: &ops,
        structure: &structure,
        module: &module,
        helpers: &helpers,
        dispatch_loop: 0,
        check,
    };
    let mut selectors = Vec::with_capacity(targets.len());
    for (selector, depth) in targets.iter().enumerate() {
        let start = structure.end[&label(*depth)?] + 1;
        selectors.push(
            machine
                .classify(selector as u32, start)
                .map_err(|error| format!("selector {selector}: {error}"))?,
        );
    }
    // The default branch must trap: it is what every other byte selects.
    let start = structure.end[&label(*default)?] + 1;
    if machine.classify(targets.len() as u32, start)? != SelectorOp::Trap {
        return Err("the selector branch's default does not trap".into());
    }
    machine.check_unwind()?;
    Ok(Lifted {
        base,
        stride,
        memory_bytes,
        selectors,
        run_export,
        flag_export,
    })
}

fn parse(bytes: &[u8]) -> Result<Module<'_>, String> {
    let mut module = Module {
        types: Vec::new(),
        imports: Vec::new(),
        defined_types: Vec::new(),
        memory_pages: None,
        exports: Vec::new(),
        bodies: Vec::new(),
    };
    let error = |error: wasmparser::BinaryReaderError| error.to_string();
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.map_err(error)? {
            Payload::TypeSection(reader) => {
                for ty in reader.into_iter_err_on_gc_types() {
                    let ty = ty.map_err(error)?;
                    module.types.push((ty.params().len(), ty.results().len()));
                    if ty.results().iter().any(|result| *result != ValType::I32) {
                        return Err("an interpreter function returns a non-i32".into());
                    }
                }
            }
            Payload::ImportSection(reader) => {
                let mut add = |namespace: &str, name: &str, ty: TypeRef| match ty {
                    TypeRef::Func(index) | TypeRef::FuncExact(index) => {
                        module.imports.push((namespace.into(), name.into(), index))
                    }
                    TypeRef::Memory(memory) => module.memory_pages = Some(memory.initial),
                    _ => {}
                };
                for group in reader {
                    match group.map_err(error)? {
                        Imports::Single(_, import) => add(import.module, import.name, import.ty),
                        Imports::Compact1 {
                            module: namespace,
                            items,
                        } => {
                            for item in items {
                                let item = item.map_err(error)?;
                                add(namespace, item.name, item.ty);
                            }
                        }
                        Imports::Compact2 {
                            module: namespace,
                            ty,
                            names,
                        } => {
                            for name in names {
                                add(namespace, name.map_err(error)?, ty);
                            }
                        }
                    }
                }
            }
            Payload::FunctionSection(reader) => {
                for ty in reader {
                    module.defined_types.push(ty.map_err(error)?);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let export = export.map_err(error)?;
                    if export.kind == ExternalKind::Func {
                        module.exports.push((export.name.into(), export.index));
                    }
                }
            }
            Payload::CodeSectionEntry(body) => module.bodies.push(body),
            _ => {}
        }
    }
    Ok(module)
}

fn operators(body: &wasmparser::FunctionBody<'_>) -> Result<Vec<Op>, String> {
    let mut ops = Vec::new();
    let reader = body.get_operators_reader().map_err(|e| e.to_string())?;
    for operator in reader {
        let operator = operator.map_err(|e| e.to_string())?;
        let offset = |memarg: wasmparser::MemArg| memarg.offset as u32;
        ops.push(match operator {
            Operator::Block { .. } => Op::Block,
            Operator::Loop { .. } => Op::Loop,
            Operator::If { .. } => Op::If,
            Operator::Else => Op::Else,
            Operator::End => Op::End,
            Operator::Br { relative_depth } => Op::Br(relative_depth),
            Operator::BrIf { relative_depth } => Op::BrIf(relative_depth),
            Operator::BrTable { targets } => {
                let default = targets.default();
                let targets = targets
                    .targets()
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| e.to_string())?;
                Op::BrTable(targets, default)
            }
            Operator::Return => Op::Return,
            Operator::Unreachable => Op::Unreachable,
            Operator::Drop => Op::Drop,
            Operator::Call { function_index } => Op::Call(function_index),
            Operator::CallIndirect {
                type_index,
                table_index,
            } => Op::CallIndirect {
                type_index,
                table: table_index,
            },
            Operator::LocalGet { local_index } => Op::LocalGet(local_index),
            Operator::LocalSet { local_index } => Op::LocalSet(local_index),
            Operator::LocalTee { local_index } => Op::LocalTee(local_index),
            Operator::GlobalGet { global_index } => Op::GlobalGet(global_index),
            Operator::GlobalSet { global_index } => Op::GlobalSet(global_index),
            Operator::I32Const { value } => Op::Const(value as i64),
            Operator::I32Add => Op::Add,
            Operator::I32Sub => Op::Sub,
            Operator::I32Mul => Op::Mul,
            Operator::I32Shl => Op::Shl,
            Operator::I32And => Op::And,
            Operator::I32Ne => Op::Ne,
            Operator::I32Load8U { memarg } => Op::Load {
                width: 1,
                offset: offset(memarg),
            },
            Operator::I32Load16U { memarg } => Op::Load {
                width: 2,
                offset: offset(memarg),
            },
            Operator::I32Load { memarg } => Op::Load {
                width: 4,
                offset: offset(memarg),
            },
            Operator::F64Load { memarg } => Op::F64Load(offset(memarg)),
            Operator::I32Store8 { memarg } => Op::Store8(offset(memarg)),
            Operator::F64ConvertI32U => Op::F64ConvertU,
            other => Op::Other(format!("{other:?}")),
        });
    }
    Ok(ops)
}

/// The function's blocks: where each ends and where its `else` is.
struct Structure {
    end: BTreeMap<usize, usize>,
    otherwise: BTreeMap<usize, usize>,
    /// The innermost block each instruction is in, by index.
    parent: Vec<Option<usize>>,
}

impl Structure {
    fn of(ops: &[Op]) -> Result<Self, String> {
        let mut end = BTreeMap::new();
        let mut otherwise = BTreeMap::new();
        let mut parent = Vec::with_capacity(ops.len());
        let mut open: Vec<usize> = Vec::new();
        for (index, op) in ops.iter().enumerate() {
            parent.push(open.last().copied());
            match op {
                Op::Block | Op::Loop | Op::If => open.push(index),
                Op::Else => {
                    let block = *open.last().ok_or("an else outside a block")?;
                    otherwise.insert(block, index);
                }
                Op::End => {
                    // The function body's own end closes nothing.
                    if let Some(block) = open.pop() {
                        end.insert(block, index);
                    }
                }
                _ => {}
            }
        }
        Ok(Self {
            end,
            otherwise,
            parent,
        })
    }

    /// The blocks open at `index`, outermost first.
    fn open_at(&self, index: usize) -> Vec<usize> {
        let mut open = Vec::new();
        let mut at = self.parent[index];
        while let Some(block) = at {
            open.push(block);
            at = self.parent[block];
        }
        open.reverse();
        open
    }
}

struct Machine<'m, 'a> {
    ops: &'m [Op],
    structure: &'m Structure,
    module: &'m Module<'a>,
    helpers: &'m BTreeMap<u32, Helper>,
    dispatch_loop: usize,
    check: usize,
}

/// Paths are short: one this long is not a dispatch path.
const MAX_PATH_STEPS: usize = 4_000;

impl Machine<'_, '_> {
    /// Every way through the code from `start`, the locals as `initial`.
    fn run(&self, start: usize, initial: BTreeMap<u32, V>) -> Result<Vec<Outcome>, String> {
        let mut done = Vec::new();
        let mut pending = vec![(
            start,
            Vec::<V>::new(),
            Outcome {
                tests: Vec::new(),
                effects: Vec::new(),
                locals: initial,
                end: End::Trap,
            },
            0usize,
        )];
        while let Some((mut at, mut stack, mut outcome, mut steps)) = pending.pop() {
            loop {
                steps += 1;
                if steps > MAX_PATH_STEPS {
                    return Err("a path runs too long".into());
                }
                if at == self.check {
                    outcome.end = End::Checked;
                    break;
                }
                let op = self.ops.get(at).ok_or("a path runs off the function")?;
                let mut pop = || stack.pop().ok_or("a path pops an empty stack");
                match op {
                    Op::Block | Op::Loop => at += 1,
                    Op::If => {
                        let condition = pop()?;
                        let else_or_end = self
                            .structure
                            .otherwise
                            .get(&at)
                            .or_else(|| self.structure.end.get(&at))
                            .copied()
                            .ok_or("an if without an end")?;
                        let taken = at + 1;
                        let skipped = else_or_end + 1;
                        match truth(&condition) {
                            Some(true) => at = taken,
                            Some(false) => at = skipped,
                            None => {
                                let mut other = outcome.clone();
                                other.tests.push((condition.clone(), false));
                                pending.push((skipped, stack.clone(), other, steps));
                                outcome.tests.push((condition, true));
                                at = taken;
                            }
                        }
                    }
                    // The `then` arm ends: skip the `else` arm.
                    Op::Else => {
                        let block = self.structure.parent[at].ok_or("an else outside a block")?;
                        at = self.structure.end[&block] + 1;
                    }
                    Op::End => at += 1,
                    Op::Br(depth) => match self.branch(at, *depth)? {
                        Some(next) => at = next,
                        None => {
                            outcome.end = End::Next;
                            break;
                        }
                    },
                    Op::BrIf(depth) => {
                        let condition = pop()?;
                        match truth(&condition) {
                            Some(true) => match self.branch(at, *depth)? {
                                Some(next) => at = next,
                                None => {
                                    outcome.end = End::Next;
                                    break;
                                }
                            },
                            Some(false) => at += 1,
                            None => return Err("a branch on a value the path does not know".into()),
                        }
                    }
                    Op::BrTable(targets, default) => {
                        let V::Const(index) = pop()? else {
                            return Err("a branch table on a value the path does not know".into());
                        };
                        let depth = usize::try_from(index)
                            .ok()
                            .and_then(|index| targets.get(index))
                            .unwrap_or(default);
                        match self.branch(at, *depth)? {
                            Some(next) => at = next,
                            None => {
                                outcome.end = End::Next;
                                break;
                            }
                        }
                    }
                    Op::Return => {
                        let V::Const(status) = pop()? else {
                            return Err("a return of a value the path does not know".into());
                        };
                        outcome.end = End::Return(status);
                        break;
                    }
                    Op::Unreachable => {
                        outcome.end = End::Trap;
                        break;
                    }
                    Op::Drop => {
                        pop()?;
                        at += 1;
                    }
                    Op::Call(function) => {
                        let ty = self.type_of(*function)?;
                        let (params, results) = self.module.types[ty as usize];
                        let mut args = (0..params).map(|_| pop()).collect::<Result<Vec<_>, _>>()?;
                        args.reverse();
                        outcome.effects.push(Effect::Call {
                            function: *function,
                            args,
                        });
                        if results == 1 {
                            stack.push(V::Result(outcome.effects.len() - 1));
                        }
                        at += 1;
                    }
                    Op::CallIndirect { type_index, table } => {
                        if *table != 0 {
                            return Err("an indirect call through another table".into());
                        }
                        let index = pop()?;
                        let (params, results) = self.module.types[*type_index as usize];
                        if results != 0 {
                            return Err("an indirect call with a result".into());
                        }
                        let mut args = (0..params).map(|_| pop()).collect::<Result<Vec<_>, _>>()?;
                        args.reverse();
                        outcome.effects.push(Effect::CallIndirect { index, args });
                        at += 1;
                    }
                    Op::LocalGet(local) => {
                        let value = outcome
                            .locals
                            .get(local)
                            .cloned()
                            .ok_or_else(|| format!("local {local} is not modeled"))?;
                        stack.push(value);
                        at += 1;
                    }
                    Op::LocalSet(local) => {
                        let value = pop()?;
                        outcome.locals.insert(*local, value);
                        at += 1;
                    }
                    Op::LocalTee(local) => {
                        let value = pop()?;
                        outcome.locals.insert(*local, value.clone());
                        stack.push(value);
                        at += 1;
                    }
                    Op::GlobalSet(0) => {
                        outcome.effects.push(match pop()? {
                            V::Const(1) => Effect::SetFlag,
                            V::Const(0) => Effect::ClearFlag,
                            _ => {
                                return Err("the exception flag is set to other than 0 or 1".into())
                            }
                        });
                        at += 1;
                    }
                    Op::Const(value) => {
                        stack.push(V::Const(*value));
                        at += 1;
                    }
                    Op::Add | Op::Sub | Op::Mul | Op::Shl | Op::And | Op::Ne => {
                        let right = pop()?;
                        let left = pop()?;
                        stack.push(arithmetic(op, left, right)?);
                        at += 1;
                    }
                    Op::Load { width, offset } => {
                        let V::Pc(k) = pop()? else {
                            return Err("a load from other than the pc".into());
                        };
                        let offset = u32::try_from(k + *offset as i64)
                            .map_err(|_| "a load before the pc")?;
                        stack.push(if *width == 4 {
                            V::Wide(offset)
                        } else {
                            V::Load(offset, *width)
                        });
                        at += 1;
                    }
                    Op::F64Load(table) => {
                        let index = pop()?;
                        stack.push(V::TableNumber {
                            table: *table,
                            index: Box::new(index),
                        });
                        at += 1;
                    }
                    Op::Store8(offset) => {
                        let value = pop()?;
                        let address = pop()?;
                        if *offset != 0 {
                            return Err("an image write at an offset".into());
                        }
                        outcome.effects.push(Effect::Store8 { address, value });
                        at += 1;
                    }
                    Op::F64ConvertU => {
                        let value = pop()?;
                        stack.push(V::Number(Box::new(value)));
                        at += 1;
                    }
                    Op::GlobalGet(_) | Op::GlobalSet(_) | Op::Other(_) => {
                        return Err(format!("a path does {op:?}"));
                    }
                }
            }
            done.push(outcome);
        }
        Ok(done)
    }

    /// Where `br depth` at `at` goes: after the block it names, or `None`
    /// back to the dispatch loop.
    fn branch(&self, at: usize, depth: u32) -> Result<Option<usize>, String> {
        let open = self.structure.open_at(at);
        let target = *open
            .iter()
            .rev()
            .nth(depth as usize)
            .ok_or("a branch out of the function")?;
        if target == self.dispatch_loop {
            return Ok(None);
        }
        if self.ops[target] == Op::Loop {
            return Err("a branch to an inner loop".into());
        }
        Ok(Some(self.structure.end[&target] + 1))
    }

    fn type_of(&self, function: u32) -> Result<u32, String> {
        let imported = self.module.imports.len() as u32;
        if function < imported {
            return Ok(self.module.imports[function as usize].2);
        }
        self.module
            .defined_types
            .get((function - imported) as usize)
            .copied()
            .ok_or_else(|| format!("no function {function}"))
    }

    fn helper(&self, function: u32) -> Option<Helper> {
        self.helpers.get(&function).copied()
    }

    /// The locals at a path's start with `tries` try frames and `records`
    /// records open.
    fn initial(tries: u32, records: u32) -> BTreeMap<u32, V> {
        let mut locals: BTreeMap<u32, V> = (0..32).map(|local| (local, V::Reg(local))).collect();
        locals.insert(PC, V::Pc(0));
        locals.insert(STATE, V::State);
        locals.insert(CONTEXT, V::Context);
        locals.insert(SELECTOR, V::Selector);
        locals.insert(TRY_DEPTH, V::Const(tries as i64));
        locals.insert(RECORD_DEPTH, V::Const(records as i64));
        locals
    }

    /// Name what `selector`'s path at `start` does, checked at every try and
    /// record depth.
    fn classify(&self, selector: u32, start: usize) -> Result<SelectorOp, String> {
        let mut named: Option<SelectorOp> = None;
        for tries in 0..=TRY_FRAMES {
            for records in 0..=ERROR_RECORDS {
                let outcomes = self.run(start, Self::initial(tries, records))?;
                let op = self
                    .name(selector, tries, records, &outcomes)
                    .map_err(|error| format!("{error} (tries {tries}, records {records})"))?;
                // A bank overflow traps at that depth only.
                let Some(op) = op else {
                    continue;
                };
                match &named {
                    None => named = Some(op),
                    Some(previous) if *previous == op => {}
                    Some(previous) => {
                        return Err(format!(
                            "does {previous:?} at one depth and {op:?} at another"
                        ))
                    }
                }
            }
        }
        // Trapping at every depth is the selector's own trap.
        Ok(named.unwrap_or(SelectorOp::Trap))
    }

    /// What one path does at these depths: `None` when it is the trap of a
    /// full bank.
    fn name(
        &self,
        selector: u32,
        tries: u32,
        records: u32,
        outcomes: &[Outcome],
    ) -> Result<Option<SelectorOp>, String> {
        let single = |outcomes: &[Outcome]| -> Result<Outcome, String> {
            match outcomes {
                [one] if one.tests.is_empty() => Ok(one.clone()),
                _ => Err("forks".into()),
            }
        };
        let changed = |outcome: &Outcome, local: u32| {
            outcome.local(local) != &Self::initial(tries, records)[&local]
        };
        let bookkeeping = |outcome: &Outcome, allowed: &[u32]| -> Result<(), String> {
            for local in [TRY_DEPTH, RECORD_DEPTH, CALL_DEPTH, RETURN_PC, RETURN_STATE]
                .into_iter()
                .chain(TRY_FRAME.into_iter().flatten())
                .chain(RECORD.into_iter().flatten())
            {
                if changed(outcome, local) && !allowed.contains(&local) {
                    return Err(format!("changes local {local}"));
                }
            }
            Ok(())
        };
        // A trap at these depths: a full (or empty) bank. One at every
        // depth is the selector's own, decided by [`Self::classify`].
        if outcomes.iter().all(|outcome| outcome.end == End::Trap) {
            if outcomes
                .iter()
                .any(|outcome| !outcome.effects.is_empty() || !outcome.tests.is_empty())
            {
                return Err("traps after doing something".into());
            }
            return Ok(None);
        }
        if outcomes.len() == 2 {
            return self.name_forked(tries, records, outcomes);
        }

        // Operation calls end at the flag check.
        if outcomes.iter().any(|outcome| outcome.end == End::Checked) {
            let outcome = single(outcomes)?;
            bookkeeping(&outcome, &[])?;
            let [effect] = outcome.effects.as_slice() else {
                return Err("does other than one operation call".into());
            };
            let operation_args = match effect {
                // Outside a try: the table entry itself.
                Effect::CallIndirect { index, args } if tries == 0 => {
                    if *index != V::Selector {
                        return Err("calls a table entry other than its selector".into());
                    }
                    args.clone()
                }
                // Inside one: a guarded wrapper, the selector last.
                Effect::Call { function, args } if tries > 0 => {
                    if self.helper(*function) != Some(Helper::Invoke) {
                        return Err("calls a helper other than an operation wrapper".into());
                    }
                    let Some((last, rest)) = args.split_last() else {
                        return Err("a wrapper call without the selector".into());
                    };
                    if *last != V::Selector {
                        return Err("a wrapper call for another selector".into());
                    }
                    rest.to_vec()
                }
                _ => return Err("calls an operation the way it should not at this depth".into()),
            };
            let Some((V::Context, operands)) = operation_args.split_first() else {
                return Err("an operation call without the context first".into());
            };
            let operands = operands
                .iter()
                .map(operand)
                .collect::<Result<Vec<_>, _>>()?;
            let V::Pc(length) = outcome.local(PC) else {
                return Err("an operation call leaves the pc unknown".into());
            };
            if *outcome.local(STATE) != V::State {
                return Err("an operation call changes the state".into());
            }
            return Ok(Some(SelectorOp::Operation {
                operation: selector,
                operands,
                length: (*length - 1) as u32,
            }));
        }

        if let [outcome] = outcomes {
            if let End::Return(status) = outcome.end {
                if !outcome.effects.is_empty() {
                    return Err("returns after doing something".into());
                }
                return Ok(Some(SelectorOp::Return {
                    status: status as i32,
                }));
            }
        }

        // The rest go back to the dispatch loop.
        let outcome = single(outcomes)?;
        if outcome.end != End::Next {
            return Err("ends other than back at the dispatch".into());
        }
        let pc = outcome.local(PC).clone();
        let state = outcome.local(STATE).clone();
        let effects = outcome.effects.as_slice();
        let helper_call = |effect: &Effect| match effect {
            Effect::Call { function, args } => {
                self.helper(*function).map(|name| (name, args.clone()))
            }
            _ => None,
        };

        // Bank operations first: they change the bank's locals.
        if changed(&outcome, TRY_DEPTH) {
            let depth = outcome.local(TRY_DEPTH);
            if *depth == V::Const(tries as i64 + 1) {
                // Try enter: frame `tries + 1` holds the handler.
                let frame = TRY_FRAME[tries as usize];
                bookkeeping(
                    &outcome,
                    &[TRY_DEPTH, frame[0], frame[1], frame[2], frame[3]],
                )?;
                if !effects.is_empty()
                    || *outcome.local(frame[2]) != V::Const(0)
                    || *outcome.local(frame[3]) != V::Const(0)
                    || state != V::State
                {
                    return Err("a try entry does more".into());
                }
                let V::Pc(length) = pc else {
                    return Err("a try entry leaves the pc unknown".into());
                };
                return Ok(Some(SelectorOp::TryEnter {
                    pc: operand(outcome.local(frame[0]))?,
                    state: operand(outcome.local(frame[1]))?,
                    length: (length - 1) as u32,
                }));
            }
            if *depth == V::Const(tries as i64 - 1) {
                if pc == V::Pc(1) && state == V::State && effects.is_empty() {
                    bookkeeping(&outcome, &[TRY_DEPTH])?;
                    return Ok(Some(SelectorOp::TryLeave));
                }
                let frame = TRY_FRAME[tries.checked_sub(1).ok_or("leaves no try")? as usize];
                // Into the frame's `finally`: a record resumes after this.
                if records < ERROR_RECORDS
                    && pc == V::Reg(frame[0])
                    && state == V::Reg(frame[1])
                    && effects.is_empty()
                {
                    let record = RECORD[records as usize];
                    bookkeeping(
                        &outcome,
                        &[TRY_DEPTH, RECORD_DEPTH, record[0], record[1], record[2]],
                    )?;
                    if *outcome.local(RECORD_DEPTH) == V::Const(records as i64 + 1)
                        && *outcome.local(record[0]) == V::Pc(1)
                        && *outcome.local(record[1]) == V::State
                        && *outcome.local(record[2]) == V::Const(0)
                    {
                        return Ok(Some(SelectorOp::FinallyEnter));
                    }
                }
            }
            return Err("changes the try depth some other way".into());
        }
        if changed(&outcome, CALL_DEPTH) {
            let frame = tries
                .checked_sub(1)
                .map(|depth| TRY_FRAME[depth as usize][3]);
            let counted = |change: i64| match frame {
                Some(calls) => *outcome.local(calls) == plus(V::Reg(calls), change),
                None => true,
            };
            let mut allowed = vec![CALL_DEPTH, RETURN_PC, RETURN_STATE];
            allowed.extend(frame);
            if *outcome.local(CALL_DEPTH) == plus(V::Reg(CALL_DEPTH), 1) && counted(1) {
                bookkeeping(&outcome, &allowed)?;
                let V::Pc(length) = outcome.local(RETURN_PC) else {
                    return Err("a call saves an unknown return pc".into());
                };
                if *outcome.local(RETURN_STATE) != V::State || !effects.is_empty() {
                    return Err("a call saves another state".into());
                }
                return Ok(Some(SelectorOp::Call {
                    pc: operand(&pc)?,
                    state: operand(&state)?,
                    length: (*length - 1) as u32,
                }));
            }
            if *outcome.local(CALL_DEPTH) == plus(V::Reg(CALL_DEPTH), -1)
                && counted(-1)
                && pc == V::Reg(RETURN_PC)
                && state == V::Reg(RETURN_STATE)
                && effects.is_empty()
            {
                bookkeeping(&outcome, &allowed)?;
                return Ok(Some(SelectorOp::CallReturn));
            }
            return Err("changes the call depth some other way".into());
        }
        if changed(&outcome, RECORD_DEPTH) {
            // A `catch` binding: write the thrown value to a slot, pop.
            if let [effect] = effects {
                if let Some((Helper::CatchBind, args)) = helper_call(effect) {
                    if *outcome.local(RECORD_DEPTH) == V::Const(records as i64 - 1)
                        && pc != V::Pc(0)
                        && state == V::State
                    {
                        bookkeeping(&outcome, &[RECORD_DEPTH])?;
                        if let [V::Context, slot] = args.as_slice() {
                            return Ok(Some(SelectorOp::CatchBind {
                                slot: operand(slot)?,
                            }));
                        }
                    }
                }
            }
            return Err("changes the record depth some other way".into());
        }
        bookkeeping(&outcome, &[])?;

        match effects {
            [] => {
                if state == V::State {
                    return Err("does nothing".into());
                }
                Ok(Some(SelectorOp::Jump {
                    pc: operand(&pc)?,
                    state: operand(&state)?,
                }))
            }
            [effect] => {
                if let Effect::Store8 { address, value } = effect {
                    if pc == V::Pc(5) && state == V::State {
                        return Ok(Some(SelectorOp::ImageWrite {
                            address: operand(address)?,
                            value: operand(value)?,
                        }));
                    }
                    return Err("an image write that moves on otherwise".into());
                }
                let Some((name, args)) = helper_call(effect) else {
                    return Err("calls other than a helper".into());
                };
                if state != V::State {
                    return Err("a helper call that changes the state".into());
                }
                match (name, args.as_slice()) {
                    (Helper::Literal, [V::Context, V::Const(kind)]) if pc == V::Pc(1) => {
                        Ok(Some(SelectorOp::PushLiteral { kind: *kind as u32 }))
                    }
                    (Helper::Number, [V::Context, V::Number(value)]) => {
                        let operand = operand(value)?;
                        if pc != V::Pc(past(operand)) {
                            return Err("a number push that moves on otherwise".into());
                        }
                        Ok(Some(SelectorOp::PushNumber { operand }))
                    }
                    (Helper::Number, [V::Context, V::TableNumber { table, index }]) => {
                        let V::Shl(index, 3) = index.as_ref() else {
                            return Err(
                                "a table number at an index other than an operand times 8".into()
                            );
                        };
                        let operand = operand(index)?;
                        if pc != V::Pc(past(operand)) {
                            return Err("a table number push that moves on otherwise".into());
                        }
                        Ok(Some(SelectorOp::PushTableNumber {
                            table: *table,
                            operand,
                        }))
                    }
                    _ => Err(format!("calls helper {name:?} with {args:?}")),
                }
            }
            [first, second] => match (helper_call(first), helper_call(second)) {
                (Some((Helper::Pop, _)), Some((Helper::Pop, _)))
                    if pc == V::Result(0) && state == V::Result(1) =>
                {
                    Ok(Some(SelectorOp::PopJump))
                }
                _ => Err("does two things".into()),
            },
            _ => Err("does several things".into()),
        }
    }

    /// The two-way paths: conditional jumps on a popped value, the end of
    /// a `finally`, and a `catch` without a binding.
    fn name_forked(
        &self,
        tries: u32,
        records: u32,
        outcomes: &[Outcome],
    ) -> Result<Option<SelectorOp>, String> {
        let [a, b] = outcomes else {
            unreachable!("two outcomes")
        };
        let (taken, fallen) = match (a.tests.as_slice(), b.tests.as_slice()) {
            ([(test_a, true)], [(test_b, false)]) if test_a == test_b => (a, b),
            ([(test_a, false)], [(test_b, true)]) if test_a == test_b => (b, a),
            _ => return Err("forks on several tests".into()),
        };
        let test = &taken.tests[0].0;
        if records > 0 {
            let record = RECORD[records as usize - 1];
            if *test == V::Reg(record[2]) && fallen.local(PC) == &V::Reg(record[0]) {
                return self.name_finally_leave(tries, records, taken, fallen);
            }
            // A catch without a binding: on a handled record, drop the thrown
            // value under it (`TJ(1)`).
            if *test == V::Reg(record[2]) {
                let popped = |outcome: &Outcome| {
                    *outcome.local(RECORD_DEPTH) == V::Const(records as i64 - 1)
                        && *outcome.local(PC) == V::Pc(1)
                        && *outcome.local(STATE) == V::State
                };
                if popped(taken) && popped(fallen) && fallen.effects.is_empty() {
                    if let [Effect::Call { function, args }] = taken.effects.as_slice() {
                        if self.helper(*function) == Some(Helper::DropUnder)
                            && args.as_slice() == [V::Context, V::Const(1)]
                        {
                            return Ok(Some(SelectorOp::CatchDrop));
                        }
                    }
                }
                return Err("tests a record some other way".into());
            }
        }
        // A conditional jump: `L` pops and tests the top of the stack.
        let V::Result(0) = test else {
            return Err("forks on other than a popped value".into());
        };
        for outcome in [taken, fallen] {
            let [Effect::Call { function, args }] = outcome.effects.as_slice() else {
                return Err("a conditional jump does more".into());
            };
            if self.helper(*function) != Some(Helper::Truth) || args.as_slice() != [V::Context] {
                return Err("a conditional jump tests other than a popped value".into());
            }
            for local in [TRY_DEPTH, RECORD_DEPTH, CALL_DEPTH] {
                if outcome.local(local) != &Self::initial(tries, records)[&local] {
                    return Err("a conditional jump changes the banks".into());
                }
            }
        }
        // One way jumps to its operands; the other falls through.
        let jump_and_fall = |jump: &Outcome, fall: &Outcome| -> Option<(Operand, Operand, u32)> {
            let V::Pc(length) = fall.local(PC) else {
                return None;
            };
            if *fall.local(STATE) != V::State {
                return None;
            }
            Some((
                operand(jump.local(PC)).ok()?,
                operand(jump.local(STATE)).ok()?,
                (*length - 1) as u32,
            ))
        };
        if let Some((pc, state, length)) = jump_and_fall(taken, fallen) {
            return Ok(Some(SelectorOp::JumpIf {
                negated: false,
                pc,
                state,
                length,
            }));
        }
        if let Some((pc, state, length)) = jump_and_fall(fallen, taken) {
            return Ok(Some(SelectorOp::JumpIf {
                negated: true,
                pc,
                state,
                length,
            }));
        }
        Err("a conditional jump that does not jump to its operands".into())
    }

    /// The end of a `finally`: pop the record; a handled one rethrows (into
    /// the open try by setting the flag, else by `TT`), another resumes at
    /// its saved pc and state.
    fn name_finally_leave(
        &self,
        tries: u32,
        records: u32,
        handled: &Outcome,
        resumed: &Outcome,
    ) -> Result<Option<SelectorOp>, String> {
        let record = RECORD[records as usize - 1];
        let popped =
            |outcome: &Outcome| *outcome.local(RECORD_DEPTH) == V::Const(records as i64 - 1);
        if !popped(handled) || !popped(resumed) {
            return Err("ends a finally without popping its record".into());
        }
        if resumed.end != End::Next
            || !resumed.effects.is_empty()
            || *resumed.local(PC) != V::Reg(record[0])
            || *resumed.local(STATE) != V::Reg(record[1])
        {
            return Err("ends a finally that resumes elsewhere".into());
        }
        let rethrows = if tries > 0 {
            handled.end == End::Checked && handled.effects == [Effect::SetFlag]
        } else {
            handled.end == End::Next
                && matches!(handled.effects.as_slice(), [Effect::Call { function, args }]
                    if self.helper(*function) == Some(Helper::Rethrow) && args.as_slice() == [V::Context])
        };
        if !rethrows {
            return Err("ends a finally whose handled record does not rethrow".into());
        }
        Ok(Some(SelectorOp::FinallyLeave))
    }

    /// The flag check after an operation call: with the flag set, it clears
    /// it, pops the innermost try frame (taking back the calls made inside
    /// it), goes to its handler and pushes a handled record. It must not
    /// touch the records otherwise: every frame enters with none (checked by
    /// [`Self::name`]'s try entry), so there are none to drop.
    fn check_unwind(&self) -> Result<(), String> {
        let after = self.check + 1;
        if self.ops.get(after) != Some(&Op::If) {
            return Err("the exception flag is not tested".into());
        }
        let start = after + 1;
        for tries in 1..=TRY_FRAMES {
            for records in 0..ERROR_RECORDS {
                let mut locals = Self::initial(tries, records);
                let frame = TRY_FRAME[tries as usize - 1];
                // Frames enter with no records.
                locals.insert(frame[2], V::Const(0));
                let outcomes = self.run(start, locals)?;
                let [outcome] = outcomes.as_slice() else {
                    return Err("the unwind forks".into());
                };
                let record = RECORD[records as usize];
                let expect = [
                    (TRY_DEPTH, V::Const(tries as i64 - 1)),
                    (PC, V::Reg(frame[0])),
                    (STATE, V::Reg(frame[1])),
                    (RECORD_DEPTH, V::Const(records as i64 + 1)),
                    (record[2], V::Const(1)),
                    (
                        CALL_DEPTH,
                        V::Bin(
                            "-",
                            Box::new(V::Reg(CALL_DEPTH)),
                            Box::new(V::Reg(frame[3])),
                        ),
                    ),
                ];
                if outcome.end != End::Next
                    || outcome.effects != [Effect::ClearFlag]
                    || expect
                        .iter()
                        .any(|(local, value)| outcome.local(*local) != value)
                {
                    return Err(format!(
                        "the unwind with {tries} try frames and {records} records is not a catch: {outcome:?}"
                    ));
                }
            }
        }
        Ok(())
    }
}

/// `value + change` with constants folded.
fn plus(value: V, change: i64) -> V {
    match value {
        V::Const(k) => V::Const(k + change),
        V::Pc(k) => V::Pc(k + change),
        V::Plus(inner, k) if k + change == 0 => *inner,
        V::Plus(inner, k) => V::Plus(inner, k + change),
        other if change == 0 => other,
        other => V::Plus(Box::new(other), change),
    }
}

fn arithmetic(op: &Op, left: V, right: V) -> Result<V, String> {
    Ok(match (op, left, right) {
        (Op::Add, V::Const(a), V::Const(b)) => V::Const(a + b),
        (Op::Sub, V::Const(a), V::Const(b)) => V::Const(a - b),
        (Op::Mul, V::Const(a), V::Const(b)) => V::Const(a * b),
        (Op::Shl, V::Const(a), V::Const(b)) => V::Const(a << b),
        (Op::Ne, V::Const(a), V::Const(b)) => V::Const((a != b) as i64),
        (Op::Add, value, V::Const(a)) | (Op::Add, V::Const(a), value) => plus(value, a),
        (Op::Sub, value, V::Const(a)) => plus(value, -a),
        (Op::And, V::Wide(offset), V::Const(0xFF_FFFF)) => V::Load(offset, 3),
        (Op::Shl, value, V::Const(k)) => V::Shl(Box::new(value), k),
        (Op::Sub, left, right) => V::Bin("-", Box::new(left), Box::new(right)),
        (Op::Ne, left, right) => V::Bin("!=", Box::new(left), Box::new(right)),
        (op, left, right) => {
            return Err(format!("{op:?} of {left:?} and {right:?}"));
        }
    })
}

fn truth(value: &V) -> Option<bool> {
    match value {
        V::Const(k) => Some(*k != 0),
        _ => None,
    }
}

/// The pc after the instruction whose last operand is `operand`.
fn past(operand: Operand) -> i64 {
    1 + operand.offset as i64 + operand.width as i64
}

/// An operand: a load after the opcode.
fn operand(value: &V) -> Result<Operand, String> {
    match value {
        V::Load(offset, width) if *offset >= 1 => Ok(Operand {
            offset: offset - 1,
            width: *width,
        }),
        other => Err(format!("{other:?} is not an operand")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interpreter_fixture() -> (Vec<u8>, BTreeMap<String, Helper>) {
        let source = include_str!("../../../tests/fixtures/kernels/webassembly/kernel.raw.js");
        let view =
            crate::transforms::deob::pipeline::wasm_proof_report(source).expect("proof view");
        let table = view.proven_operation_tables[0].clone();
        let admitted = crate::dispatch::prefix::wasm::admit(&view, &table).expect("interpreter is admitted");
        (admitted.interpreter, admitted.roles)
    }

    fn at(offset: u32, width: u8) -> Operand {
        Operand { offset, width }
    }

    #[test]
    fn the_interpreter_lifts_every_selector() {
        let (bytes, roles) = interpreter_fixture();
        let lifted = lift(&bytes, &roles).expect("interpreter lifts");
        assert_eq!(
            (lifted.run_export.as_str(), lifted.flag_export.as_str()),
            ("W", "Te")
        );
        assert_eq!(
            (lifted.base, lifted.stride, lifted.memory_bytes),
            (170_845, 468, 196_608)
        );
        assert_eq!(lifted.selectors.len(), 234);
        // Every table selector calls the operation of its number.
        for selector in 0..201u8 {
            assert!(
                matches!(lifted.op(selector), SelectorOp::Operation { operation, .. } if *operation == selector as u32),
                "{selector}: {:?}",
                lifted.op(selector)
            );
        }
        assert_eq!(
            *lifted.op(21),
            SelectorOp::Operation {
                operation: 21,
                operands: vec![at(0, 2)],
                length: 2
            }
        );
        // A masked four-byte load is a three-byte operand.
        assert_eq!(
            *lifted.op(39),
            SelectorOp::Operation {
                operation: 39,
                operands: vec![at(0, 1), at(1, 1), at(2, 3), at(5, 3)],
                length: 8
            }
        );
        let expected = [
            (
                232,
                SelectorOp::ImageWrite {
                    address: at(0, 3),
                    value: at(3, 1),
                },
            ),
            (
                230,
                SelectorOp::Jump {
                    pc: at(0, 2),
                    state: at(2, 1),
                },
            ),
            (
                219,
                SelectorOp::JumpIf {
                    negated: true,
                    pc: at(0, 3),
                    state: at(3, 1),
                    length: 4,
                },
            ),
            (
                227,
                SelectorOp::JumpIf {
                    negated: false,
                    pc: at(0, 2),
                    state: at(2, 1),
                    length: 3,
                },
            ),
            (
                221,
                SelectorOp::Call {
                    pc: at(0, 3),
                    state: at(3, 1),
                    length: 4,
                },
            ),
            (202, SelectorOp::CallReturn),
            (
                229,
                SelectorOp::TryEnter {
                    pc: at(0, 2),
                    state: at(2, 1),
                    length: 3,
                },
            ),
            (204, SelectorOp::TryLeave),
            (211, SelectorOp::FinallyEnter),
            (216, SelectorOp::FinallyLeave),
            (226, SelectorOp::CatchDrop),
            (210, SelectorOp::CatchBind { slot: at(0, 1) }),
            (214, SelectorOp::PopJump),
            (207, SelectorOp::Return { status: 1 }),
            (220, SelectorOp::Return { status: 0 }),
            (233, SelectorOp::PushLiteral { kind: 6 }),
            (215, SelectorOp::PushNumber { operand: at(0, 1) }),
            (
                223,
                SelectorOp::PushTableNumber {
                    table: 169_485,
                    operand: at(0, 1),
                },
            ),
        ];
        for (selector, op) in expected {
            assert_eq!(*lifted.op(selector), op, "selector {selector}");
        }
        // Past the branch table, every byte traps.
        assert_eq!(*lifted.op(234), SelectorOp::Trap);
        assert_eq!(*lifted.op(255), SelectorOp::Trap);
    }

    #[test]
    fn a_truncated_module_or_unknown_helpers_are_refused() {
        let (bytes, roles) = interpreter_fixture();
        assert!(lift(&bytes[..bytes.len() / 2], &roles).is_err());
        // Without the helpers' roles no operation call can be named.
        assert!(lift(&bytes, &BTreeMap::new()).is_err());
    }

    #[test]
    fn a_path_that_writes_the_flag_otherwise_is_refused() {
        // `i32.const 1; global.set 0` in Te, the only flag setter outside the
        // run function, is not on a path; one inside it is refused unless it
        // is a `finally`'s rethrow.
        let mut machine_ops = vec![Op::Const(2), Op::GlobalSet(0)];
        machine_ops.push(Op::End);
        let structure = Structure::of(&machine_ops).unwrap();
        let module = Module {
            types: Vec::new(),
            imports: Vec::new(),
            defined_types: Vec::new(),
            memory_pages: None,
            exports: Vec::new(),
            bodies: Vec::new(),
        };
        let machine = Machine {
            ops: &machine_ops,
            structure: &structure,
            module: &module,
            helpers: &BTreeMap::new(),
            dispatch_loop: usize::MAX,
            check: usize::MAX,
        };
        let error = machine.run(0, Machine::initial(0, 0)).unwrap_err();
        assert!(error.contains("exception flag"), "{error}");
    }
}
