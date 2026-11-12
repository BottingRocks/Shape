use super::*;

/// Stable identity of a node in a [`TraceArena`].
pub type TraceNodeId = u32;

/// Stable identity of a statement in a [`TraceArena`].
pub type TraceStatementId = u32;

/// A value supplied by a decoder or used as a concrete symbolic value.
///
/// `External` is an intentionally opaque, valid symbolic input and is emitted
/// as a function parameter. `Unknown` is different: it means the recipe or
/// supplied input is not safe to execute and causes a partial trace.
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolicValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    /// A stable identity for an object-like value. It is never coerced to a
    /// number by the tracer.
    Object {
        identity: u64,
    },
    External {
        name: String,
    },
    /// A computation whose syntax is retained in the trace but whose
    /// concrete result is not proven.  This is not an unsupported value.
    Computed {
        node: TraceNodeId,
    },
    Unknown {
        reason: String,
    },
}

/// One explicitly decoded instruction. The handler key is dispatch metadata;
/// it is never interpreted as a semantic opcode by this module.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedInstruction {
    pub handler_index: u32,
    pub operands: Vec<SymbolicValue>,
}

impl DecodedInstruction {
    #[must_use]
    pub fn new(handler_index: u32, operands: Vec<SymbolicValue>) -> Self {
        Self {
            handler_index,
            operands,
        }
    }
}

/// Preferred public name for one decoded VM instruction occurrence.
///
/// A `VMInstruction` is the runtime input to tracing/execution: the selected
/// handler key plus already-decoded operand values.  It is distinct from
/// [`HandlerIR`](crate::HandlerIR), which is the static recovered program for
/// that handler and is reused across instruction occurrences.  The dispatch PC
/// and dispatch provenance remain in the dispatch record; this alias adds no
/// such fields.  This is an alias of [`DecodedInstruction`], so existing
/// callers, `Debug`, and type identity are unchanged.
pub type VMInstruction = DecodedInstruction;

/// Limits that make tracing deterministic on malformed or adversarial input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceLimits {
    pub max_steps: usize,
    pub max_nodes: usize,
    pub max_stack_length: usize,
    pub max_memory_slots: usize,
}

/// Shared status for a live debug writer.
///
/// A writer error never panics and never changes symbolic execution. The sink
/// disables itself after the first failed write/flush and records the error
/// here for callers that need to surface it. Keep a clone of the status before
/// passing the options into an execution API:
///
/// ```
/// # use shape::TraceDebugOptions;
/// let options = TraceDebugOptions::stderr();
/// let status = options.status();
/// // pass `options` to an execute/trace API
/// assert!(!status.has_error());
/// ```
#[derive(Clone, Default)]
pub struct TraceDebugStatus {
    pub(super) error: Arc<Mutex<Option<String>>>,
}

impl TraceDebugStatus {
    /// Return the first writer error, if the live sink failed.
    #[must_use]
    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|error| error.clone())
    }

    /// Whether the live writer has failed and been disabled.
    #[must_use]
    pub fn has_error(&self) -> bool {
        self.error().is_some()
    }
}

impl fmt::Debug for TraceDebugStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TraceDebugStatus")
            .field("error", &self.error())
            .finish()
    }
}

/// Opt-in live per-operation debug output.
///
/// The writer is owned by the execution, so no global mutable writer or
/// replayed/collected log is involved. `stderr()` is convenient for command
/// line debugging; `writer` accepts a caller-owned writer value such as a file
/// or a test buffer. Output is flushed before each operation and after each
/// committed operation. Existing APIs use [`Self::disabled`] and remain
/// silent.
pub struct TraceDebugOptions {
    pub(super) writer: Option<Box<dyn Write + Send + Sync>>,
    pub(super) status: TraceDebugStatus,
}

impl Default for TraceDebugOptions {
    fn default() -> Self {
        Self::disabled()
    }
}

impl TraceDebugOptions {
    /// Disable live output. This is the default and avoids debug formatting.
    #[must_use]
    pub fn disabled() -> Self {
        Self {
            writer: None,
            status: TraceDebugStatus::default(),
        }
    }

    /// Write live output to stderr, flushing each operation.
    #[must_use]
    pub fn stderr() -> Self {
        Self::writer(io::stderr())
    }

    /// Write live output to an owned caller-supplied writer.
    #[must_use]
    pub fn writer<W>(writer: W) -> Self
    where
        W: Write + Send + Sync + 'static,
    {
        Self {
            writer: Some(Box::new(writer)),
            status: TraceDebugStatus::default(),
        }
    }

    /// Observe write failures after the options have been moved into a run.
    #[must_use]
    pub fn status(&self) -> TraceDebugStatus {
        self.status.clone()
    }

    /// Whether this configuration will perform live output.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.writer.is_some()
    }
}

pub(super) struct DebugSink {
    pub(super) writer: Option<Box<dyn Write + Send + Sync>>,
    pub(super) status: TraceDebugStatus,
    pub(super) operation_count: usize,
    pub(super) committed_count: usize,
    pub(super) pending: Option<DebugOperation>,
    pub(super) finished: bool,
}

pub(super) struct DebugOperation {
    pub(super) pc: Option<usize>,
    pub(super) state: Option<usize>,
    pub(super) opcode: Option<u8>,
    pub(super) handler_key: u32,
    pub(super) nodes_before: usize,
    pub(super) events_before: usize,
}

impl DebugSink {
    pub(super) fn disabled() -> Self {
        Self {
            writer: None,
            status: TraceDebugStatus::default(),
            operation_count: 0,
            committed_count: 0,
            pending: None,
            finished: false,
        }
    }

    pub(super) fn from_options(options: TraceDebugOptions) -> Self {
        Self {
            writer: options.writer,
            status: options.status,
            operation_count: 0,
            committed_count: 0,
            pending: None,
            finished: false,
        }
    }

    pub(super) fn enabled(&self) -> bool {
        self.writer.is_some() && !self.finished
    }

    pub(super) fn emit(&mut self, text: String) {
        let Some(writer) = self.writer.as_mut() else {
            return;
        };
        if let Err(error) = writer
            .write_all(text.as_bytes())
            .and_then(|()| writer.flush())
        {
            if let Ok(mut status) = self.status.error.lock() {
                if status.is_none() {
                    *status = Some(error.to_string());
                }
            }
            // A diagnostic writer must not turn a valid trace into a panic or
            // a different execution. Disable it explicitly after the first
            // error; callers can inspect TraceDebugStatus for the reason.
            self.writer = None;
            self.pending = None;
        }
    }

    pub(super) fn before(
        &mut self,
        pc: Option<usize>,
        state: Option<usize>,
        opcode: Option<u8>,
        handler_key: u32,
        nodes_before: usize,
        events_before: usize,
    ) {
        if !self.enabled() {
            return;
        }
        self.operation_count += 1;
        let next = match (state, pc) {
            (Some(state), Some(pc)) => format!("{state}_{pc}"),
            _ => "unavailable".into(),
        };
        let mut text = format!("NEXT:{next}\nOPKEY:{handler_key}\n");
        if let Some(opcode) = opcode {
            text.push_str(&format!("OPCODE:{opcode}\n"));
        }
        self.emit(std::mem::take(&mut text));
        if self.enabled() {
            self.pending = Some(DebugOperation {
                pc,
                state,
                opcode,
                handler_key,
                nodes_before,
                events_before,
            });
        }
    }

    pub(super) fn after(&mut self, text: String) {
        if !self.enabled() {
            return;
        }
        self.emit(text);
        if self.enabled() {
            self.committed_count += 1;
            self.pending = None;
        }
    }

    pub(super) fn stop(&mut self, code: impl fmt::Display, message: &str) {
        if !self.enabled() || self.finished {
            return;
        }
        self.emit(format!(
            "STOP:{code} steps={} committed={} message={message}\n",
            self.operation_count, self.committed_count
        ));
        self.pending = None;
        self.finished = true;
    }

    pub(super) fn finish(&mut self, completion: &TraceCompletion) {
        if !self.enabled() || self.finished {
            return;
        }
        self.emit(format!(
            "STOP:{completion:?} steps={} committed={}\n",
            self.operation_count, self.committed_count
        ));
        self.finished = true;
    }
}

impl Default for TraceLimits {
    fn default() -> Self {
        Self {
            max_steps: 10_000,
            max_nodes: 50_000,
            max_stack_length: 4_096,
            max_memory_slots: 4_096,
        }
    }
}

