#![warn(missing_docs)]

//! CoffeeScript-inspired parser, compiler, bytecode VM, and embedding API.
//! The public API intentionally exposes values and native functions, never JS-like objects.

mod ast;
mod bytecode;
mod cson;
mod json;
mod lexer;
mod lowering;
mod module;
mod parser;
mod resource;
mod source;
mod vm;

pub use bytecode::{Chunk, Constant, Instruction, Pattern};
pub use cson::{CsonError, CsonErrorCode, CsonLimits, parse_cson, parse_cson_with_limits};
pub use module::{
    MODULE_GRAPH_FINGERPRINT_VERSION, MemoryModuleLoader, Module, ModuleExports, ModuleLoader,
    ModulePackage, ModuleSource, RestrictedFileModuleLoader,
};
pub use resource::{CompileLimits, ResourceLimit, ResourceLimits};
pub use vm::{
    CancellationToken, CapabilityKey, CapabilityKind, Context, ContextBuilder,
    ContextualNativeFunction, Decimal, DiagnosticLabel, DiagnosticLabelKind, Engine, Error,
    ErrorKind, ExecutionPolicy, ExecutionStats, Function, HostCapabilities, HostState, Integer,
    IntoValue, LiveManagedMemory, LiveMemoryCheckpoint, LiveMemoryObservation, LiveMemoryOutcome,
    LiveMemoryReport, NativeCallContext, NativeFunction, Program, RetainedMemory, Runtime,
    RuntimeBuilder, RuntimeCacheStats, ScriptError, SourcePosition, SourceSpan, TryFromValue,
    Value, ValueKind,
};

/// Compiles `source` to verified bytecode without executing it.
pub fn compile(source: &str) -> Result<Chunk, Error> {
    Engine::new().compile(source)
}

/// Compiles `source` to verified bytecode and attaches the opaque
/// host-provided `source_name` to any diagnostic labels produced on failure.
/// A name ending in `.litcoffee` enables literate CoffeeScript preprocessing.
pub fn compile_named(source_name: &str, source: &str) -> Result<Chunk, Error> {
    Engine::new().compile_named(source_name, source)
}

/// Compiles `source` to a cheaply cloneable shared verified program.
pub fn compile_program(source: &str) -> Result<Program, Error> {
    Engine::new().compile_program(source)
}

/// Compiles named source to a cheaply cloneable shared verified program.
pub fn compile_program_named(source_name: &str, source: &str) -> Result<Program, Error> {
    Engine::new().compile_program_named(source_name, source)
}

/// Evaluates `source` in a freshly created context.
pub fn eval(source: &str) -> Result<Value, Error> {
    Context::new().eval(source)
}

/// Evaluates named source in a freshly created context.
pub fn eval_named(source_name: &str, source: &str) -> Result<Value, Error> {
    Context::new().eval_named(source_name, source)
}
