//! Canonical kernel vocabulary.
//!
//! The names here follow the coherent naming contract: `vm` + descriptive
//! UpperCamel for JavaScript bindings, nouns for tables/values. A role is only
//! assigned when it is demonstrated structurally; display names never decide
//! identity.

use std::fmt;

/// Semantic role of a kernel-owned binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VmRole {
    /// VM entry (the outer IIFE function).
    Entry,
    /// Interpreter run callback (invokes the compiled VM for a context).
    Run,
    /// Descriptor factory: descriptor lookup then thread construction.
    CreateThread,
    /// Thread capture: builds a per-invocation memory snapshot and closure.
    CaptureThread,
    /// Thread invocation builder: returns the per-call closure.
    BuildThreadInvocation,
    /// Thread invocation closure (the value returned by the builder).
    InvokeThread,
    /// Thread body execution.
    ExecuteThread,
    /// Execution context constructor.
    Context,
    /// Memory constructor.
    Memory,
    /// Shared linear memory (WebAssembly.Memory instance).
    LinearMemory,
    /// Stack factory (array with native push/pop/slice/splice methods).
    CreateStack,
    /// Bytecode image.
    Bytecode,
    /// Embedded WebAssembly module bytes, distinct from VM bytecode.
    WasmModule,
    /// First WebAssembly module: operation-handler table module.
    WasmHandlerTableModule,
    /// Second WebAssembly module: bytecode interpreter module.
    WasmInterpreterModule,
    /// Bytes consumed by the handler-table module.
    WasmHandlerTableBytes,
    /// Bytes consumed by the interpreter module.
    WasmInterpreterBytes,
    /// First WebAssembly instance: operation-handler table instance.
    WasmHandlerTableInstance,
    /// Second WebAssembly instance: bytecode interpreter instance.
    WasmInterpreterInstance,
    /// Host imports object supplied to the interpreter module.
    HostImports,
    /// Dispatch row/byte to operation mapping.
    DispatchTable,
    /// Operation definitions.
    Operations,
    /// Thread descriptor records (per-thread initial/captured/argument slots).
    ThreadDescriptors,
    /// String constants.
    StringTable,
    /// Number constants.
    NumberTable,
    /// Prototype values.
    PrototypeTable,
    /// Decoded-string cache.
    StringCache,
    /// Bytecode / base64 decoder.
    DecodeBase64,
    /// Operation exception boundary.
    InvokeGuarded,
    /// Small wrapper that pops the current VM stack value.
    PopStack,
    /// Exception routing.
    HandleException,
    /// The native-function alias array.
    NativeFunctions,
    /// Reused property descriptor for the memory `slots` initializer.
    SlotDescriptor,
    /// Cached worker instance built from the entry source.
    Worker,
    /// Array of worker callbacks indexed by the inbound message payload.
    WorkerCallbacks,
    /// Worker bootstrap that builds the worker and installs `onmessage`.
    InitializeWorker,
    /// Sentinel meaning no pending `run` completion is recorded.
    NoResultSentinel,
    /// Sentinel returned by an operation that completes with no value.
    EmptyResultSentinel,
    /// Pending `run` completion value read by the interpreter loop.
    CompletionResult,
}

impl VmRole {
    /// All roles, in canonical order.
    pub const ALL: [VmRole; 39] = [
        VmRole::Entry,
        VmRole::Run,
        VmRole::CreateThread,
        VmRole::CaptureThread,
        VmRole::BuildThreadInvocation,
        VmRole::InvokeThread,
        VmRole::ExecuteThread,
        VmRole::Context,
        VmRole::Memory,
        VmRole::LinearMemory,
        VmRole::CreateStack,
        VmRole::Bytecode,
        VmRole::WasmModule,
        VmRole::WasmHandlerTableModule,
        VmRole::WasmInterpreterModule,
        VmRole::WasmHandlerTableBytes,
        VmRole::WasmInterpreterBytes,
        VmRole::WasmHandlerTableInstance,
        VmRole::WasmInterpreterInstance,
        VmRole::HostImports,
        VmRole::DispatchTable,
        VmRole::Operations,
        VmRole::ThreadDescriptors,
        VmRole::StringTable,
        VmRole::NumberTable,
        VmRole::PrototypeTable,
        VmRole::StringCache,
        VmRole::DecodeBase64,
        VmRole::InvokeGuarded,
        VmRole::PopStack,
        VmRole::HandleException,
        VmRole::NativeFunctions,
        VmRole::SlotDescriptor,
        VmRole::Worker,
        VmRole::WorkerCallbacks,
        VmRole::InitializeWorker,
        VmRole::NoResultSentinel,
        VmRole::EmptyResultSentinel,
        VmRole::CompletionResult,
    ];