/// Mutable symbolic VM state. A stack hole is distinct from a value of
/// `undefined`; both read as JavaScript `undefined`, but only the latter is a
/// deliberate value write. Memory initialization is likewise distinct from a
/// missing slot.
#[derive(Debug, Clone, Default)]
pub struct TraceContext {
    pub(super) stack: Vec<StackCell>,
    /// Holes a join put into this path's stack, from `stack_pad_start`, so
    /// that what one side pushed above the part both sides share lines up
    /// from the top with the other side's (see `TraceSession::pad_stack`).
    /// A step that reaches them, or what lies under them, reads what only
    /// one side has there: the join is undone.
    pub(super) stack_pad: usize,
    pub(super) stack_pad_start: usize,
    /// How many joins put the holes there, and whether the one that did
    /// lined up a shorter side that popped values the other keeps: then a
    /// step that reads into them reads what only the longer side holds.
    pub(super) stack_pad_joins: usize,
    pub(super) stack_pad_deficit: bool,
    /// The join that put them there, by tag.
    pub(super) stack_pad_join: Option<usize>,
    /// Shared until written: checkpoints and forked paths copy only the
    /// handle, and the first write copies the slots.
    pub(super) memory: Arc<Slots>,
    /// The VM call stack (`context.callFrameStack`). A VM value: frames are
    /// concrete return coordinates.
    pub(super) call_frames: Vec<CallFrame>,
    /// The `this` the running VM function was called with
    /// (`context.receiverValue`). `None` when the caller did not say; a
    /// handler that reads it then stops.
    pub(super) receiver: Option<SymbolicValue>,
    /// Active `try` blocks (`context.tryFrameStack`), innermost last.
    pub(super) try_frames: Vec<TryFrame>,
    /// `try` exits run with no `try` active: `tryFrameStack.pop()` of an
    /// empty stack does nothing, and the WASM interpreter's try depth goes
    /// below zero. A `try` entered or a throw after one is not traced.
    pub(super) try_underflow: u32,
    /// Error records of the `catch`/`finally` bodies being run
    /// (`context.errorStack`).
    pub(super) error_records: Vec<ErrorRecord>,
    /// Pending exceptions (`context.thrownValueStack`), program values.
    pub(super) thrown: Vec<StoredValue>,
    /// Cells a `catch` reads as variables: inside the `try` body every write
    /// to one also assigns its variable.
    pub(super) try_variables: Arc<BTreeMap<TraceCell, String>>,
    /// Cells the innermost `try` body changed: written with a value other
    /// than the one they held when the `try` was entered.
    pub(super) try_writes: Arc<BTreeSet<TraceCell>>,
    /// The state the innermost `try` was entered with.
    pub(super) try_entry: Option<Arc<TraceContext>>,
    /// The enclosing `try`s' variables, changed cells and entry states,
    /// outermost first; restored when the innermost one ends.
    pub(super) try_outer: Arc<Vec<TryScope>>,
    /// What the branches this path took say about their tested values: a
    /// [`predicate::test_key`] and whether that value is truthy here.
    pub(super) branch_facts: Arc<BTreeMap<String, bool>>,
    /// What the type tests this path took say about their tested values:
    /// the types each value ([`predicate::type_test`]) can still have here.
    pub(super) type_facts: Arc<BTreeMap<String, u16>>,
    /// Variables a join made from a concrete value on at least one side
    /// (a jump one side set up, the other not), with the join they came
    /// from. A VM value read from one (a jump target, a
    /// state, an index) cannot be traced merged; see
    /// [`TraceSession::take_split`].
    pub(super) merged_concrete: Arc<BTreeMap<String, MergedConcrete>>,
    /// Loop variables, with the cell each carries between iterations.
    pub(super) loop_variables: Arc<BTreeMap<String, TraceCell>>,
    /// Variables known to hold one of a few concrete values: a loop
    /// variable whose every iteration sets a concrete value, or a merge of
    /// concrete values.
    pub(super) value_sets: Arc<BTreeMap<String, Arc<[SymbolicValue]>>>,
    /// `try` variables whose every write in the body is a VM address. With
    /// an entry value that is no concrete one, a VM jump through one goes to
    /// one of those addresses, as one to the entry's program value would
    /// leave the code the VM runs; the state of that jump is then one the
    /// body wrote too, whatever its variable held at entry. Read only by VM
    /// jumps.
    pub(super) jump_sets: Arc<BTreeMap<String, Arc<[SymbolicValue]>>>,
    /// What this path knows about such variables: forked on a test of one.
    pub(super) assumed: Arc<BTreeMap<String, Assumption>>,
    /// The path came through a `catch`: the VM stack below what the `try`
    /// was entered with is the throw's, which the tracer does not know.
    pub(super) from_catch: bool,
    /// The `try` statement whose catch the path came through (the earliest
    /// of a join's), while `from_catch`.
    pub(super) catch_try: Option<TraceStatementId>,
    /// A `try` body that left its block: that block. What it binds outside
    /// is not in scope inside, where a join with the catch assigns its
    /// values.
    pub(super) left_try: Option<BlockTarget>,
    /// The forks inside a `finally` this path was in when the `finally`
    /// ended: the path went on after the `try`, out of their arms, though
    /// their other side ended (see `TraceSession::continue_alone`).
    pub(super) finally_left: Arc<BTreeSet<TraceStatementId>>,
    /// In a `finally` traced once from the end of its `try` body and from a
    /// throw: what each of the two put in a shared cell, where they put
    /// different values, until the `finally` writes the cell (see
    /// `Executor::finally_reads`).
    pub(super) finally_concrete: Arc<BTreeMap<u32, (TraceNodeId, TraceNodeId)>>,
    /// The memory each `try`'s catch path started with, by statement (see
    /// `TraceSession::finally_writes`).
    pub(super) catch_memory: Arc<BTreeMap<TraceStatementId, Arc<Slots>>>,
    /// What this path last wrote to each shared cell (see
    /// [`TraceSession::open_cells`]), known until a statement that may run
    /// other code, which may write the cell too. Only decisions use it: the
    /// emitted code reads the cell.
    pub(super) cell_facts: Arc<BTreeMap<u32, SymbolicValue>>,
    /// The node this path last wrote to each shared cell, known as long as
    /// [`Self::cell_facts`] would be: a read of the cell is that value to
    /// decisions (its type, say), never to the emitted code.
    pub(super) cell_nodes: Arc<BTreeMap<u32, TraceNodeId>>,
    /// The node this path last wrote to each shared cell (or first read
    /// there), kept past code that may run: what the cell concretely holds
    /// unless a closure wrote it. Only tests shaped like an opaque predicate
    /// are decided on it (see [`predicate::opaque_shape`]).
    pub(super) cell_concrete: Arc<BTreeMap<u32, TraceNodeId>>,
    /// When this path last wrote each shared cell, by the executor's write
    /// serial: a loop's iteration wrote the cells written after its header.
    pub(super) cell_writes: Arc<BTreeMap<u32, u64>>,
    /// The VM position (a jump target or dispatch state) this path last wrote
    /// to each shared cell, kept past code that may run: a closure that
    /// wrote another value there would send the VM's own jump into a
    /// program value, which code that runs does not do. Read only by VM
    /// jumps (see [`Executor::vm_position`]), never by tests or code.
    pub(super) cell_positions: Arc<BTreeMap<u32, usize>>,
    /// Shared cells this path last wrote a value to that is no VM position
    /// (`undefined` as the cell is made, say), kept as `cell_positions` is:
    /// a VM jump through one goes to a program value, as no other function
    /// writes the VM positions this function's code jumps to.
    pub(super) cell_values: Arc<BTreeSet<u32>>,
    /// What a value dispatch on a shared cell's read told this path of that
    /// read, for the instruction it runs again: the read gets a new name.
    pub(super) cell_assumed: Arc<BTreeMap<CellRead, Assumption>>,
    /// Shared cells some other VM function holds on this path: those the
    /// caller passed, and those a closure created here captured. Only these
    /// may change behind the path's back.
    pub(super) escaped_cells: Arc<BTreeSet<u32>>,
    /// Objects this path made that no other code holds yet (no call, store,
    /// assignment or return has had one), by the node that holds each: the
    /// properties it wrote with a known value. A read of one is that value.
    pub(super) made_objects: Arc<BTreeMap<TraceNodeId, BTreeMap<String, SymbolicValue>>>,
    /// Inside a VM subroutine traced as a function: its parameters' names.
    pub(super) params: Arc<BTreeSet<String>>,
    /// Shared cells the function writes one of its parameters to, with the
    /// parameter: what that puts in the cell depends on each call.
    pub(super) param_writes: Arc<Vec<(u32, String)>>,
    /// Inside a VM subroutine traced as a function: which, and the
    /// parameter each caller cell read so far is read as.
    pub(super) subroutine: Option<Arc<OpenSubroutine>>,
    pub(super) placeholders: Arc<BTreeMap<TraceCell, String>>,
}

/// See [`Executor::scopes`].
pub(super) struct Scopes {
    pub(super) parent: Vec<Option<BlockTarget>>,
    pub(super) declared: Declared,
}

/// The statement declaring each name: a generated name by its serial (see
/// [`Definitions`]), any other in a map by name.
#[derive(Default)]
pub(super) struct Declared {
    by_serial: Vec<Option<TraceStatementId>>,
    other: HashMap<String, TraceStatementId>,
}

