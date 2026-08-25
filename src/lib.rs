#![warn(missing_docs)]

//! CoffeeScript-inspired parser, compiler, bytecode VM, and embedding API.
//! The public API intentionally exposes values and native functions, never JS-like objects.

mod ast;
mod bytecode;
mod lexer;
mod lowering;
mod module;
mod parser;
mod vm;

pub use bytecode::{Chunk, Constant, Instruction, Pattern};
pub use module::{
    MemoryModuleLoader, Module, ModuleExports, ModuleLoader, ModuleSource,
    RestrictedFileModuleLoader,
};
pub use vm::{
    CancellationToken, Context, Decimal, DiagnosticLabel, DiagnosticLabelKind, Engine, Error,
    ErrorKind, ExecutionStats, Function, Integer, NativeFunction, Program, ResourceLimit,
    ScriptError, SourcePosition, SourceSpan, Value, ValueKind,
};

/// Compiles `source` to verified bytecode without executing it.
pub fn compile(source: &str) -> Result<Chunk, Error> {
    let ast = parser::parse(source)?;
    let chunk = lowering::compile(&ast)?;
    chunk.verify()?;
    Ok(chunk)
}

/// Compiles `source` to verified bytecode and attaches the opaque
/// host-provided `source_name` to any diagnostic labels produced on failure.
pub fn compile_named(source_name: &str, source: &str) -> Result<Chunk, Error> {
    compile(source).map_err(|error| error.with_source_name(source_name))
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
