use crate::{
    bytecode::{Chunk, Constant, Instruction, Pattern},
    compile,
};
use std::{cell::RefCell, collections::BTreeMap, fmt, rc::Rc};

const MAX_RANGE_ITEMS: i128 = 1_000_000;

#[derive(Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Number(f64),
    String(Rc<str>),
    Array(Rc<Vec<Value>>),
    Map(Rc<BTreeMap<String, Value>>),
    Function(Rc<Function>),
}
impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => write!(f, "nil"),
            Self::Bool(x) => write!(f, "{x}"),
            Self::Number(x) => write!(f, "{x}"),
            Self::String(x) => write!(f, "{x:?}"),
            Self::Array(x) => f.debug_list().entries(x.iter()).finish(),
            Self::Map(x) => f.debug_map().entries(x.iter()).finish(),
            Self::Function(_) => write!(f, "<function>"),
        }
    }
}
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nil => write!(f, "nil"),
            Self::Bool(x) => write!(f, "{x}"),
            Self::Number(x) => write!(f, "{x}"),
            Self::String(x) => write!(f, "{x}"),
            Self::Array(x) => {
                write!(f, "[")?;
                for (i, v) in x.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?
                    }
                    write!(f, "{v}")?
                }
                write!(f, "]")
            }
            Self::Map(x) => {
                write!(f, "{{")?;
                for (i, (k, v)) in x.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?
                    }
                    write!(f, "{k}: {v}")?
                }
                write!(f, "}}")
            }
            Self::Function(_) => write!(f, "<function>"),
        }
    }
}
impl Value {
    /// Builds a QuickCoffee string without exposing its `Rc<str>` storage.
    pub fn string(value: impl Into<Rc<str>>) -> Self {
        Self::String(value.into())
    }
    /// Builds an immutable QuickCoffee array from host values.
    pub fn array(values: impl Into<Vec<Value>>) -> Self {
        Self::Array(Rc::new(values.into()))
    }
    /// Builds an immutable QuickCoffee map from host key/value entries.
    pub fn map<I, K>(entries: I) -> Self
    where
        I: IntoIterator<Item = (K, Value)>,
        K: Into<String>,
    {
        Self::Map(Rc::new(
            entries
                .into_iter()
                .map(|(key, value)| (key.into(), value))
                .collect(),
        ))
    }
    pub fn as_number(&self) -> Option<f64> {
        if let Self::Number(x) = self {
            Some(*x)
        } else {
            None
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(x) = self {
            Some(*x)
        } else {
            None
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(x) = self {
            Some(x)
        } else {
            None
        }
    }
    pub fn as_array(&self) -> Option<&[Value]> {
        if let Self::Array(values) = self {
            Some(values)
        } else {
            None
        }
    }
    pub fn as_map(&self) -> Option<&BTreeMap<String, Value>> {
        if let Self::Map(values) = self {
            Some(values)
        } else {
            None
        }
    }
}
impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}
impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Self::Number(value)
    }
}
impl From<i64> for Value {
    fn from(value: i64) -> Self {
        Self::Number(value as f64)
    }
}
impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::string(value)
    }
}
impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::string(value)
    }
}
/// Stable category for an error crossing the Rust embedding boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Parse,
    Verify,
    Runtime,
}
/// One-based source line attached to a lexical or parse diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,
}
impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse => write!(f, "parse"),
            Self::Verify => write!(f, "verify"),
            Self::Runtime => write!(f, "runtime"),
        }
    }
}
#[derive(Debug, Clone)]
pub struct Error {
    kind: ErrorKind,
    message: String,
    position: Option<SourcePosition>,
}
impl Error {
    pub(crate) fn parse(m: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Parse,
            message: m.into(),
            position: None,
        }
    }
    pub(crate) fn verify(m: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Verify,
            message: m.into(),
            position: None,
        }
    }
    /// Creates a runtime error for a host callback to return across the VM boundary.
    pub fn runtime(m: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Runtime,
            message: m.into(),
            position: None,
        }
    }
    /// Returns the machine-readable category without requiring display-text parsing.
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
    /// Returns the human-readable detail without its category prefix.
    pub fn message(&self) -> &str {
        &self.message
    }
    /// Returns the one-based source line when the compiler knows it.
    pub fn position(&self) -> Option<SourcePosition> {
        self.position
    }
    pub(crate) fn at_line(mut self, line: usize) -> Self {
        self.position = Some(SourcePosition { line });
        self
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(position) = self.position {
            write!(
                f,
                "{} error (line {}): {}",
                self.kind, position.line, self.message
            )
        } else {
            write!(f, "{} error: {}", self.kind, self.message)
        }
    }
}
impl std::error::Error for Error {}