impl Declared {
    pub(super) fn get(&self, name: &str) -> Option<TraceStatementId> {
        match generated_serial(name) {
            Some(serial) => self.by_serial.get(serial).copied().flatten(),
            None => self.other.get(name).copied(),
        }
    }

    pub(super) fn insert(&mut self, name: &str, statement: TraceStatementId) {
        match generated_serial(name) {
            Some(serial) => {
                if serial >= self.by_serial.len() {
                    self.by_serial.resize(serial + 1, None);
                }
                self.by_serial[serial] = Some(statement);
            }
            None => {
                self.other.insert(name.to_owned(), statement);
            }
        }
    }
}

/// `name`'s serial, when `name` is the one `trace_value_{serial}` gives.
fn generated_serial(name: &str) -> Option<usize> {
    let digits = name.strip_prefix("trace_value_")?;
    let canonical = digits.bytes().all(|b| b.is_ascii_digit())
        && !digits.is_empty()
        && (digits.len() == 1 || !digits.starts_with('0'));
    canonical.then(|| digits.parse().ok()).flatten()
}

/// A map keyed by trace node. Node numbers are dense, so it is a vector
/// indexed by them: the lookups the executor makes on every step cost no
/// search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeMap<V> {
    slots: Vec<Option<V>>,
}

impl<V> Default for NodeMap<V> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<V> NodeMap<V> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn get(&self, node: &TraceNodeId) -> Option<&V> {
        self.slots.get(*node as usize)?.as_ref()
    }

    #[must_use]
    pub fn contains_key(&self, node: &TraceNodeId) -> bool {
        self.get(node).is_some()
    }

    pub fn insert(&mut self, node: TraceNodeId, value: V) -> Option<V> {
        let index = node as usize;
        if index >= self.slots.len() {
            self.slots.resize_with(index + 1, || None);
        }
        self.slots[index].replace(value)
    }

    /// One past the highest node it can hold an entry for: no more entries
    /// than that, so a chain of entries without a cycle is no longer.
    #[must_use]
    pub fn span(&self) -> usize {
        self.slots.len()
    }

    /// Forget every node from `node` on (a rollback past it).
    pub fn truncate(&mut self, node: TraceNodeId) {
        self.slots.truncate(node as usize);
    }
}

/// Each generated variable's definitions (statement, value), in the order
/// made. A generated name is `trace_value_` and its serial, so it is held in
/// a vector indexed by the serial; any other name that reads as one (a
/// leading zero, say) in a map by name.
#[derive(Debug, Clone, Default)]
pub(super) struct Definitions {
    by_serial: Vec<Vec<(TraceStatementId, TraceNodeId)>>,
    other: BTreeMap<String, Vec<(TraceStatementId, TraceNodeId)>>,
}

impl Definitions {
    pub(super) fn get(&self, name: &str) -> Option<&Vec<(TraceStatementId, TraceNodeId)>> {
        match generated_serial(name) {
            Some(serial) => self
                .by_serial
                .get(serial)
                .filter(|defined| !defined.is_empty()),
            None => self.other.get(name),
        }
    }

    pub(super) fn contains_key(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub(super) fn push(&mut self, name: &str, statement: TraceStatementId, value: TraceNodeId) {
        let defined = match generated_serial(name) {
            Some(serial) => {
                if serial >= self.by_serial.len() {
                    self.by_serial.resize_with(serial + 1, Vec::new);
                }
                &mut self.by_serial[serial]
            }
            None => self.other.entry(name.to_owned()).or_default(),
        };
        defined.push((statement, value));
    }

    /// Forget the definitions made by statement `statements` on.
    pub(super) fn truncate(&mut self, statements: TraceStatementId) {
        for defined in &mut self.by_serial {
            defined.retain(|(statement, _)| *statement < statements);
        }
        self.other.retain(|_, defined| {
            defined.retain(|(statement, _)| *statement < statements);
            !defined.is_empty()
        });
    }
}

impl std::ops::Index<&String> for Definitions {
    type Output = Vec<(TraceStatementId, TraceNodeId)>;

    fn index(&self, name: &String) -> &Self::Output {
        self.get(name).expect("a defined name")
    }
}

impl<V> FromIterator<(TraceNodeId, V)> for NodeMap<V> {
    fn from_iter<I: IntoIterator<Item = (TraceNodeId, V)>>(entries: I) -> Self {
        let mut map = Self::new();
        for (node, value) in entries {
            map.insert(node, value);
        }
        map
    }
}

/// See [`TraceSession::stop_marks`].
#[derive(Debug, Clone)]
pub struct StopMarks {
    pub(super) diagnostics: usize,
    pub(super) completion: TraceCompletion,
}

/// What a path knows about a variable with a value set.
#[derive(Debug, Clone)]
pub(super) enum Assumption {
    Is(SymbolicValue),
    Not(Vec<SymbolicValue>),
}

/// The most values a variable's value set may have and still be dispatched
/// on: more is not a jump table.
pub(super) const MAX_VALUE_SET: usize = 64;

/// A VM jump target or state that is a variable with a known value set: the
/// driver branches on each of `candidates`. See
/// [`TraceSession::take_dispatch`].
#[derive(Debug, Clone, PartialEq)]
pub struct ValueDispatch {
    pub name: String,
    pub candidates: Vec<SymbolicValue>,
    /// The shared cell read `name` was bound to, by the instruction that
    /// needed its value: run again, the read gets a new name, so the test
    /// is on the cell and what it tells holds for that read.
    pub cell: Option<CellRead>,
}

/// One read of a shared cell by an instruction: the cell's slot and how
/// many reads of it the instruction made before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellRead {
    pub slot: u32,
    pub read: u32,
}

/// What a shared memory cell can hold: the VM addresses (non-negative
/// integers) functions holding it write, and whether any of them writes a
/// JavaScript value besides. A compiled program never makes a bytecode
/// address a JavaScript value: an address in a cell is one some function's
/// own code wrote there as a jump setup.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellContent {
    pub addresses: BTreeSet<usize>,
    pub data: bool,
}

/// What a walk does with a shared cell's contents when it reads the cell
/// (see `Executor::read_cell`, the only reader of the contents): two
/// contents with the same effect trace every read alike.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum CellReadEffect {
    Nothing,
    /// A JavaScript value, no address: the read is program data.
    Data,
    /// One of a few addresses (a value set), and whether data too.
    Addresses(BTreeSet<usize>, bool),
}

/// The shared cells a walk read, and of those the cells whose reads it
/// asked were program data (`data`) and whose reads it asked for a value
/// set (`sets`): all it used of what it was told they hold.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellReads {
    pub cells: BTreeSet<u32>,
    pub data: BTreeSet<u32>,
    pub sets: BTreeSet<u32>,
}

impl CellReads {
    /// Whether a walk that read these cells traces the same told `new` of
    /// the cells' contents as told `old`: where it asked whether a read is
    /// data, the answer is the same, and where it asked a read's value set
    /// (and whether the cell holds data too), so is that. Two walks ask
    /// alike until an answer differs, so one that never asked used nothing
    /// else of the contents.
    #[must_use]
    pub fn same(&self, old: &BTreeMap<u32, CellContent>, new: &BTreeMap<u32, CellContent>) -> bool {
        let data = |effect: &CellReadEffect| matches!(effect, CellReadEffect::Data);
        let set = |effect: &CellReadEffect| match effect {
            CellReadEffect::Addresses(addresses, data) => Some((addresses.clone(), *data)),
            _ => None,
        };
        self.cells.iter().all(|slot| {
            let (old, new) = (
                CellReadEffect::of(old.get(slot)),
                CellReadEffect::of(new.get(slot)),
            );
            (!self.data.contains(slot) || data(&old) == data(&new))
                && (!self.sets.contains(slot) || set(&old) == set(&new))
        })
    }
}

impl CellReadEffect {
    pub(super) fn of(content: Option<&CellContent>) -> Self {
        match content {
            None => Self::Nothing,
            Some(content) if content.addresses.is_empty() => {
                if content.data {
                    Self::Data
                } else {
                    Self::Nothing
                }
            }
            Some(content) if content.addresses.len() <= MAX_VALUE_SET => {
                Self::Addresses(content.addresses.clone(), content.data)
            }
            Some(_) => Self::Nothing,
        }
    }
}

impl CellContent {
    /// Both: what either can hold.
    pub fn merge(&mut self, other: &Self) {
        self.addresses.extend(other.addresses.iter().copied());
        self.data |= other.data;
    }
}

/// What one enclosing `try` tracks while a nested one runs.
#[derive(Debug, Clone, Default)]
pub(super) struct TryScope {
    pub(super) variables: Arc<BTreeMap<TraceCell, String>>,
    pub(super) writes: Arc<BTreeSet<TraceCell>>,
    pub(super) entry: Option<Arc<TraceContext>>,
}

/// Where a variable that merges a concrete value came from: the caller's
/// tag for the join and the cell it merged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergedConcrete {
    pub join: usize,
    pub cell: TraceCell,
}

/// One active `try`: where its catch or finally handler starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TryFrame {
    pub handler_byte_offset: usize,
    pub handler_dispatch_state: usize,
}

