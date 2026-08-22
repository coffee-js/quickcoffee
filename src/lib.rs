#![warn(missing_docs)]

//! CoffeeScript-inspired parser, compiler, bytecode VM, and embedding API.
//! The public API intentionally exposes values and native functions, never JS-like objects.

mod bytecode;
mod lexer;
mod parser;
mod vm;

pub use bytecode::{Chunk, Constant, Instruction, Pattern};
pub use vm::{
    Context, Engine, Error, ErrorKind, ExecutionStats, Function, NativeFunction, Program,
    SourcePosition, Value,
};

/// Compiles `source` to verified bytecode without executing it.
pub fn compile(source: &str) -> Result<Chunk, Error> {
    let ast = parser::parse(source)?;
    let chunk = bytecode::compile(&ast)?;
    chunk.verify()?;
    Ok(chunk)
}

/// Compiles `source` to a cheaply cloneable shared verified program.
pub fn compile_program(source: &str) -> Result<Program, Error> {
    Engine::new().compile_program(source)
}

/// Evaluates `source` in a freshly created context.
pub fn eval(source: &str) -> Result<Value, Error> {
    Context::new().eval(source)
}