pub type NativeFunction = Rc<dyn Fn(&[Value]) -> Result<Value, Error>>;
/// Opaque callable values are constructed by QuickCoffee or `Context::add_native`.
pub struct Function {
    inner: FunctionKind,
}
enum FunctionKind {
    Bytecode {
        params: Vec<Pattern>,
        required: usize,
        rest: Option<String>,
        chunk: Rc<Chunk>,
        env: Env,
    },
    Native(NativeFunction),
}
type Env = Rc<RefCell<Environment>>;
struct Environment {
    values: BTreeMap<String, Value>,
    parent: Option<Env>,
}
fn env(parent: Option<Env>) -> Env {
    Rc::new(RefCell::new(Environment {
        values: BTreeMap::new(),
        parent,
    }))
}
fn lookup(e: &Env, n: &str) -> Option<Value> {
    let b = e.borrow();
    if let Some(v) = b.values.get(n) {
        return Some(v.clone());
    }
    let p = b.parent.clone();
    drop(b);
    p.and_then(|p| lookup(&p, n))
}

#[derive(Clone, Default)]
pub struct Engine;
/// A verified, reference-counted compiled program for repeated execution.
///
/// The shared storage is private so embedding callers do not need to manage
/// `Rc` themselves; cloning a `Program` is cheap and does not copy bytecode.
#[derive(Clone, Debug)]
pub struct Program(Rc<Chunk>);
impl From<Chunk> for Program {
    fn from(chunk: Chunk) -> Self {
        Self(Rc::new(chunk))
    }
}
impl Program {
    pub fn verify(&self) -> Result<(), Error> {
        self.0.verify()
    }
    pub fn disassemble(&self) -> String {
        self.0.disassemble()
    }
}
impl Engine {
    pub fn new() -> Self {
        Self
    }
    pub fn compile(&self, source: &str) -> Result<Chunk, Error> {
        compile(source)
    }
    /// Compiles source into cheaply cloneable shared bytecode.
    pub fn compile_program(&self, source: &str) -> Result<Program, Error> {
        Ok(self.compile(source)?.into())
    }
}
pub struct Context {
    engine: Engine,
    global: Env,
    fuel: u64,
}
impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
impl Context {
    pub fn new() -> Self {
        let global = env(None);
        let mut x = Self {
            engine: Engine::new(),
            global,
            fuel: 1_000_000,
        };
        x.install_builtins();
        x
    }
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }
    pub fn set_global(&mut self, name: impl Into<String>, value: Value) {
        self.global.borrow_mut().values.insert(name.into(), value);
    }
    /// Reads a global value without exposing the VM environment or running code.
    pub fn get_global(&self, name: &str) -> Option<Value> {
        lookup(&self.global, name)
    }
    pub fn add_native<F>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(&[Value]) -> Result<Value, Error> + 'static,
    {
        self.set_global(
            name,
            Value::Function(Rc::new(Function {
                inner: FunctionKind::Native(Rc::new(f)),
            })),
        );
    }
    pub fn eval(&mut self, source: &str) -> Result<Value, Error> {
        let program = self.engine.compile_program(source)?;
        self.run_program(&program)
    }
    pub fn run(&mut self, chunk: Chunk) -> Result<Value, Error> {
        self.run_program(&chunk.into())
    }
    /// Runs shared compiled bytecode without cloning its instruction stream.
    pub fn run_program(&mut self, program: &Program) -> Result<Value, Error> {
        program.verify()?;
        Vm { fuel: self.fuel }.run(Rc::clone(&program.0), self.global.clone())
    }
    fn install_builtins(&mut self) {
        self.add_native("print", |xs| {
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    print!(" ")
                }
                print!("{x}")
            }
            println!();
            Ok(Value::Nil)
        });
        self.add_native("len", |xs| {
            if xs.len() != 1 {
                return Err(Error::runtime("len expects one argument"));
            }
            let n = match &xs[0] {
                Value::String(x) => x.len(),
                Value::Array(x) => x.len(),
                Value::Map(x) => x.len(),
                _ => return Err(Error::runtime("len expects string, array, or map")),
            };
            Ok(Value::Number(n as f64))
        });
        self.add_native("type", |xs| {
            if xs.len() != 1 {
                return Err(Error::runtime("type expects one argument"));
            }
            let n = match xs[0] {
                Value::Nil => "nil",
                Value::Bool(_) => "bool",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Map(_) => "map",
                Value::Function(_) => "function",
            };
            Ok(Value::String(Rc::from(n)))
        });
        self.add_native("range", |xs| {
            if xs.len() != 2 {
                return Err(Error::runtime("range expects two arguments"));
            }
            let (a, b) = numbers(xs)?;
            numeric_range(a, b, false)
        });
        self.add_native("str", |xs| {
            if xs.len() != 1 {
                return Err(Error::runtime("str expects one argument"));
            }
            Ok(Value::String(Rc::from(xs[0].to_string())))
        });
        self.add_native("keys", |xs| {
            if xs.len() != 1 {
                return Err(Error::runtime("keys expects one argument"));
            }
            let Value::Map(map) = &xs[0] else {
                return Err(Error::runtime("keys expects a map"));
            };
            Ok(Value::Array(Rc::new(
                map.keys()
                    .map(|key| Value::String(Rc::from(key.as_str())))
                    .collect(),
            )))
        });
        self.add_native("values", |xs| {
            if xs.len() != 1 {
                return Err(Error::runtime("values expects one argument"));
            }
            let Value::Map(map) = &xs[0] else {
                return Err(Error::runtime("values expects a map"));
            };
            Ok(Value::Array(Rc::new(map.values().cloned().collect())))
        });
        self.add_native("join", |xs| {
            if xs.len() != 2 {
                return Err(Error::runtime("join expects array and separator"));
            }
            let Value::Array(values) = &xs[0] else {
                return Err(Error::runtime("join expects an array"));
            };
            let Value::String(separator) = &xs[1] else {
                return Err(Error::runtime("join separator must be string"));
            };
            Ok(Value::String(Rc::from(
                values
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(separator),
            )))
        });
        self.add_native("split", |xs| {
            if xs.len() != 2 {
                return Err(Error::runtime("split expects string and separator"));
            }
            let Value::String(input) = &xs[0] else {
                return Err(Error::runtime("split expects a string"));
            };
            let Value::String(separator) = &xs[1] else {
                return Err(Error::runtime("split separator must be string"));
            };
            Ok(Value::Array(Rc::new(
                input
                    .split(separator.as_ref())
                    .map(|part| Value::String(Rc::from(part)))
                    .collect(),
            )))
        });
        self.add_native("assert", |xs| {
            let Some(Value::Bool(condition)) = xs.first() else {
                return Err(Error::runtime("assert expects a boolean condition"));
            };
            if *condition {
                return Ok(Value::Nil);
            }
            let detail = match xs.get(1) {
                Some(Value::String(message)) => format!(": {message}"),
                Some(_) => return Err(Error::runtime("assert message must be string")),
                None => String::new(),
            };
            Err(Error::runtime(format!("assertion failed{detail}")))
        });
    }
}
struct Frame {
    chunk: Rc<Chunk>,
    pc: usize,
    stack: Vec<Value>,
    iterators: Vec<Iteration>,
    handlers: Vec<Handler>,
    env: Env,
}
struct Handler {
    catch_pc: usize,
    stack_depth: usize,
    iterator_depth: usize,
    name: String,
}
struct Iteration {
    kind: IterationKind,
}
enum IterationKind {
    Array {
        values: Rc<Vec<Value>>,
        position: usize,
        step: usize,
    },
    Map {
        entries: Vec<(String, Value)>,
        position: usize,
    },
}
struct Vm {
    fuel: u64,
}
enum Step {
    Continue,
    Return(Value),
    Call { callee: Value, args: Vec<Value> },
}
impl Vm {
    fn run(&mut self, chunk: Rc<Chunk>, global: Env) -> Result<Value, Error> {
        let mut frames = vec![Frame {
            chunk,
            pc: 0,
            stack: vec![],
            iterators: vec![],
            handlers: vec![],
            env: global,
        }];
        loop {
            if self.fuel == 0 {
                return Err(Error::runtime("execution fuel exhausted"));
            }
            self.fuel -= 1;
            let step = (|| -> Result<Step, Error> {
                let frame = frames.last_mut().expect("VM has an initial frame");
                let op = frame
                    .chunk
                    .code
                    .get(frame.pc)
                    .cloned()
                    .ok_or_else(|| Error::runtime("instruction pointer escaped chunk"))?;
                frame.pc += 1;
                match op {
                    Instruction::Constant(i) => match frame
                        .chunk
                        .constants
                        .get(i)
                        .ok_or_else(|| Error::runtime("invalid constant"))?
                    {
                        Constant::Value(v) => frame.stack.push(v.clone()),
                        _ => {
                            return Err(Error::runtime("function template used as value constant"));
                        }
                    },
                    Instruction::Load(n) => frame.stack.push(
                        lookup(&frame.env, &n)
                            .ok_or_else(|| Error::runtime(format!("unknown name '{n}'")))?,
                    ),
                    Instruction::LoadOrNil(n) => frame
                        .stack
                        .push(lookup(&frame.env, &n).unwrap_or(Value::Nil)),
                    Instruction::Store(n) => {
                        let v = pop(frame)?;
                        frame.env.borrow_mut().values.insert(n, v.clone());
                        frame.stack.push(v)
                    }
                    Instruction::Destructure(pattern) => {
                        let value = pop(frame)?;
                        let mut bindings = vec![];
                        bind_pattern(&pattern, &value, &mut bindings)?;
                        let mut environment = frame.env.borrow_mut();
                        for (name, item) in bindings {
                            if name != "_" {
                                environment.values.insert(name, item);
                            }
                        }
                        drop(environment);
                        frame.stack.push(value);
                    }
                    Instruction::Pop => {
                        pop(frame)?;
                    }
                    Instruction::Dup => {
                        let value = frame
                            .stack
                            .last()
                            .cloned()
                            .ok_or_else(|| Error::runtime("stack underflow"))?;
                        frame.stack.push(value);
                    }
                    Instruction::Swap => {
                        if frame.stack.len() < 2 {
                            return Err(Error::runtime("stack underflow"));
                        }
                        let last = frame.stack.len() - 1;
                        frame.stack.swap(last, last - 1);
                    }
                    Instruction::Rotate3 => {
                        if frame.stack.len() < 3 {
                            return Err(Error::runtime("stack underflow"));
                        }
                        let start = frame.stack.len() - 3;
                        frame.stack[start..].rotate_left(1);
                    }
                    Instruction::Neg => {
                        let x = number(pop(frame)?)?;
                        frame.stack.push(Value::Number(-x))
                    }
                    Instruction::Not => {
                        let x = truth(pop(frame)?)?;
                        frame.stack.push(Value::Bool(!x))
                    }
                    Instruction::BitNot => {
                        let x = bit_integer(pop(frame)?)?;
                        frame.stack.push(Value::Number((!x) as f64));
                    }
                    Instruction::Exists => {
                        let value = pop(frame)?;
                        frame.stack.push(Value::Bool(!matches!(value, Value::Nil)))
                    }
                    Instruction::Add => binary(frame, |a, b| Value::Number(a + b))?,
                    Instruction::Sub => binary(frame, |a, b| Value::Number(a - b))?,
                    Instruction::Mul => binary(frame, |a, b| Value::Number(a * b))?,
                    Instruction::Div => binary(frame, |a, b| Value::Number(a / b))?,
                    Instruction::FloorDiv => binary(frame, |a, b| Value::Number((a / b).floor()))?,
                    Instruction::Rem => binary(frame, |a, b| Value::Number(a % b))?,
                    Instruction::Modulo => binary(frame, |a, b| Value::Number((a % b + b) % b))?,
                    Instruction::BitAnd => bit_binary(frame, |a, b| a & b)?,
                    Instruction::BitOr => bit_binary(frame, |a, b| a | b)?,
                    Instruction::BitXor => bit_binary(frame, |a, b| a ^ b)?,
                    Instruction::ShiftLeft => bit_shift(frame, |a, b| a.wrapping_shl(b))?,
                    Instruction::ShiftRight => bit_shift(frame, |a, b| a.wrapping_shr(b))?,
                    Instruction::ShiftRightUnsigned => {
                        bit_shift(frame, |a, b| ((a as u32).wrapping_shr(b)) as i32)?
                    }
                    Instruction::Pow => binary(frame, |a, b| Value::Number(a.powf(b)))?,
                    Instruction::Eq => compare(frame, equal)?,
                    Instruction::Ne => compare(frame, |a, b| !equal(a, b))?,
                    Instruction::Lt => order(frame, |a, b| a < b)?,
                    Instruction::Le => order(frame, |a, b| a <= b)?,
                    Instruction::Gt => order(frame, |a, b| a > b)?,
                    Instruction::Ge => order(frame, |a, b| a >= b)?,
                    Instruction::Contains => {
                        let target = pop(frame)?;
                        let needle = pop(frame)?;
                        let Value::Array(values) = target else {
                            return Err(Error::runtime("right side of 'in' must be an array"));
                        };
                        frame.stack.push(Value::Bool(
                            values.iter().any(|value| equal(&needle, value)),
                        ));
                    }
                    Instruction::HasKey => {
                        let target = pop(frame)?;
                        let key = pop(frame)?;
                        let (Value::String(key), Value::Map(values)) = (key, target) else {
                            return Err(Error::runtime("'of' expects a string key and a map"));
                        };
                        frame
                            .stack
                            .push(Value::Bool(values.contains_key(key.as_ref())));
                    }
                    Instruction::Jump(delta) => jump(frame, delta)?,
                    Instruction::JumpIfFalse(delta) => {
                        if !truth(
                            frame
                                .stack
                                .last()
                                .cloned()
                                .ok_or_else(|| Error::runtime("stack underflow"))?,
                        )? {
                            jump(frame, delta)?
                        }
                    }
                    Instruction::JumpIfNil(delta) => {
                        if matches!(frame.stack.last(), Some(Value::Nil)) {
                            jump(frame, delta)?
                        }
                    }
                    Instruction::Try { catch, name } => frame.handlers.push(Handler {
                        catch_pc: (frame.pc as i64 + catch as i64)
                            .try_into()
                            .map_err(|_| Error::runtime("invalid catch target"))?,
                        stack_depth: frame.stack.len(),
                        iterator_depth: frame.iterators.len(),
                        name,
                    }),
                    Instruction::EndTry => {
                        frame
                            .handlers
                            .pop()
                            .ok_or_else(|| Error::runtime("handler stack underflow"))?;
                    }
                    Instruction::Throw => {
                        let value = pop(frame)?;
                        return Err(Error::runtime(format!("thrown: {value}")));
                    }
                    Instruction::IterStartArray => {
                        let step = array_iteration_step(pop(frame)?)?;
                        match pop(frame)? {
                            Value::Array(values) => frame.iterators.push(Iteration {
                                kind: IterationKind::Array {
                                    values,
                                    position: 0,
                                    step,
                                },
                            }),
                            _ => return Err(Error::runtime("for expects an array iterable")),
                        }
                    }
                    Instruction::IterStartMap => match pop(frame)? {
                        Value::Map(map) => frame.iterators.push(Iteration {
                            kind: IterationKind::Map {
                                entries: map
                                    .iter()
                                    .map(|(key, value)| (key.clone(), value.clone()))
                                    .collect(),
                                position: 0,
                            },
                        }),
                        _ => return Err(Error::runtime("for of expects a map iterable")),
                    },
                    Instruction::IterNext { patterns, end } => {
                        let next = {
                            let iterator = frame
                                .iterators
                                .last_mut()
                                .ok_or_else(|| Error::runtime("iterator stack underflow"))?;
                            match &mut iterator.kind {
                                IterationKind::Array {
                                    values,
                                    position,
                                    step,
                                } => {
                                    let value = values.get(*position).cloned().map(|value| {
                                        if patterns.len() == 2 {
                                            vec![value, Value::Number(*position as f64)]
                                        } else {
                                            vec![value]
                                        }
                                    });
                                    if value.is_some() {
                                        *position = position.saturating_add(*step);
                                    }
                                    value
                                }
                                IterationKind::Map { entries, position } => {
                                    let value = entries.get(*position).map(|(key, value)| {
                                        vec![Value::String(Rc::from(key.as_str())), value.clone()]
                                    });
                                    if value.is_some() {
                                        *position += 1;
                                    }
                                    value
                                }
                            }
                        };
                        if let Some(values) = next {
                            if patterns.len() != values.len() {
                                return Err(Error::runtime("iterator binding arity mismatch"));
                            }
                            if patterns.iter().all(|pattern| {
                                matches!(pattern, Pattern::Bind(_) | Pattern::Ignore)
                            }) {
                                let mut environment = frame.env.borrow_mut();
                                for (pattern, value) in patterns.into_iter().zip(values) {
                                    if let Pattern::Bind(name) = pattern {
                                        environment.values.insert(name, value);
                                    }
                                }
                            } else {
                                let mut bindings = vec![];
                                for (pattern, value) in patterns.iter().zip(values.iter()) {
                                    bind_pattern(pattern, value, &mut bindings)?;
                                }
                                let mut environment = frame.env.borrow_mut();
                                for (name, value) in bindings {
                                    environment.values.insert(name, value);
                                }
                            }
                        } else {
                            frame.iterators.pop();
                            jump(frame, end)?;
                        }
                    }
                    Instruction::IterEnd => {
                        frame
                            .iterators
                            .pop()
                            .ok_or_else(|| Error::runtime("iterator stack underflow"))?;
                    }
                    Instruction::MakeArray(n) => {
                        let v = take(frame, n)?;
                        frame.stack.push(Value::Array(Rc::new(v)))
                    }
                    Instruction::Append => {
                        let value = pop(frame)?;
                        let Value::Array(mut values) = pop(frame)? else {
                            return Err(Error::runtime("append expects an array"));
                        };
                        Rc::make_mut(&mut values).push(value);
                        frame.stack.push(Value::Array(values));
                    }
                    Instruction::MergeArrays(n) => {
                        let segments = take(frame, n)?;
                        let mut values = vec![];
                        for segment in segments {
                            let Value::Array(segment) = segment else {
                                return Err(Error::runtime("splat expects an array"));
                            };
                            values.extend(segment.iter().cloned());
                        }
                        frame.stack.push(Value::Array(Rc::new(values)));
                    }
                    Instruction::MakeRange(inclusive) => {
                        let end = pop(frame)?;
                        let start = pop(frame)?;
                        let (Value::Number(start), Value::Number(end)) = (start, end) else {
                            return Err(Error::runtime("range bounds must be numbers"));
                        };
                        frame.stack.push(numeric_range(start, end, inclusive)?);
                    }
                    Instruction::MakeMap(keys) => {
                        let v = take(frame, keys.len())?;
                        frame
                            .stack
                            .push(Value::Map(Rc::new(keys.into_iter().zip(v).collect())))
                    }
                    Instruction::Stringify => {
                        let value = pop(frame)?;
                        frame.stack.push(Value::String(Rc::from(value.to_string())));
                    }
                    Instruction::Concat(n) => {
                        let values = take(frame, n)?;
                        let mut output = String::new();
                        for value in values {
                            let Value::String(value) = value else {
                                return Err(Error::runtime("concat received non-string"));
                            };
                            output.push_str(&value);
                        }
                        frame.stack.push(Value::String(Rc::from(output)));
                    }
                    Instruction::Index => {
                        let key = pop(frame)?;
                        let target = pop(frame)?;
                        frame.stack.push(index(target, key)?)
                    }
                    Instruction::Slice(inclusive) => {
                        let end = pop(frame)?;
                        let start = pop(frame)?;
                        let target = pop(frame)?;
                        frame.stack.push(slice(target, start, end, inclusive)?)
                    }
                    Instruction::Member(name) => match pop(frame)? {
                        Value::Map(map) => {
                            frame.stack.push(map.get(&name).cloned().ok_or_else(|| {
                                Error::runtime(format!("map key '{name}' not found"))
                            })?)
                        }
                        _ => return Err(Error::runtime("member access expects a map")),
                    },
                    Instruction::MakeFunction(i) => match frame
                        .chunk
                        .constants
                        .get(i)
                        .ok_or_else(|| Error::runtime("invalid function template"))?
                    {
                        Constant::Function {
                            params,
                            required,
                            rest,
                            chunk,
                        } => frame.stack.push(Value::Function(Rc::new(Function {
                            inner: FunctionKind::Bytecode {
                                params: params.clone(),
                                required: *required,
                                rest: rest.clone(),
                                chunk: chunk.clone(),
                                env: frame.env.clone(),
                            },
                        }))),
                        _ => return Err(Error::runtime("value used as function template")),
                    },
                    Instruction::Call(n) => {
                        let args = take(frame, n)?;
                        let callee = pop(frame)?;
                        return Ok(Step::Call { callee, args });
                    }
                    Instruction::CallSpread => {
                        let args = pop(frame)?;
                        let Value::Array(args) = args else {
                            return Err(Error::runtime("splat call expects an array"));
                        };
                        let callee = pop(frame)?;
                        return Ok(Step::Call {
                            callee,
                            args: args.as_ref().clone(),
                        });
                    }
                    Instruction::Return => {
                        if !frame.handlers.is_empty() {
                            return Err(Error::runtime("handler leaked at Return"));
                        }
                        let v = pop(frame)?;
                        return Ok(Step::Return(v));
                    }
                }
                Ok(Step::Continue)
            })();
            match step {
                Ok(Step::Continue) => {}
                Ok(Step::Return(value)) => {
                    frames.pop();
                    if let Some(parent) = frames.last_mut() {
                        parent.stack.push(value);
                    } else {
                        return Ok(value);
                    }
                }
                Ok(Step::Call { callee, args }) => {
                    if let Err(error) = call(&mut frames, callee, args)
                        && !handle_error(&mut frames, &error)
                    {
                        return Err(error);
                    }
                }
                Err(error) => {
                    if !handle_error(&mut frames, &error) {
                        return Err(error);
                    }
                }
            }
        }
    }
}
fn call(frames: &mut Vec<Frame>, callee: Value, args: Vec<Value>) -> Result<(), Error> {
    match callee {
        Value::Function(function) => match &function.inner {
            FunctionKind::Native(function) => {
                let value = function(&args)?;
                frames
                    .last_mut()
                    .expect("call has a caller frame")
                    .stack
                    .push(value);
            }
            FunctionKind::Bytecode {
                params,
                required,
                rest,
                chunk,
                env: captured,
            } => {
                if args.len() < *required || (rest.is_none() && args.len() > params.len()) {
                    return Err(Error::runtime(format!(
                        "expected {}{} arguments, got {}",
                        required,
                        if rest.is_some() { " or more" } else { "" },
                        args.len()
                    )));
                }
                let local = env(Some(captured.clone()));
                for (index, pattern) in params.iter().enumerate() {
                    let value = args.get(index).cloned().unwrap_or(Value::Nil);
                    let mut bindings = vec![];
                    bind_pattern(pattern, &value, &mut bindings)?;
                    let mut environment = local.borrow_mut();
                    for (key, value) in bindings {
                        environment.values.insert(key, value);
                    }
                }
                if let Some(rest) = rest {
                    local.borrow_mut().values.insert(
                        rest.clone(),
                        Value::Array(Rc::new(args[params.len()..].to_vec())),
                    );
                }
                frames.push(Frame {
                    chunk: chunk.clone(),
                    pc: 0,
                    stack: vec![],
                    iterators: vec![],
                    handlers: vec![],
                    env: local,
                });
            }
        },
        _ => return Err(Error::runtime("attempted to call a non-function")),
    }
    Ok(())
}
fn handle_error(frames: &mut Vec<Frame>, error: &Error) -> bool {
    loop {
        let Some(frame) = frames.last_mut() else {
            return false;
        };
        if let Some(handler) = frame.handlers.pop() {
            frame.stack.truncate(handler.stack_depth);
            frame.iterators.truncate(handler.iterator_depth);
            frame
                .env
                .borrow_mut()
                .values
                .insert(handler.name, Value::String(Rc::from(error.to_string())));
            frame.pc = handler.catch_pc;
            return true;
        }
        frames.pop();
    }
}
fn pop(f: &mut Frame) -> Result<Value, Error> {
    f.stack
        .pop()
        .ok_or_else(|| Error::runtime("stack underflow"))
}
fn take(f: &mut Frame, n: usize) -> Result<Vec<Value>, Error> {
    if f.stack.len() < n {
        return Err(Error::runtime("stack underflow"));
    }
    Ok(f.stack.split_off(f.stack.len() - n))
}
fn number(v: Value) -> Result<f64, Error> {
    v.as_number()
        .ok_or_else(|| Error::runtime("expected number"))
}
fn numbers(xs: &[Value]) -> Result<(f64, f64), Error> {
    if xs.len() != 2 {
        return Err(Error::runtime("expected two numbers"));
    }
    Ok((number(xs[0].clone())?, number(xs[1].clone())?))
}
fn numeric_range(start: f64, end: f64, inclusive: bool) -> Result<Value, Error> {
    if !start.is_finite()
        || !end.is_finite()
        || start.fract() != 0.
        || end.fract() != 0.
        || start < i64::MIN as f64
        || start > i64::MAX as f64
        || end < i64::MIN as f64
        || end > i64::MAX as f64
    {
        return Err(Error::runtime("range bounds must be finite integers"));
    }
    let start = start as i64;
    let end = end as i64;
    let end = if inclusive {
        end.checked_add(1)
            .ok_or_else(|| Error::runtime("inclusive range end is too large"))?
    } else {
        end
    };
    let count = (i128::from(end) - i128::from(start)).max(0);
    if count > MAX_RANGE_ITEMS {
        return Err(Error::runtime("range is too large"));
    }
    Ok(Value::Array(Rc::new(
        (start..end)
            .map(|value| Value::Number(value as f64))
            .collect(),
    )))
}
fn array_iteration_step(value: Value) -> Result<usize, Error> {
    let Value::Number(step) = value else {
        return Err(Error::runtime("for by step must be a positive integer"));
    };
    if !step.is_finite() || step.fract() != 0. || step < 1. || step > usize::MAX as f64 {
        return Err(Error::runtime("for by step must be a positive integer"));
    }
    Ok(step as usize)
}
fn truth(v: Value) -> Result<bool, Error> {
    v.as_bool()
        .ok_or_else(|| Error::runtime("condition must be bool"))
}
fn binary(f: &mut Frame, op: impl Fn(f64, f64) -> Value) -> Result<(), Error> {
    let (a, b) = numbers(&[pop(f)?, pop(f)?])?;
    f.stack.push(op(b, a));
    Ok(())
}
fn bit_integer(value: Value) -> Result<i32, Error> {
    let value = number(value)?;
    if !value.is_finite()
        || value.fract() != 0.
        || !(-2_147_483_648.0..=2_147_483_647.0).contains(&value)
    {
        return Err(Error::runtime(
            "bitwise operands must be finite 32-bit integers",
        ));
    }
    Ok(value as i32)
}
fn bit_binary(f: &mut Frame, op: impl Fn(i32, i32) -> i32) -> Result<(), Error> {
    let b = bit_integer(pop(f)?)?;
    let a = bit_integer(pop(f)?)?;
    f.stack.push(Value::Number(op(a, b) as f64));
    Ok(())
}
fn bit_shift(f: &mut Frame, op: impl Fn(i32, u32) -> i32) -> Result<(), Error> {
    let shift = bit_integer(pop(f)?)?;
    if !(0..32).contains(&shift) {
        return Err(Error::runtime(
            "shift count must be an integer from 0 to 31",
        ));
    }
    let value = bit_integer(pop(f)?)?;
    f.stack.push(Value::Number(op(value, shift as u32) as f64));
    Ok(())
}
fn compare(f: &mut Frame, op: impl Fn(&Value, &Value) -> bool) -> Result<(), Error> {
    let b = pop(f)?;
    let a = pop(f)?;
    f.stack.push(Value::Bool(op(&a, &b)));
    Ok(())
}
fn order(f: &mut Frame, op: impl Fn(f64, f64) -> bool) -> Result<(), Error> {
    let (a, b) = numbers(&[pop(f)?, pop(f)?])?;
    f.stack.push(Value::Bool(op(b, a)));
    Ok(())
}
fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Nil, Value::Nil) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| equal(a, b))
        }
        (Value::Map(x), Value::Map(y)) => {
            x.len() == y.len() && x.iter().all(|(k, a)| y.get(k).is_some_and(|b| equal(a, b)))
        }
        _ => false,
    }
}
fn jump(f: &mut Frame, delta: i32) -> Result<(), Error> {
    f.pc = (f.pc as i64 + delta as i64)
        .try_into()
        .map_err(|_| Error::runtime("invalid jump"))?;
    Ok(())
}
fn bind_pattern(
    pattern: &Pattern,
    value: &Value,
    bindings: &mut Vec<(String, Value)>,
) -> Result<(), Error> {
    match pattern {
        Pattern::Ignore => Ok(()),
        Pattern::Bind(name) => {
            bindings.push((name.clone(), value.clone()));
            Ok(())
        }
        Pattern::Array(patterns) => {
            let Value::Array(values) = value else {
                return Err(Error::runtime("array destructuring expects an array"));
            };
            if values.len() != patterns.len() {
                return Err(Error::runtime(format!(
                    "array destructuring expected {} values, got {}",
                    patterns.len(),
                    values.len()
                )));
            }
            for (pattern, value) in patterns.iter().zip(values.iter()) {
                bind_pattern(pattern, value, bindings)?;
            }
            Ok(())
        }
        Pattern::Map(fields) => {
            let Value::Map(map) = value else {
                return Err(Error::runtime("map destructuring expects a map"));
            };
            for (key, pattern) in fields {
                let value = map
                    .get(key)
                    .ok_or_else(|| Error::runtime(format!("map key '{key}' not found")))?;
                bind_pattern(pattern, value, bindings)?;
            }
            Ok(())
        }
    }
}
fn index(target: Value, key: Value) -> Result<Value, Error> {
    match (target, key) {
        (Value::Array(xs), Value::Number(i)) if i >= 0. && i.fract() == 0. => xs
            .get(i as usize)
            .cloned()
            .ok_or_else(|| Error::runtime("array index out of range")),
        (Value::Map(m), Value::String(k)) => m
            .get(k.as_ref())
            .cloned()
            .ok_or_else(|| Error::runtime("map key not found")),
        _ => Err(Error::runtime("invalid index operation")),
    }
}
fn slice(target: Value, start: Value, end: Value, inclusive: bool) -> Result<Value, Error> {
    let Value::Array(values) = target else {
        return Err(Error::runtime("slice expects an array"));
    };
    let start = slice_bound(start, values.len(), "slice start")?;
    let mut end = slice_bound(end, values.len(), "slice end")?;
    if inclusive {
        end = end
            .checked_add(1)
            .ok_or_else(|| Error::runtime("inclusive slice end is too large"))?;
    }
    if start > end || end > values.len() {
        return Err(Error::runtime("slice bounds out of range"));
    }
    Ok(Value::Array(Rc::new(values[start..end].to_vec())))
}
fn slice_bound(value: Value, len: usize, name: &str) -> Result<usize, Error> {
    let Value::Number(value) = value else {
        return Err(Error::runtime(format!("{name} must be a finite integer")));
    };
    if !value.is_finite() || value.fract() != 0. || value < -(len as f64) || value > len as f64 {
        return Err(Error::runtime(format!("{name} out of range")));
    }
    let value = value as i64;
    let index = if value < 0 { len as i64 + value } else { value };
    usize::try_from(index).map_err(|_| Error::runtime(format!("{name} out of range")))
}