/// One error record: whether an exception is pending, and where a normally
/// entered `finally` resumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ErrorRecord {
    pub handled: bool,
    pub saved: Option<CallFrame>,
    /// The `try` whose `finally` the path traces with this record: popping
    /// it ends the `finally` (see `TraceSession::join_finally`).
    pub finally: Option<TraceStatementId>,
    /// The `try` whose catch path this record started, the exception
    /// pending: popping it ends that handler (see
    /// `TraceSession::finally_writes`).
    pub thrown_in: Option<TraceStatementId>,
}

/// One VM call frame `{returnByteOffset, returnDispatchState}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CallFrame {
    pub return_byte_offset: usize,
    pub return_dispatch_state: usize,
}

/// Where the VM stands while one handler runs: the PC after the handler's
/// operands and the dispatch state already selected by `dispatch()`. A call
/// frame pushed by the handler returns here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmCursor {
    pub byte_offset: usize,
    pub dispatch_state: usize,
}

/// The PC and dispatch state a handler assigned, if any. The driver continues
/// at these instead of the ordinary fall-through.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ControlTransfer {
    pub byte_offset: Option<usize>,
    pub dispatch_state: Option<usize>,
}

/// `Param` (in both kinds of cell): inside a subroutine traced as a
/// function, the caller's value of the cell, which the function reads as a
/// parameter. It is named when first read (see
/// [`Executor::param_value`]); most caller cells never are.
#[derive(Debug, Clone)]
pub(super) enum StackCell {
    Hole,
    Value(StoredValue),
    Param,
}

#[derive(Debug, Clone)]
pub(super) enum MemoryCell {
    Initialized,
    Value(StoredValue),
    Param,
}

/// The subroutine a path is traced inside, for naming its parameters.
#[derive(Debug)]
pub(super) struct OpenSubroutine {
    pub(super) name: String,
    /// The stack length at the call.
    pub(super) entry_stack: usize,
    pub(super) provenance: TraceProvenance,
}

/// Memory cells by slot, in slot order. One allocation: a path's first write
/// after a checkpoint or a fork copies it, and a dropped checkpoint frees
/// it, in one piece.
#[derive(Debug, Clone, Default)]
pub(super) struct Slots(pub(super) Vec<(u32, MemoryCell)>);

impl Slots {
    pub(super) fn find(&self, slot: u32) -> Result<usize, usize> {
        self.0.binary_search_by_key(&slot, |(slot, _)| *slot)
    }

    pub(super) fn get(&self, slot: &u32) -> Option<&MemoryCell> {
        self.find(*slot).ok().map(|index| &self.0[index].1)
    }

    pub(super) fn get_mut(&mut self, slot: &u32) -> Option<&mut MemoryCell> {
        self.find(*slot).ok().map(|index| &mut self.0[index].1)
    }

    pub(super) fn contains_key(&self, slot: &u32) -> bool {
        self.find(*slot).is_ok()
    }

    pub(super) fn insert(&mut self, slot: u32, cell: MemoryCell) -> Option<MemoryCell> {
        match self.find(slot) {
            Ok(index) => Some(std::mem::replace(&mut self.0[index].1, cell)),
            Err(index) => {
                self.0.insert(index, (slot, cell));
                None
            }
        }
    }

    pub(super) fn len(&self) -> usize {
        self.0.len()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (&u32, &MemoryCell)> {
        self.0.iter().map(|(slot, cell)| (slot, cell))
    }

    pub(super) fn keys(&self) -> impl Iterator<Item = &u32> {
        self.0.iter().map(|(slot, _)| slot)
    }

    pub(super) fn values(&self) -> impl Iterator<Item = &MemoryCell> {
        self.0.iter().map(|(_, cell)| cell)
    }
}

impl std::ops::Index<&u32> for Slots {
    type Output = MemoryCell;

    fn index(&self, slot: &u32) -> &MemoryCell {
        self.get(slot).expect("a memory slot")
    }
}

#[derive(Debug, Clone)]
pub(super) struct StoredValue {
    pub(super) value: SymbolicValue,
    pub(super) fact: Option<SymbolicValue>,
    pub(super) node: Option<TraceNodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceContextError {
    pub what: &'static str,
    pub limit: usize,
}

impl fmt::Display for TraceContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} exceeds configured limit {}", self.what, self.limit)
    }
}

impl std::error::Error for TraceContextError {}

impl TraceContext {
    /// Whether stack position `index`, or a stack `index` values long,
    /// reaches the holes a join put into the stack or what lies under them
    /// (see `stack_pad`): the join that put them, if so.
    pub(super) fn padded_below(&self, index: usize) -> Option<usize> {
        (self.stack_pad > 0 && index < self.stack_pad_start + self.stack_pad)
            .then_some(self.stack_pad_join)
            .flatten()
    }

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an existing or new stack value. Missing positions become holes.
    pub fn set_stack_value(
        &mut self,
        index: usize,
        value: SymbolicValue,
    ) -> Result<(), TraceContextError> {
        self.set_stack_value_bounded(index, value, TraceLimits::default().max_stack_length)
    }

    /// Checked context initialization for callers with a task-specific limit.
    pub fn set_stack_value_bounded(
        &mut self,
        index: usize,
        value: SymbolicValue,
        max_stack_length: usize,
    ) -> Result<(), TraceContextError> {
        reject_external_computed(&value)?;
        let length = index.checked_add(1).ok_or(TraceContextError {
            what: "stack index",
            limit: max_stack_length,
        })?;
        if length > max_stack_length {
            return Err(TraceContextError {
                what: "stack length",
                limit: max_stack_length,
            });
        }
        if self.stack.len() < length {
            self.stack.resize(length, StackCell::Hole);
        }
        self.stack[index] = StackCell::Value(stored_value(value));
        Ok(())
    }

    /// Grow the stack with holes without performing a value write.
    pub fn set_stack_length(&mut self, length: usize) -> Result<(), TraceContextError> {
        self.set_stack_length_bounded(length, TraceLimits::default().max_stack_length)
    }

    pub fn set_stack_length_bounded(
        &mut self,
        length: usize,
        max_stack_length: usize,
    ) -> Result<(), TraceContextError> {
        if length > max_stack_length {
            return Err(TraceContextError {
                what: "stack length",
                limit: max_stack_length,
            });
        }
        self.stack.resize(length, StackCell::Hole);
        Ok(())
    }

    pub fn set_memory_value(
        &mut self,
        slot: u32,
        value: SymbolicValue,
    ) -> Result<(), TraceContextError> {
        self.set_memory_value_bounded(slot, value, TraceLimits::default().max_memory_slots)
    }

    pub fn set_memory_value_bounded(
        &mut self,
        slot: u32,
        value: SymbolicValue,
        max_memory_slots: usize,
    ) -> Result<(), TraceContextError> {
        reject_external_computed(&value)?;
        if !self.memory.contains_key(&slot) && self.memory.len() >= max_memory_slots {
            return Err(TraceContextError {
                what: "memory slots",
                limit: max_memory_slots,
            });
        }
        self.memory_mut()
            .insert(slot, MemoryCell::Value(stored_value(value)));
        Ok(())
    }

    /// Set the `this` the running VM function was called with. A plain call
    /// such as the initial thread's passes `undefined`.
    pub fn set_receiver(&mut self, value: SymbolicValue) -> Result<(), TraceContextError> {
        reject_external_computed(&value)?;
        self.receiver = Some(value);
        Ok(())
    }

    /// Initialize a slot with a fresh `undefined` cell.
    ///
    /// This deliberately replaces an existing cell, matching the canonical
    /// helper's `this.slots[index] = { value: void 0 }` behavior. Use
    /// [`Self::set_memory_value`] when a caller needs to seed a concrete cell.
    pub fn initialize_memory_slot(&mut self, slot: u32) -> Result<(), TraceContextError> {
        self.initialize_memory_slot_bounded(slot, TraceLimits::default().max_memory_slots)
    }

    pub fn initialize_memory_slot_bounded(
        &mut self,
        slot: u32,
        max_memory_slots: usize,
    ) -> Result<(), TraceContextError> {
        if !self.memory.contains_key(&slot) && self.memory.len() >= max_memory_slots {
            return Err(TraceContextError {
                what: "memory slots",
                limit: max_memory_slots,
            });
        }
        self.memory_mut().insert(slot, MemoryCell::Initialized);
        Ok(())
    }

    #[must_use]
    pub fn stack_length(&self) -> usize {
        self.stack.len()
    }

    /// The innermost `try` body changed `cell`.
    /// The shared cells this path knows exactly or concretely what it put
    /// there.
    pub(super) fn known_cells(&self) -> BTreeSet<u32> {
        self.cell_facts
            .keys()
            .chain(self.cell_nodes.keys())
            .chain(self.cell_concrete.keys())
            .copied()
            .collect()
    }