    /// Canonical JavaScript binding name for this role.
    pub fn canonical(self) -> &'static str {
        match self {
            VmRole::Entry => "vmEntry",
            VmRole::Run => "vmRun",
            VmRole::CreateThread => "vmCreateThread",
            VmRole::CaptureThread => "vmCaptureThread",
            VmRole::BuildThreadInvocation => "vmBuildThreadInvocation",
            VmRole::InvokeThread => "vmInvokeThread",
            VmRole::ExecuteThread => "vmExecuteThread",
            VmRole::Context => "vmContext",
            VmRole::Memory => "vmMemory",
            VmRole::LinearMemory => "vmLinearMemory",
            VmRole::CreateStack => "vmCreateStack",
            VmRole::Bytecode => "vmBytecode",
            VmRole::WasmModule => "vmWasmModule",
            VmRole::WasmHandlerTableModule => "vmHandlerTableModule",
            VmRole::WasmInterpreterModule => "vmInterpreterModule",
            VmRole::WasmHandlerTableBytes => "vmHandlerTableBytes",
            VmRole::WasmInterpreterBytes => "vmInterpreterBytes",
            VmRole::WasmHandlerTableInstance => "vmHandlerTableInstance",
            VmRole::WasmInterpreterInstance => "vmInterpreterInstance",
            VmRole::HostImports => "vmHostImports",
            VmRole::DispatchTable => "vmDispatchTable",
            VmRole::Operations => "vmOperations",
            VmRole::ThreadDescriptors => "vmThreadDescriptors",
            VmRole::StringTable => "vmStringTable",
            VmRole::NumberTable => "vmNumberTable",
            VmRole::PrototypeTable => "vmPrototypeTable",
            VmRole::StringCache => "vmStringCache",
            VmRole::DecodeBase64 => "vmDecodeBase64",
            VmRole::InvokeGuarded => "vmInvokeOperation",
            VmRole::PopStack => "vmPopStack",
            VmRole::HandleException => "vmHandleException",
            VmRole::NativeFunctions => "vmNativeFunctions",
            VmRole::SlotDescriptor => "vmSlotDescriptor",
            VmRole::Worker => "vmWorker",
            VmRole::WorkerCallbacks => "vmWorkerCallbacks",
            VmRole::InitializeWorker => "vmInitializeWorker",
            VmRole::NoResultSentinel => "vmNoResultSentinel",
            VmRole::EmptyResultSentinel => "vmEmptyResultSentinel",
            VmRole::CompletionResult => "vmPendingResult",
        }
    }

    /// Rust vocabulary for the same role (UpperCamel type / snake_case concept).
    pub fn rust(self) -> &'static str {
        match self {
            VmRole::Entry => "VmEntry",
            VmRole::Run => "run",
            VmRole::CreateThread => "create_thread",
            VmRole::CaptureThread => "capture_thread",
            VmRole::BuildThreadInvocation => "build_thread_invocation",
            VmRole::InvokeThread => "invoke_thread",
            VmRole::ExecuteThread => "execute_thread",
            VmRole::Context => "VmContext",
            VmRole::Memory => "VmMemory",
            VmRole::LinearMemory => "LinearMemory",
            VmRole::CreateStack => "create_stack",
            VmRole::Bytecode => "BytecodeImage",
            VmRole::WasmModule => "WasmModule",
            VmRole::WasmHandlerTableModule => "HandlerTableModule",
            VmRole::WasmInterpreterModule => "InterpreterModule",
            VmRole::WasmHandlerTableBytes => "HandlerTableBytes",
            VmRole::WasmInterpreterBytes => "InterpreterBytes",
            VmRole::WasmHandlerTableInstance => "HandlerTableInstance",
            VmRole::WasmInterpreterInstance => "InterpreterInstance",
            VmRole::HostImports => "HostImports",
            VmRole::DispatchTable => "DispatchTable",
            VmRole::Operations => "DispatchOperation",
            VmRole::ThreadDescriptors => "ThreadDescriptor",
            VmRole::StringTable => "StringTable",
            VmRole::NumberTable => "NumberTable",
            VmRole::PrototypeTable => "PrototypeTable",
            VmRole::StringCache => "StringCache",
            VmRole::DecodeBase64 => "decode_base64",
            VmRole::InvokeGuarded => "invoke_operation",
            VmRole::PopStack => "pop_stack",
            VmRole::HandleException => "handle_exception",
            VmRole::NativeFunctions => "NativeFunctions",
            VmRole::SlotDescriptor => "VmSlotDescriptor",
            VmRole::Worker => "VmWorker",
            VmRole::WorkerCallbacks => "VmWorkerCallbacks",
            VmRole::InitializeWorker => "initialize_worker",
            VmRole::NoResultSentinel => "no_result_sentinel",
            VmRole::EmptyResultSentinel => "empty_result_sentinel",
            VmRole::CompletionResult => "completion_result",
        }
    }

    /// Positional parameter roles, most-significant first.
    ///
    /// Only parameters whose behaviour is confirmed by the surrounding structure
    /// are listed. Unknown parameters are reported as unresolved rather than
    /// being assigned a positional guess.
    pub fn parameter_roles(self) -> &'static [&'static str] {
        match self {
            VmRole::Entry => &["host", "entrySeed", "dispatchMode", "initialMemorySlots"],
            VmRole::Run => &["context"],
            VmRole::Context => &[],
            VmRole::CreateThread => &["descriptorIndex", "byteOffset", "dispatchState", "memory"],
            // Thread-helper parameters are named by the dataflow pass in
            // `naming.rs`; positional fallback would relabel shuffled or
            // shadowed parameters.
            VmRole::CaptureThread => &[],
            VmRole::BuildThreadInvocation => &[],
            VmRole::DecodeBase64 => &["encodedString"],
            VmRole::InvokeGuarded => &["context", "operand1", "operand2", "operand3", "operand4"],
            _ => &[],
        }
    }

    /// Short label used in diagnostics.
    pub fn label(self) -> &'static str {
        self.canonical()
    }
}

impl fmt::Display for VmRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.canonical())
    }
}