    pub(super) fn note_try_write(&mut self, cell: TraceCell) {
        if !self.try_writes.contains(&cell) {
            Arc::make_mut(&mut self.try_writes).insert(cell);
        }
    }

    pub(super) fn memory_mut(&mut self) -> &mut Slots {
        Arc::make_mut(&mut self.memory)
    }

    /// The value of one cell; a hole or an initialized slot is `undefined`.
    pub(super) fn cell(&self, cell: TraceCell) -> Option<StoredValue> {
        match cell {
            TraceCell::Stack(index) => self.stack.get(index).map(stack_cell_value),
            TraceCell::Memory(slot) => self.memory.get(&slot).map(memory_cell_value),
        }
    }

    pub(super) fn set_cell(&mut self, cell: TraceCell, value: StoredValue) {
        match cell {
            TraceCell::Stack(index) => self.stack[index] = StackCell::Value(value),
            TraceCell::Memory(slot) => {
                self.memory_mut().insert(slot, MemoryCell::Value(value));
            }
        }
    }

    #[must_use]
    pub fn memory_slot_count(&self) -> usize {
        self.memory.len()
    }

    /// Whether `slot` already owns a cell.
    ///
    /// Positional `writeSlot`-style writes update an existing cell and must not
    /// allocate one, so callers check this before
    /// [`Self::set_memory_value_bounded`].
    #[must_use]
    pub fn contains_memory_slot(&self, slot: u32) -> bool {
        self.memory.contains_key(&slot)
    }
}

/// Provenance carried by every constructed expression and statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceProvenance {
    pub handler_index: u32,
    pub table_key: u32,
    pub semantic_identity: Arc<str>,
    pub step_order: u32,
    /// Monotonic identity of this execution step across all handler calls.
    pub trace_step: u64,
    pub span: SpanRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceNodeKind {
    Undefined,
    Null,
    Boolean(bool),
    Number(String),
    String(String),
    Object {
        identity: u64,
    },
    External {
        name: String,
    },
    Identifier {
        name: String,
    },
    StackLength,
    StackRead {
        index: TraceNodeId,
    },
    MemoryRead {
        slot: TraceNodeId,
    },
    Binary {
        operator: PrimitiveOperator,
        left: TraceNodeId,
        right: TraceNodeId,
    },
    Unary {
        operator: PrimitiveOperator,
        value: TraceNodeId,
    },
    /// A free name such as `host`, emitted verbatim as an identifier reference.
    Global {
        name: String,
    },
    /// `object[key]`.
    PropertyRead {
        object: TraceNodeId,
        key: TraceNodeId,
    },
    /// `{ keys[i]: values[i], ... }`.
    ObjectLiteral {
        keys: Vec<String>,
        values: Vec<TraceNodeId>,
    },
    /// `[elements...]`.
    ArrayLiteral {
        elements: Vec<TraceNodeId>,
    },
    /// A VM closure: a program function whose body is the VM thread at
    /// `byte_offset` in `dispatch_state`, built from thread descriptor
    /// `descriptor`. Emitted as `vmCreateThread(descriptor, byte_offset,
    /// dispatch_state, memory)` until the thread body is traced. `memory`
    /// is `{ slot: cell, ... }`, the shared cells of the slots the closure
    /// captures, when the walk knows which those are.
    VmFunction {
        descriptor: usize,
        byte_offset: usize,
        dispatch_state: usize,
        memory: Option<TraceNodeId>,
    },
    /// A program function call with an explicit receiver:
    /// `callee.call(this_value, arguments...)`.
    Call {
        callee: TraceNodeId,
        this_value: TraceNodeId,
        arguments: Vec<TraceNodeId>,
    },
    /// `new callee(arguments...)`.
    New {
        callee: TraceNodeId,
        arguments: Vec<TraceNodeId>,
    },
    /// `delete object[key]`.
    Delete {
        object: TraceNodeId,
        key: TraceNodeId,
    },
    /// A closed function expression, emitted from its source text.
    Function {
        source: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceNode {
    pub id: TraceNodeId,
    pub kind: TraceNodeKind,
    pub provenance: TraceProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceStatementKind {
    Let {
        name: String,
        value: TraceNodeId,
    },
    Assign {
        name: String,
        value: TraceNodeId,
    },
    Evaluate {
        value: TraceNodeId,
    },
    Return {
        value: TraceNodeId,
    },
    Throw {
        value: TraceNodeId,
    },
    /// `object[key] = value`.
    PropertyWrite {
        object: TraceNodeId,
        key: TraceNodeId,
        value: TraceNodeId,
    },
    /// `let names...;` without initializers: the variables the next `if`
    /// merges its branches into. Filled in when the branches join.
    Declare {
        names: Vec<String>,
    },
    /// `if (test) { taken } else { fall_through }`: a conditional jump on a
    /// program value. The arms list the statements that belong to them, not
    /// to the enclosing block.
    If {
        test: TraceNodeId,
        taken: Vec<TraceStatementId>,
        fall_through: Vec<TraceStatementId>,
    },
    /// `label: for (;;) { body }`: a VM loop. A path that comes back to the
    /// loop header ends with `Continue`; one that leaves the loop where code
    /// outside it meets it ends with `Break`, any other with `return`.
    Loop {
        label: String,
        body: Vec<TraceStatementId>,
    },
    /// `continue label;`
    Continue {
        label: String,
    },
    /// `break label;`
    Break {
        label: String,
    },
    /// `function name(params) { body }`: a VM subroutine traced once, at
    /// the top level, and called from every site that calls it.
    Function {
        name: String,
        params: Vec<String>,
        body: Vec<TraceStatementId>,
    },
    /// `try { body } catch (param) { handler } finally { finalizer }`: a VM
    /// `try`. The handler is the code the VM runs when the body throws. A
    /// `finally` is code the VM enters both from the end of the body (with a
    /// record of where to go on) and on a throw: traced once, from both, it
    /// is the finalizer, and the handler is then no `catch` at all. With no
    /// finalizer and no `catch`, the handler is the `finally`, traced after
    /// a throw only: every other way in returned (see
    /// `TraceSession::skip_finally`).
    Try {
        param: String,
        body: Vec<TraceStatementId>,
        handler: Vec<TraceStatementId>,
        /// Whether the handler is a `catch`: not for `try { } finally { }`.
        catches: bool,
        finalizer: Option<Vec<TraceStatementId>>,
    },
}

/// A block that owns statements: an arm of an `If` or the body of a `Loop`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockArm {
    /// The jump was taken: the condition holds.
    Taken,
    /// The jump fell through.
    FallThrough,
    LoopBody,
    TryBody,
    Catch,
    Finally,
    FunctionBody,
}

/// Where new statements go: a block of an `If` or `Loop` statement, or the
/// function body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BlockTarget {
    pub(super) statement: TraceStatementId,
    pub(super) arm: BlockArm,
}

/// One VM storage cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TraceCell {
    Stack(usize),
    Memory(u32),
}

/// What a path knows a shared cell holds, for a VM jump through it (see
/// `TraceContext::cell_positions` and `cell_values`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellMark {
    /// A VM position this path wrote there.
    Position(usize),
    /// A value that is no VM position.
    Value,
    Unknown,
}

impl CellMark {
    pub(super) fn of(context: &TraceContext, slot: u32) -> Self {
        match context.cell_positions.get(&slot) {
            Some(&position) => Self::Position(position),
            None if context.cell_values.contains(&slot) => Self::Value,
            None => Self::Unknown,
        }
    }

    /// What a cell holds where paths meet: a VM jump on a path that put a
    /// value there goes to a program value, so one through it after they
    /// meet goes to the position the other path wrote.
    #[must_use]
    pub fn join(self, other: Self) -> Self {
        match (self, other) {
            (a, b) if a == b => a,
            (Self::Position(position), Self::Value) | (Self::Value, Self::Position(position)) => {
                Self::Position(position)
            }
            _ => Self::Unknown,
        }
    }

    pub(super) fn set(self, context: &mut TraceContext, slot: u32) {
        if self == Self::of(context, slot) {
            return;
        }
        let positions = Arc::make_mut(&mut context.cell_positions);
        let values = Arc::make_mut(&mut context.cell_values);
        match self {
            Self::Position(position) => {
                positions.insert(slot, position);
                values.remove(&slot);
            }
            Self::Value => {
                positions.remove(&slot);
                values.insert(slot);
            }
            Self::Unknown => {
                positions.remove(&slot);
                values.remove(&slot);
            }
        }
    }
}

/// What one side of a join knew where it gave the join's variable its value
/// (see [`TraceSession::note_join_arms`]).
#[derive(Debug, Clone)]
pub(super) struct JoinArm {
    pub(super) value: TraceNodeId,
    /// The value is falsy on that side: a short-circuit's operand.
    pub(super) falsy: bool,
    pub(super) branch_facts: Arc<BTreeMap<String, bool>>,
    pub(super) type_facts: Arc<BTreeMap<String, u16>>,
}

/// An open loop created by [`TraceSession::open_loop`]: the state at its
/// header and the variables that carry values between iterations.
#[derive(Debug, Clone)]
pub struct TraceLoop {
    pub(super) label: String,
    pub(super) parent: Option<BlockTarget>,
    /// The state at the header, with loop variables in the widened cells.
    pub(super) entry: TraceContext,
    pub(super) variables: BTreeMap<TraceCell, String>,
    /// The value each loop variable started with.
    pub(super) starts: BTreeMap<TraceCell, TraceNodeId>,
    /// Declares what a `break` carries out of the loop.
    pub(super) declare: TraceStatementId,
    /// The first trace node made inside the loop body.
    pub(super) first_node: usize,
}

impl TraceLoop {
    /// The loop variable that carries `cell`.
    #[must_use]
    pub fn variable_name(&self, cell: TraceCell) -> Option<&String> {
        self.variables.get(&cell)
    }

    /// Whether `name` is one of this loop's variables.
    #[must_use]
    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.values().any(|variable| variable == name)
    }
}

/// A VM subroutine being traced as a function: see
/// [`TraceSession::open_subroutine`].
#[derive(Debug, Clone)]
pub struct SubroutineFrame {
    pub(super) name: String,
    pub(super) statement: TraceStatementId,
    pub(super) caller: TraceContext,
    pub(super) caller_block: Option<BlockTarget>,
    pub(super) entry_stack: usize,
}

/// A caller cell relative to the call: a stack cell by its distance from
/// the stack length at the call (`-1` is the top), or a memory slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum CallCell {
    Stack(isize),
    Memory(u32),
}

/// What a subroutine leaves in a caller cell.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum CallOutput {
    /// The same value at every call.
    Constant(SymbolicValue),
    /// The `index`th value the function returns.
    Returned(usize),
}

/// A VM subroutine traced as `function name(params)`: which caller cells it
/// reads, and what it leaves in which cells. See
/// [`TraceSession::call_subroutine`].
#[derive(Debug, Clone, PartialEq)]
pub struct SubroutineSummary {
    pub(super) name: String,
    pub(super) statement: TraceStatementId,
    pub(super) params: Vec<CallCell>,
    pub(super) outputs: Vec<(CallCell, CallOutput)>,
    pub(super) returned: usize,
    pub(super) stack_delta: isize,
    /// Shared cells the function writes a caller cell's value to.
    pub(super) cell_writes: Vec<(u32, CallCell)>,
    /// What it returns, in terms of its parameters, by name.
    pub(super) returns: Vec<TraceNodeId>,
    /// For each of `returns`, when it is an object the function made and
    /// held alone up to its return: the properties it knew of it then (see
    /// [`TraceContext::made_objects`]). Each call returns a new one.
    pub(super) returned_objects: Vec<Option<BTreeMap<String, SymbolicValue>>>,
    pub(super) param_names: Vec<String>,
    /// For each of `param_names`, whether the body hands it on (a call, a
    /// store, a return) or writes a property of it: an object passed there
    /// is no longer the caller's alone.
    pub(super) param_escapes: Arc<[bool]>,
    /// For each of `param_names` that escapes only because the body defines
    /// properties on it with `Object.defineProperty` and a plain data
    /// descriptor (see `Executor::plain_definition`): those definitions, in
    /// order, as the key node (in terms of the parameters) and the value
    /// when known. The body is straight-line code, so each runs once a
    /// call; a made object passed there stays the caller's alone, with
    /// those properties.
    pub(super) param_defines: Arc<[Option<Vec<(TraceNodeId, Option<SymbolicValue>)>>]>,
    /// The parameter the function returns, as its one value, when the body
    /// hands it on nowhere else (its `param_defines` entry is `Some`): a
    /// call's result is the object passed there, not another one.
    pub(super) returns_param: Option<usize>,
}

/// A saved trace state, restored by [`TraceSession::rollback`].
#[derive(Debug, Clone)]
pub struct TraceCheckpoint(pub(super) Checkpoint);

/// A conditional jump whose condition is a program value: the target is
/// `(byte_offset, dispatch_state)` when it holds, the fall-through otherwise.
/// When `decided` is set the test was decided (a fake branch, recognized as
/// `by` says) and the handler already chose the successor; otherwise the
/// driver traces both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingBranch {
    pub test: TraceNodeId,
    pub byte_offset: usize,
    pub dispatch_state: usize,
    pub decided: Option<bool>,
    pub by: BranchEvidence,
}

/// One successor of a fork: its own VM state and the `if` arm its statements
/// go to. A driver runs it by [`TraceSession::switch_path`].
#[derive(Debug, Clone)]
pub struct TracePath {
    pub(super) context: TraceContext,
    pub(super) block: Option<BlockTarget>,
}

impl TracePath {
    /// The VM call stack of this path, innermost frame last.
    #[must_use]
    pub fn call_frames(&self) -> &[CallFrame] {
        &self.context.call_frames
    }

    /// The number of active `try` frames on this path.
    #[must_use]
    pub fn try_depth(&self) -> usize {
        self.context.try_frames.len()
    }
}

/// An open `if` created by [`TraceSession::fork`], closed by
/// [`TraceSession::join`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TraceFork {
    pub(crate) statement: TraceStatementId,
    pub(super) declare: TraceStatementId,
    /// The first trace node made inside the statement's blocks.
    pub(super) first_node: usize,
    /// The serial of the first variable named inside them (see
    /// `generated_binding_serial`).
    pub(super) first_binding: usize,
    pub(super) parent: Option<BlockTarget>,
    /// The blocks of the two paths, in `join` order.
    pub(super) arms: (BlockArm, BlockArm),
}

/// Why two branches cannot merge into one VM state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceJoinError(pub String);

impl fmt::Display for TraceJoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceStatement {
    pub id: TraceStatementId,
    pub kind: TraceStatementKind,
    pub provenance: TraceProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceEventKind {
    StackRead {
        index: TraceNodeId,
        value: TraceNodeId,
    },
    StackWrite {
        index: TraceNodeId,
        value: TraceNodeId,
    },
    StackLengthRead {
        value: TraceNodeId,
    },
    StackLengthSet {
        value: TraceNodeId,
    },
    MemoryRead {
        slot: TraceNodeId,
        value: TraceNodeId,
    },
    MemoryWrite {
        slot: TraceNodeId,
        value: TraceNodeId,
    },
    MemoryInitialize {
        slot: TraceNodeId,
    },
    /// A conditional jump whose test is a program value is a fake branch:
    /// only the `taken` side is traced. `by` says how that was decided.
    BranchDecided {
        test: TraceNodeId,
        taken: bool,
        by: BranchEvidence,
    },
    /// An event of a fork's side that the walk cut back to an earlier
    /// step: that side never made it.
    Undone,
}

/// How a fake branch was recognized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchEvidence {
    /// What is known about the test's operands decides it.
    Test,
    /// The other side reaches a state real code never reaches.
    Context,
    /// The user decided it (see [`BranchOverrides::fake`]).
    Override,
}

/// Conditional jumps the user decides, by the jump instruction's position
/// (`pc`, `state`), as `shape/`'s `--fake-branches` and
/// `--not-fake-branches` did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BranchOverrides {
    /// Real branches: never decided fake, by their test or by what a side
    /// reaches. A side that reaches a state real code never reaches stops
    /// the walk instead: no other branch takes the evidence.
    pub real: BTreeSet<(usize, usize)>,
    /// Fake branches and the side each always takes: `true`, the jump.
    pub fake: BTreeMap<(usize, usize), bool>,
}

impl BranchOverrides {
    /// Parse a position written `pc/state`.
    pub fn position(text: &str) -> Result<(usize, usize), String> {
        let (pc, state) = text
            .split_once('/')
            .ok_or_else(|| format!("branch position {text:?} is not pc/state"))?;
        let number = |part: &str| {
            part.trim()
                .parse::<usize>()
                .map_err(|_| format!("branch position {text:?} is not pc/state"))
        };
        Ok((number(pc)?, number(state)?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEvent {
    pub id: u32,
    pub kind: TraceEventKind,
    pub provenance: TraceProvenance,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraceArena {
    pub nodes: Vec<TraceNode>,
    pub statements: Vec<TraceStatement>,
    /// VM storage actions are retained for audit/provenance but intentionally
    /// have no JavaScript statement: their values are already in the graph.
    pub events: Vec<TraceEvent>,
}

impl TraceArena {
    /// A one-line JavaScript-like rendering of `node` for diagnostics, with
    /// `let` bindings that are never reassigned replaced by their values. Not emission: it is not
    /// validated and deep or repeated parts are abbreviated.
    #[must_use]
    pub fn describe(&self, node: TraceNodeId) -> String {
        self.describer().describe(node)
    }

    /// Which nodes a statement's values reach. A node no statement reaches
    /// was built on a path the walk went back from: it is in no code.
    #[must_use]
    pub fn live_nodes(&self) -> Vec<bool> {
        let mut live = vec![false; self.nodes.len()];
        let mut pending: Vec<TraceNodeId> = Vec::new();
        for statement in &self.statements {
            match &statement.kind {
                TraceStatementKind::Let { value, .. }
                | TraceStatementKind::Assign { value, .. }
                | TraceStatementKind::Evaluate { value }
                | TraceStatementKind::Return { value }
                | TraceStatementKind::Throw { value }
                | TraceStatementKind::If { test: value, .. } => pending.push(*value),
                TraceStatementKind::PropertyWrite { object, key, value } => {
                    pending.extend([*object, *key, *value]);
                }
                _ => {}
            }
        }
        while let Some(node) = pending.pop() {
            let Some(seen) = live.get_mut(node as usize) else {
                continue;
            };
            if std::mem::replace(seen, true) {
                continue;
            }
            pending.extend(node_children(&self.nodes[node as usize].kind));
        }
        live
    }

    /// A reusable [`Self::describe`]: the binding map is built once, so
    /// describing many nodes stays linear.
    #[must_use]
    pub fn describer(&self) -> TraceDescriber<'_> {
        // A reassigned name (a merge or loop variable) has no single value.
        let reassigned: BTreeSet<&str> = self
            .statements
            .iter()
            .filter_map(|statement| match &statement.kind {
                TraceStatementKind::Assign { name, .. } => Some(name.as_str()),
                _ => None,
            })
            .collect();
        let bindings: BTreeMap<&str, TraceNodeId> = self
            .statements
            .iter()
            .filter_map(|statement| match &statement.kind {
                TraceStatementKind::Let { name, value } if !reassigned.contains(name.as_str()) => {
                    Some((name.as_str(), *value))
                }
                _ => None,
            })
            .collect();
        TraceDescriber {
            arena: self,
            bindings,
        }
    }

    pub(super) fn describe_into(
        &self,
        node: TraceNodeId,
        bindings: &BTreeMap<&str, TraceNodeId>,
        depth: usize,
        out: &mut String,
    ) {
        use std::fmt::Write as _;
        let Some(kind) = self.nodes.get(node as usize).map(|node| &node.kind) else {
            out.push('?');
            return;
        };
        if depth > 12 {
            out.push('…');
            return;
        }
        let mut child = |node: TraceNodeId, out: &mut String| {
            self.describe_into(node, bindings, depth + 1, out);
        };
        let list = |nodes: &[TraceNodeId],
                    out: &mut String,
                    child: &mut dyn FnMut(TraceNodeId, &mut String)| {
            for (index, node) in nodes.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                child(*node, out);
            }
        };
        match kind {
            TraceNodeKind::Undefined => out.push_str("undefined"),
            TraceNodeKind::Null => out.push_str("null"),
            TraceNodeKind::Boolean(value) => {
                let _ = write!(out, "{value}");
            }
            TraceNodeKind::Number(text) => out.push_str(text),
            TraceNodeKind::String(text) => {
                let _ = write!(out, "{text:?}");
            }
            TraceNodeKind::Object { identity } => {
                let _ = write!(out, "object_{identity}");
            }
            TraceNodeKind::External { name } | TraceNodeKind::Global { name } => out.push_str(name),
            TraceNodeKind::Identifier { name } => match bindings.get(name.as_str()) {
                Some(value) => child(*value, out),
                None => out.push_str(name),
            },
            TraceNodeKind::StackLength => out.push_str("<stack length>"),
            TraceNodeKind::StackRead { .. } => out.push_str("<stack read>"),
            TraceNodeKind::MemoryRead { .. } => out.push_str("<memory read>"),
            TraceNodeKind::Binary {
                operator,
                left,
                right,
            } => {
                out.push('(');
                child(*left, out);
                let _ = write!(
                    out,
                    " {} ",
                    binary_operator(*operator).map_or("?", |operator| operator.as_str())
                );
                child(*right, out);
                out.push(')');
            }
            TraceNodeKind::Unary { operator, value } => {
                let _ = write!(
                    out,
                    "{}",
                    unary_operator(*operator).map_or("?", |operator| operator.as_str())
                );
                if matches!(
                    operator,
                    PrimitiveOperator::Typeof | PrimitiveOperator::Void
                ) {
                    out.push(' ');
                }
                child(*value, out);
            }
            TraceNodeKind::PropertyRead { object, key } => {
                child(*object, out);
                out.push('[');
                child(*key, out);
                out.push(']');
            }
            TraceNodeKind::ObjectLiteral { keys, .. } => {
                let _ = write!(out, "{{{}}}", keys.join(", "));
            }
            TraceNodeKind::ArrayLiteral { elements } => {
                out.push('[');
                list(elements, out, &mut child);
                out.push(']');
            }
            TraceNodeKind::VmFunction {
                descriptor,
                byte_offset,
                dispatch_state,
                memory,
            } => {
                let _ = write!(
                    out,
                    "vmCreateThread({descriptor}, {byte_offset}, {dispatch_state}"
                );
                if let Some(memory) = memory {
                    out.push_str(", ");
                    child(*memory, out);
                }
                out.push(')');
            }
            TraceNodeKind::Call {
                callee, arguments, ..
            } => {
                child(*callee, out);
                out.push('(');
                list(arguments, out, &mut child);
                out.push(')');
            }
            TraceNodeKind::New { callee, arguments } => {
                out.push_str("new ");
                child(*callee, out);
                out.push('(');
                list(arguments, out, &mut child);
                out.push(')');
            }
            TraceNodeKind::Delete { object, key } => {
                out.push_str("delete ");
                child(*object, out);
                out.push('[');
                child(*key, out);
                out.push(']');
            }
            TraceNodeKind::Function { .. } => out.push_str("function () { … }"),
        }
    }
}

/// Renders trace nodes for diagnostics; see [`TraceArena::describer`].
pub struct TraceDescriber<'a> {
    pub(super) arena: &'a TraceArena,
    pub(super) bindings: BTreeMap<&'a str, TraceNodeId>,
}

impl TraceDescriber<'_> {
    /// One line of JavaScript-like text for `node`.
    #[must_use]
    pub fn describe(&self, node: TraceNodeId) -> String {
        let mut text = String::new();
        self.arena.describe_into(node, &self.bindings, 0, &mut text);
        text
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceValue {
    pub value: SymbolicValue,
    pub node: TraceNodeId,
}

/// Closed set of stop and diagnostic codes produced by the symbolic tracer or
/// by a bounded driver that owns a [`TraceSession`].
///
/// The variants are grouped by origin only in their names: tracer-level
/// diagnostics, complete-dispatch driver halts (`dispatch-*`), and prefix driver
/// halts (`prefix-*`). [`Self::as_str`] is the single source of the externally
/// visible strings; it is used at every serialization and display point.
///
/// The custom [`fmt::Debug`] implementation renders the same quoted string a
/// `String` used to render, so debugging a completion, diagnostic, or finish
/// error stays byte-identical.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TraceStopCode {
    // Tracer diagnostics.
    ComputedInput,
    InitialStackBudget,
    InitialMemoryBudget,
    StepBudget,
    MissingHandler,
    UnsupportedHandler,
    Budget,
    UnknownOperation,
    ContextValue,
    ContextOperand,
    MissingOperand,
    MissingTemporary,
    MissingMemorySlot,
    MissingMemorySlotWrite,
    MissingVmCursor,
    EmptyCallFrameStack,
    CallFrameReadBeforePop,
    DynamicCallArguments,
    DynamicBranch,
    UnhandledThreadReturn,
    UnhandledTry,
    EmptyExceptionStack,
    /// A `catch` read below the stack its `try` was entered with. The VM
    /// keeps the stack the throw left, which the tracer does not know.
    CatchStackUnknown,
    /// A concrete read past the end of a constant table.
    ConstantTableIndex,
    DynamicIndex,
    InvalidIndex,
    UnknownValue,
    UnknownInput,
    InvalidComputedInput,
    InvalidNumber,
    // Complete-dispatch driver halts.
    DispatchTableOverflow,
    DispatchPrepareBudget,
    DispatchRepeatedPcState,
    DispatchTruncatedImage,
    DispatchMissingState,
    DispatchMissingOpcode,
    DispatchInvalidNextState,
    DispatchCompletionPcOverflow,
    DispatchCompletionBudget,
    DispatchMissingHandler,
    DispatchPcOverflow,
    DispatchTruncatedOperands,
    DispatchUnsupportedOperands,
    DispatchNextPcOverflow,
    DispatchStepBudget,
    // Prefix driver halts.
    PrefixRepeatedPcState,
    PrefixTruncatedImage,
    PrefixMissingState,
    PrefixMissingOpcode,
    PrefixInvalidNextState,
    PrefixPcOverflow,
    PrefixImagePatch,
    PrefixMissingHandler,
    PrefixUnsupportedHandler,
    PrefixMissingOperands,
    PrefixUnliftedHandler,
    PrefixTruncatedOperands,
    PrefixUnsupportedOperands,
    PrefixNextPcOverflow,
    PrefixThrowingHandler,
    PrefixThrownHandler,
    PrefixStepBudget,
    PrefixBranchJoin,
    PrefixLoopStackChange,
    ProgramValueJump,
    /// A step reached the holes a join put into the stack to line two
    /// sides up from the top, or counted stack positions from the bottom:
    /// the join is undone.
    PaddedStack,
    /// A VM call of a known string, number or boolean: it throws a
    /// `TypeError`, which code that runs does not do.
    PrimitiveCall,
    /// A VM function made whose first instruction reads below the empty
    /// stack it starts with: no call of it runs.
    JunkFunctionEntry,
    /// A selector the WASM interpreter traps on: no code that runs gets
    /// there.
    WasmTrap,
    /// A `try` entered after more `try` exits than entries: the WASM
    /// interpreter traps there (see `TraceContext::try_underflow`).
    TryUnderflow,
}

impl TraceStopCode {
    /// The exact code string used by trace reports, diagnostics, and errors.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ComputedInput => "computed-input",
            Self::InitialStackBudget => "initial-stack-budget",
            Self::InitialMemoryBudget => "initial-memory-budget",
            Self::StepBudget => "step-budget",
            Self::MissingHandler => "missing-handler",
            Self::UnsupportedHandler => "unsupported-handler",
            Self::Budget => "budget",
            Self::UnknownOperation => "unknown-operation",
            Self::ContextValue => "context-value",
            Self::ContextOperand => "context-operand",
            Self::MissingOperand => "missing-operand",
            Self::MissingTemporary => "missing-temporary",
            Self::MissingMemorySlot => "missing-memory-slot",
            Self::MissingMemorySlotWrite => "missing-memory-slot-write",
            Self::MissingVmCursor => "missing-vm-cursor",
            Self::EmptyCallFrameStack => "empty-call-frame-stack",
            Self::CallFrameReadBeforePop => "call-frame-read-before-pop",
            Self::DynamicCallArguments => "dynamic-call-arguments",
            Self::DynamicBranch => "dynamic-branch",
            Self::UnhandledThreadReturn => "unhandled-thread-return",
            Self::UnhandledTry => "unhandled-try",
            Self::EmptyExceptionStack => "empty-exception-stack",
            Self::CatchStackUnknown => "catch-stack-unknown",
            Self::ConstantTableIndex => "constant-table-index",
            Self::DynamicIndex => "dynamic-index",
            Self::InvalidIndex => "invalid-index",
            Self::UnknownValue => "unknown-value",
            Self::UnknownInput => "unknown-input",
            Self::InvalidComputedInput => "invalid-computed-input",
            Self::InvalidNumber => "invalid-number",
            Self::DispatchTableOverflow => "dispatch-table-overflow",
            Self::DispatchPrepareBudget => "dispatch-prepare-budget",
            Self::DispatchRepeatedPcState => "dispatch-repeated-pc-state",
            Self::DispatchTruncatedImage => "dispatch-truncated-image",
            Self::DispatchMissingState => "dispatch-missing-state",
            Self::DispatchMissingOpcode => "dispatch-missing-opcode",
            Self::DispatchInvalidNextState => "dispatch-invalid-next-state",
            Self::DispatchCompletionPcOverflow => "dispatch-completion-pc-overflow",
            Self::DispatchCompletionBudget => "dispatch-completion-budget",
            Self::DispatchMissingHandler => "dispatch-missing-handler",
            Self::DispatchPcOverflow => "dispatch-pc-overflow",
            Self::DispatchTruncatedOperands => "dispatch-truncated-operands",
            Self::DispatchUnsupportedOperands => "dispatch-unsupported-operands",
            Self::DispatchNextPcOverflow => "dispatch-next-pc-overflow",
            Self::DispatchStepBudget => "dispatch-step-budget",
            Self::PrefixRepeatedPcState => "prefix-repeated-pc-state",
            Self::PrefixTruncatedImage => "prefix-truncated-image",
            Self::PrefixMissingState => "prefix-missing-state",
            Self::PrefixMissingOpcode => "prefix-missing-opcode",
            Self::PrefixInvalidNextState => "prefix-invalid-next-state",
            Self::PrefixPcOverflow => "prefix-pc-overflow",
            Self::PrefixImagePatch => "prefix-image-patch",
            Self::PrefixMissingHandler => "prefix-missing-handler",
            Self::PrefixUnsupportedHandler => "prefix-unsupported-handler",
            Self::PrefixMissingOperands => "prefix-missing-operands",
            Self::PrefixUnliftedHandler => "prefix-unlifted-handler",
            Self::PrefixTruncatedOperands => "prefix-truncated-operands",
            Self::PrefixUnsupportedOperands => "prefix-unsupported-operands",
            Self::PrefixNextPcOverflow => "prefix-next-pc-overflow",
            Self::PrefixThrowingHandler => "prefix-throwing-handler",
            Self::PrefixThrownHandler => "prefix-thrown-handler",
            Self::PrefixStepBudget => "prefix-step-budget",
            Self::PrefixBranchJoin => "prefix-branch-join",
            Self::PrefixLoopStackChange => "prefix-loop-stack-change",
            Self::ProgramValueJump => "program-value-jump",
            Self::PaddedStack => "padded-stack",
            Self::PrimitiveCall => "primitive-call",
            Self::JunkFunctionEntry => "junk-function-entry",
            Self::WasmTrap => "wasm-trap",
            Self::TryUnderflow => "try-underflow",
        }
    }
}

impl fmt::Display for TraceStopCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for TraceStopCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TraceCompletion {
    Completed,
    Returned(TraceValue),
    Thrown(TraceValue),
    PartialUnsupported,
    BudgetStopped,
    /// The owning driver halted the committed prefix before a terminal recipe
    /// outcome (for example a truncated image, a missing state/opcode, or a
    /// driver step budget). This is distinct from a tracer-level unsupported
    /// recipe or budget so a caller cannot report a partial prefix as a
    /// completed run.
    PrefixStopped(TraceStopCode),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceDiagnostic {
    pub code: TraceStopCode,
    pub message: String,
    pub provenance: Option<TraceProvenance>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceStateSnapshot {
    pub stack_length: usize,
    pub memory_slots: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TraceReport {
    pub arena: TraceArena,
    pub completion: TraceCompletion,
    pub diagnostics: Vec<TraceDiagnostic>,
    pub handlers_traced: usize,
    pub external_inputs: Vec<String>,
    pub final_state: TraceStateSnapshot,
}

/// Result of one incremental handler invocation. A recipe `return` is a
/// handler return, not a VM return; the dispatch driver decides whether the
/// VM's completion contract makes it terminal.
#[derive(Debug, Clone, PartialEq)]
pub enum TraceHandlerResult {
    Continued,
    Returned(TraceValue),
    Thrown(TraceValue),
    Partial,
    BudgetStopped,
}

/// Why a driver could not synthesize a typed terminal completion value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceFinishError {
    /// The session already reached a terminal completion and cannot accept a
    /// synthesized value.
    AlreadyStopped,
    /// The requested stack index is outside the configured stack bound.
    StackIndexOutOfRange { index: usize, limit: usize },
    /// The value graph could not admit the completion node/read under the
    /// configured node budget.
    ValueGraphUnavailable(String),
    /// The completion value could not be read because the seeded context or
    /// value graph contained an unsupported value.
    Unsupported {
        code: TraceStopCode,
        message: String,
    },
}

impl fmt::Display for TraceFinishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyStopped => f.write_str("trace session already stopped"),
            Self::StackIndexOutOfRange { index, limit } => write!(
                f,
                "completion stack index {index} exceeds configured stack limit {limit}"
            ),
            Self::ValueGraphUnavailable(reason) => {
                write!(f, "completion value could not be recorded: {reason}")
            }
            Self::Unsupported { code, message } => {
                write!(f, "completion value is unsupported ({code}): {message}")
            }
        }
    }
}
impl std::error::Error for TraceFinishError {}

/// Told each VM function a trace creates, as `(descriptor, byte offset,
/// dispatch state)`, the moment it does: a driver may start walking it while
/// the trace goes on. The trace may still drop it (a path given up).
pub type CreatedHook = Arc<dyn Fn(usize, usize, usize) + Send + Sync>;

/// Whether every return of the VM function made from descriptor, byte
/// offset and dispatch state gives a VM function made: a call of it then is
/// a function, to tests (see [`TraceSession::with_returns_hook`]).
pub type ReturnsHook = Arc<dyn Fn(usize, usize, usize) -> bool + Send + Sync>;

/// The JavaScript types (as [`crate::tracing::predicate`] type bits) the VM
/// function made from descriptor, byte offset and dispatch state puts in
/// its captured cell of `slot`, with every function it makes that captures
/// that slot too: all a closure holding a cell can write there. `None` when
/// not known (a walk that stopped, a write of a value of unknown type).
/// See [`TraceSession::with_cell_types_hook`].
pub type CellTypesHook = Arc<dyn Fn(usize, usize, usize, u32) -> Option<u16> + Send + Sync>;
