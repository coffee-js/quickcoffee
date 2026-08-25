use crate::vm::{Error, Value};
use std::{
    collections::{BTreeSet, VecDeque},
    fmt::Write,
    rc::Rc,
};

pub(crate) const RECEIVER_NAME: &str = "\0quickcoffee.receiver";

/// A strict, recursively shaped binding pattern used by destructuring assignment.
#[allow(missing_docs)]
#[derive(Clone, Debug)]
pub enum Pattern {
    Ignore,
    Bind(String),
    Rest(String),
    Default {
        pattern: Box<Pattern>,
        default: Rc<Chunk>,
    },
    Array(Vec<Pattern>),
    Map(Vec<(String, Pattern)>),
    MapRest {
        fields: Vec<(String, Pattern)>,
        rest: String,
    },
}

/// Verified bytecode consisting of a constant pool and instruction stream.
#[derive(Clone, Debug, Default)]
pub struct Chunk {
    /// Values and nested functions referenced by instructions.
    pub constants: Vec<Constant>,
    /// Instructions executed by the VM.
    pub code: Vec<Instruction>,
}
/// A value or nested function stored in a [`Chunk`] constant pool.
#[allow(missing_docs)]
#[derive(Clone, Debug)]
pub enum Constant {
    Value(Value),
    Function {
        params: Vec<Pattern>,
        required: usize,
        rest: Option<String>,
        receiver: bool,
        receiver_bound: bool,
        chunk: Rc<Chunk>,
    },
}
/// The public low-level instruction set accepted by [`Chunk::verify`].
#[allow(missing_docs)]
#[derive(Clone, Debug)]
pub enum Instruction {
    Constant(usize),
    Load(String),
    LoadOrNil(String),
    Store(String),
    Destructure(Pattern),
    Pop,
    Dup,
    Swap,
    Rotate3,
    Neg,
    Not,
    BitNot,
    Exists,
    Increment,
    Decrement,
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Rem,
    Modulo,
    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,
    ShiftRightUnsigned,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
    HasKey,
    Jump(i32),
    JumpIfFalse(i32),
    JumpIfNil(i32),
    Try {
        catch: i32,
        name: String,
    },
    EndTry,
    Throw,
    /// Starts an `in` iterator over an array or string, consuming iterable and step.
    IterStartEnumerable,
    IterStartMap,
    IterNext {
        patterns: Vec<Pattern>,
        end: i32,
    },
    IterEnd,
    MakeArray(usize),
    Append,
    MergeArrays(usize),
    MergeMaps(usize),
    MakeRange(bool),
    MakeMap(Vec<String>),
    Stringify,
    Concat(usize),
    Index,
    Slice(bool),
    Member(String),
    SetMember(String),
    MemberCall {
        name: String,
        count: usize,
    },
    MemberCallSpread(String),
    Call(usize),
    CallSpread,
    Construct(usize),
    ConstructSpread,
    MakeFunction(usize),
    MakeClass {
        name: String,
        extends: bool,
        constructor: bool,
        instance_methods: Vec<String>,
        static_methods: Vec<String>,
    },
    Return,
    LoadReceiver,
    SuperCall(usize),
    SuperCallSpread,
    MakeBoundFunction(usize),
}

impl Chunk {
    /// Returns a human-readable instruction listing with stable program counters.
    pub fn disassemble(&self) -> String {
        let mut output = String::new();
        for (i, op) in self.code.iter().enumerate() {
            writeln!(&mut output, "{i:04} {op:?}").expect("writing to a String cannot fail");
        }
        output
    }
    /// Returns a deterministic content fingerprint for cache keys and diagnostics.
    ///
    /// The encoding is explicit rather than based on `Debug` formatting, so a
    /// Rust toolchain changing its human-readable representation cannot silently
    /// invalidate cache keys.
    pub fn fingerprint(&self) -> u64 {
        let mut encoder = FingerprintEncoder::new();
        encoder.chunk(self);
        encoder.finish()
    }
    /// Verifies stack/control-flow safety before execution.
    pub fn verify(&self) -> Result<(), Error> {
        self.verify_inner(false)
            .map_err(|error| error.with_verification_chunk(self as *const Self as usize))
    }
    fn verify_inner(&self, receiver_context: bool) -> Result<(), Error> {
        if self.code.is_empty() {
            return Err(Error::verify("chunk is empty"));
        }
        if !matches!(self.code.last(), Some(Instruction::Return)) {
            return Err(
                Error::verify("chunk does not end in Return").at_instruction(self.code.len() - 1)
            );
        }
        for (pc, op) in self.code.iter().enumerate() {
            match op {
                Instruction::Constant(i) => match self.constants.get(*i) {
                    Some(Constant::Value(_)) => {}
                    Some(_) => {
                        return Err(Error::verify(format!(
                            "constant {i} at instruction {pc} is not a value"
                        ))
                        .at_instruction(pc));
                    }
                    None => {
                        return Err(Error::verify(format!(
                            "constant {i} at instruction {pc} is out of bounds"
                        ))
                        .at_instruction(pc));
                    }
                },
                Instruction::MakeFunction(i) | Instruction::MakeBoundFunction(i) => match self
                    .constants
                    .get(*i)
                {
                    Some(Constant::Function {
                        chunk,
                        params,
                        required,
                        receiver,
                        receiver_bound,
                        ..
                    }) => {
                        params
                            .iter()
                            .try_for_each(validate_pattern)
                            .map_err(|error| error.at_instruction(pc))?;
                        chunk.verify_inner(*receiver).map_err(|error| {
                            error.with_verification_chunk(Rc::as_ptr(chunk) as usize)
                        })?;
                        let captures_receiver = matches!(op, Instruction::MakeBoundFunction(_));
                        if captures_receiver && !receiver_context {
                            return Err(Error::verify(format!(
                                "bound function creation outside class receiver context at instruction {pc}"
                            ))
                            .at_instruction(pc));
                        }
                        if *receiver {
                            if *required == 0
                                || !matches!(
                                    params.first(),
                                    Some(Pattern::Bind(name)) if name == RECEIVER_NAME
                                )
                            {
                                return Err(Error::verify(format!(
                                    "receiver function at instruction {pc} has an invalid hidden receiver parameter"
                                ))
                                .at_instruction(pc));
                            }
                            if captures_receiver {
                                if !receiver_bound {
                                    return Err(Error::verify(format!(
                                        "bound function at instruction {pc} is not marked receiver-bound"
                                    ))
                                    .at_instruction(pc));
                                }
                                if chunk.code.iter().any(|instruction| {
                                    matches!(
                                        instruction,
                                        Instruction::SuperCall(_) | Instruction::SuperCallSpread
                                    )
                                }) {
                                    return Err(Error::verify(format!(
                                        "bound function at instruction {pc} cannot capture super context"
                                    ))
                                    .at_instruction(pc));
                                }
                            } else {
                                let mut start = pc;
                                while start > 0
                                    && matches!(self.code[start - 1], Instruction::MakeFunction(_))
                                {
                                    start -= 1;
                                }
                                let mut end = pc + 1;
                                while matches!(
                                    self.code.get(end),
                                    Some(Instruction::MakeFunction(_))
                                ) {
                                    end += 1;
                                }
                                let valid = matches!(
                                    self.code.get(end),
                                    Some(Instruction::MakeClass {
                                        constructor,
                                        instance_methods,
                                        static_methods,
                                        ..
                                    }) if end - start
                                        == usize::from(*constructor)
                                            + instance_methods.len()
                                            + static_methods.len()
                                );
                                if !valid {
                                    return Err(Error::verify(format!(
                                        "receiver function at instruction {pc} is not confined to a class template"
                                    ))
                                    .at_instruction(pc));
                                }
                            }
                        } else if *receiver_bound || captures_receiver {
                            return Err(Error::verify(format!(
                                "bound function at instruction {pc} has no hidden receiver"
                            ))
                            .at_instruction(pc));
                        }
                    }
                    Some(_) => {
                        return Err(Error::verify(format!(
                            "constant {i} at instruction {pc} is not a function"
                        ))
                        .at_instruction(pc));
                    }
                    None => {
                        return Err(Error::verify(format!(
                            "constant {i} at instruction {pc} is out of bounds"
                        ))
                        .at_instruction(pc));
                    }
                },
                Instruction::Destructure(pattern) => {
                    validate_pattern(pattern).map_err(|error| error.at_instruction(pc))?
                }
                Instruction::SetMember(_) if !receiver_context => {
                    return Err(Error::verify(format!(
                        "receiver field write outside class member at instruction {pc}"
                    ))
                    .at_instruction(pc));
                }
                Instruction::LoadReceiver if !receiver_context => {
                    return Err(Error::verify(format!(
                        "receiver load outside class member at instruction {pc}"
                    ))
                    .at_instruction(pc));
                }
                Instruction::SuperCall(_) | Instruction::SuperCallSpread if !receiver_context => {
                    return Err(Error::verify(format!(
                        "super call outside class member at instruction {pc}"
                    ))
                    .at_instruction(pc));
                }
                Instruction::MakeClass {
                    extends,
                    constructor,
                    instance_methods,
                    static_methods,
                    ..
                } => {
                    let count =
                        usize::from(*constructor) + instance_methods.len() + static_methods.len();
                    if pc < count {
                        return Err(Error::verify(format!(
                            "class template at instruction {pc} is missing member functions"
                        ))
                        .at_instruction(pc));
                    }
                    let mut instance_names = BTreeSet::new();
                    let mut static_names = BTreeSet::new();
                    if instance_methods
                        .iter()
                        .any(|name| name.is_empty() || !instance_names.insert(name))
                        || static_methods
                            .iter()
                            .any(|name| name.is_empty() || !static_names.insert(name))
                    {
                        return Err(Error::verify(format!(
                            "class template at instruction {pc} has invalid or duplicate members"
                        ))
                        .at_instruction(pc));
                    }
                    for instruction in &self.code[pc - count..pc] {
                        let Instruction::MakeFunction(index) = instruction else {
                            return Err(Error::verify(format!(
                                "class template at instruction {pc} is not preceded by member functions"
                            ))
                            .at_instruction(pc));
                        };
                        match self.constants.get(*index) {
                            Some(Constant::Function {
                                receiver: true,
                                chunk,
                                ..
                            }) => {
                                if !extends
                                    && chunk.code.iter().any(|instruction| {
                                        matches!(
                                            instruction,
                                            Instruction::SuperCall(_)
                                                | Instruction::SuperCallSpread
                                        )
                                    })
                                {
                                    return Err(Error::verify(format!(
                                        "base class template at instruction {pc} contains a super call"
                                    ))
                                    .at_instruction(pc));
                                }
                            }
                            _ => {
                                return Err(Error::verify(format!(
                                    "class template at instruction {pc} uses a non-receiver function"
                                ))
                                .at_instruction(pc));
                            }
                        }
                    }
                }
                Instruction::Jump(offset)
                | Instruction::JumpIfFalse(offset)
                | Instruction::JumpIfNil(offset)
                | Instruction::IterNext { end: offset, .. } => {
                    if let Instruction::IterNext { patterns, .. } = op {
                        patterns
                            .iter()
                            .try_for_each(validate_pattern)
                            .map_err(|error| error.at_instruction(pc))?;
                    }
                    let target = pc as i64 + 1 + *offset as i64;
                    if target < 0 || target >= self.code.len() as i64 {
                        return Err(Error::verify(format!(
                            "jump at instruction {pc} leaves chunk"
                        ))
                        .at_instruction(pc));
                    }
                }
                Instruction::Try { catch, .. } => {
                    let target = pc as i64 + 1 + *catch as i64;
                    if target < 0 || target >= self.code.len() as i64 {
                        return Err(Error::verify(format!(
                            "catch target at instruction {pc} leaves chunk"
                        ))
                        .at_instruction(pc));
                    }
                }
                _ => {}
            }
        }
        self.verify_control_flow()?;
        Ok(())
    }
    fn verify_control_flow(&self) -> Result<(), Error> {
        type State = (i32, i32, i32); // (value stack, iterator stack, handler stack)
        let mut states: Vec<Option<State>> = vec![None; self.code.len()];
        let mut work = VecDeque::from([0usize]);
        states[0] = Some((0, 0, 0));
        while let Some(pc) = work.pop_front() {
            let state = states[pc].expect("queued verifier state");
            let require = |count: i32| {
                if state.0 < count {
                    Err(
                        Error::verify(format!("stack underflow at instruction {pc}"))
                            .at_instruction(pc),
                    )
                } else {
                    Ok(())
                }
            };
            let mut next = |target: usize, successor: State| -> Result<(), Error> {
                match states[target] {
                    Some(existing) if existing != successor => Err(Error::verify(format!(
                        "inconsistent stack state at instruction {target}"
                    ))
                    .at_instruction(target)),
                    Some(_) => Ok(()),
                    None => {
                        states[target] = Some(successor);
                        work.push_back(target);
                        Ok(())
                    }
                }
            };
            let fallthrough = pc + 1;
            match &self.code[pc] {
                Instruction::Constant(_)
                | Instruction::Load(_)
                | Instruction::LoadOrNil(_)
                | Instruction::LoadReceiver
                | Instruction::MakeFunction(_)
                | Instruction::MakeBoundFunction(_) => {
                    next(fallthrough, (state.0 + 1, state.1, state.2))?;
                }
                Instruction::Dup => {
                    require(1)?;
                    next(fallthrough, (state.0 + 1, state.1, state.2))?;
                }
                Instruction::Swap => {
                    require(2)?;
                    next(fallthrough, state)?;
                }
                Instruction::Rotate3 => {
                    require(3)?;
                    next(fallthrough, state)?;
                }
                Instruction::Store(_)
                | Instruction::Destructure(_)
                | Instruction::Neg
                | Instruction::Not
                | Instruction::BitNot
                | Instruction::Exists
                | Instruction::Increment
                | Instruction::Decrement
                | Instruction::Stringify
                | Instruction::Member(_) => {
                    require(1)?;
                    next(fallthrough, state)?;
                }
                Instruction::SetMember(_) => {
                    require(1)?;
                    next(fallthrough, state)?;
                }
                Instruction::Pop => {
                    require(1)?;
                    next(fallthrough, (state.0 - 1, state.1, state.2))?;
                }
                Instruction::Add
                | Instruction::Sub
                | Instruction::Mul
                | Instruction::Div
                | Instruction::FloorDiv
                | Instruction::Rem
                | Instruction::Modulo
                | Instruction::BitAnd
                | Instruction::BitOr
                | Instruction::BitXor
                | Instruction::ShiftLeft
                | Instruction::ShiftRight
                | Instruction::ShiftRightUnsigned
                | Instruction::Pow
                | Instruction::Eq
                | Instruction::Ne
                | Instruction::Lt
                | Instruction::Le
                | Instruction::Gt
                | Instruction::Ge
                | Instruction::Contains
                | Instruction::HasKey
                | Instruction::Index
                | Instruction::MakeRange(_) => {
                    require(2)?;
                    next(fallthrough, (state.0 - 1, state.1, state.2))?;
                }
                Instruction::Slice(_) => {
                    require(3)?;
                    next(fallthrough, (state.0 - 2, state.1, state.2))?;
                }
                Instruction::MakeArray(count)
                | Instruction::MergeArrays(count)
                | Instruction::MergeMaps(count)
                | Instruction::Concat(count) => {
                    let count = *count as i32;
                    require(count)?;
                    next(fallthrough, (state.0 - count + 1, state.1, state.2))?;
                }
                Instruction::Append => {
                    require(2)?;
                    next(fallthrough, (state.0 - 1, state.1, state.2))?;
                }
                Instruction::MakeMap(keys) => {
                    let count = keys.len() as i32;
                    require(count)?;
                    next(fallthrough, (state.0 - count + 1, state.1, state.2))?;
                }
                Instruction::Call(count) => {
                    let count = *count as i32;
                    require(count + 1)?;
                    next(fallthrough, (state.0 - count, state.1, state.2))?;
                }
                Instruction::SuperCall(count) => {
                    let count = *count as i32;
                    require(count)?;
                    next(fallthrough, (state.0 - count + 1, state.1, state.2))?;
                }
                Instruction::MemberCall { count, .. } | Instruction::Construct(count) => {
                    let count = *count as i32;
                    require(count + 1)?;
                    next(fallthrough, (state.0 - count, state.1, state.2))?;
                }
                Instruction::MemberCallSpread(_) | Instruction::ConstructSpread => {
                    require(2)?;
                    next(fallthrough, (state.0 - 1, state.1, state.2))?;
                }
                Instruction::CallSpread => {
                    require(2)?;
                    next(fallthrough, (state.0 - 1, state.1, state.2))?;
                }
                Instruction::SuperCallSpread => {
                    require(1)?;
                    next(fallthrough, state)?;
                }
                Instruction::MakeClass {
                    extends,
                    constructor,
                    instance_methods,
                    static_methods,
                    ..
                } => {
                    let count = i32::from(*extends)
                        + i32::from(*constructor)
                        + instance_methods.len() as i32
                        + static_methods.len() as i32;
                    require(count)?;
                    next(fallthrough, (state.0 - count + 1, state.1, state.2))?;
                }
                Instruction::Jump(offset) => next(jump_target(pc, *offset), state)?,
                Instruction::JumpIfFalse(offset) | Instruction::JumpIfNil(offset) => {
                    require(1)?;
                    next(fallthrough, state)?;
                    next(jump_target(pc, *offset), state)?;
                }
                Instruction::Try { catch, .. } => {
                    next(fallthrough, (state.0, state.1, state.2 + 1))?;
                    next(jump_target(pc, *catch), state)?;
                }
                Instruction::EndTry => {
                    if state.2 == 0 {
                        return Err(Error::verify(format!(
                            "handler stack underflow at instruction {pc}"
                        ))
                        .at_instruction(pc));
                    }
                    next(fallthrough, (state.0, state.1, state.2 - 1))?;
                }
                Instruction::Throw => {
                    require(1)?;
                }
                Instruction::IterStartEnumerable => {
                    require(2)?;
                    next(fallthrough, (state.0 - 2, state.1 + 1, state.2))?;
                }
                Instruction::IterStartMap => {
                    require(1)?;
                    next(fallthrough, (state.0 - 1, state.1 + 1, state.2))?;
                }
                Instruction::IterNext { end, .. } => {
                    if state.1 == 0 {
                        return Err(Error::verify(format!(
                            "iterator stack underflow at instruction {pc}"
                        ))
                        .at_instruction(pc));
                    }
                    next(fallthrough, state)?;
                    next(jump_target(pc, *end), (state.0, state.1 - 1, state.2))?;
                }
                Instruction::IterEnd => {
                    if state.1 == 0 {
                        return Err(Error::verify(format!(
                            "iterator stack underflow at instruction {pc}"
                        ))
                        .at_instruction(pc));
                    }
                    next(fallthrough, (state.0, state.1 - 1, state.2))?;
                }
                Instruction::Return => {
                    require(1)?;
                    if state.1 != 0 {
                        return Err(Error::verify(format!(
                            "iterator leaked at Return instruction {pc}"
                        ))
                        .at_instruction(pc));
                    }
                    if state.2 != 0 {
                        return Err(Error::verify(format!(
                            "handler leaked at Return instruction {pc}"
                        ))
                        .at_instruction(pc));
                    }
                }
            }
        }
        Ok(())
    }
}

struct FingerprintEncoder {
    hash: u64,
}

impl FingerprintEncoder {
    fn new() -> Self {
        Self {
            hash: 0xcbf29ce484222325,
        }
    }
    fn finish(self) -> u64 {
        self.hash
    }
    fn byte(&mut self, byte: u8) {
        self.hash ^= u64::from(byte);
        self.hash = self.hash.wrapping_mul(0x100000001b3);
    }
    fn tag(&mut self, tag: u8) {
        self.byte(tag);
    }
    fn u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }
    fn i32(&mut self, value: i32) {
        self.u64(value as i64 as u64);
    }
    fn bool(&mut self, value: bool) {
        self.byte(u8::from(value));
    }
    fn string(&mut self, value: &str) {
        self.u64(value.len() as u64);
        for byte in value.as_bytes() {
            self.byte(*byte);
        }
    }
    fn chunk(&mut self, chunk: &Chunk) {
        self.tag(0x01);
        self.u64(chunk.constants.len() as u64);
        for constant in &chunk.constants {
            self.constant(constant);
        }
        self.u64(chunk.code.len() as u64);
        for instruction in &chunk.code {
            self.instruction(instruction);
        }
    }
    fn constant(&mut self, constant: &Constant) {
        match constant {
            Constant::Value(value) => {
                self.tag(0x10);
                self.value(value);
            }
            Constant::Function {
                params,
                required,
                rest,
                receiver,
                receiver_bound,
                chunk,
            } => {
                self.tag(0x11);
                self.u64(params.len() as u64);
                for pattern in params {
                    self.pattern(pattern);
                }
                self.u64(*required as u64);
                self.option_string(rest.as_deref());
                self.bool(*receiver);
                if *receiver_bound {
                    self.tag(0x12);
                }
                self.chunk(chunk);
            }
        }
    }
    fn option_string(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.tag(1);
                self.string(value);
            }
            None => self.tag(0),
        }
    }
    fn value(&mut self, value: &Value) {
        match value {
            Value::Nil => self.tag(0x20),
            Value::Bool(value) => {
                self.tag(0x21);
                self.bool(*value);
            }
            Value::Number(value) => {
                self.tag(0x22);
                self.u64(value.to_bits());
            }
            Value::Integer(value) => {
                self.tag(0x27);
                let bytes = value.inner().to_signed_bytes_le();
                self.u64(bytes.len() as u64);
                for byte in bytes {
                    self.byte(byte);
                }
            }
            Value::Decimal(value) => {
                self.tag(0x29);
                let bytes = value.inner().to_signed_bytes_le();
                self.u64(bytes.len() as u64);
                for byte in bytes {
                    self.byte(byte);
                }
                self.u64(u64::from(value.scale()));
            }
            Value::String(value) => {
                self.tag(0x23);
                self.string(value);
            }
            Value::Array(values) => {
                self.tag(0x24);
                self.u64(values.len() as u64);
                for value in values.iter() {
                    self.value(value);
                }
            }
            Value::Map(values) => {
                self.tag(0x25);
                self.u64(values.len() as u64);
                for (key, value) in values.iter() {
                    self.string(key);
                    self.value(value);
                }
            }
            Value::Error(error) => {
                self.tag(0x28);
                self.string(error.code());
                self.string(error.message());
                self.value(error.data());
                match error.cause() {
                    Some(cause) => {
                        self.tag(1);
                        self.value(&Value::Error(Rc::new(cause.clone())));
                    }
                    None => self.tag(0),
                }
            }
            // Native/opaque functions cannot occur in compiler constants. Keep
            // their fingerprint representation deterministic if a host builds a
            // custom Chunk containing one.
            Value::Function(_) => self.tag(0x26),
            Value::Class(_) => self.tag(0x2a),
            Value::Instance(_) => self.tag(0x2b),
        }
    }
    fn pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Ignore => self.tag(0x30),
            Pattern::Bind(name) => {
                self.tag(0x31);
                self.string(name);
            }
            Pattern::Rest(name) => {
                self.tag(0x32);
                self.string(name);
            }
            Pattern::Default { pattern, default } => {
                self.tag(0x33);
                self.pattern(pattern);
                self.chunk(default);
            }
            Pattern::Array(patterns) => {
                self.tag(0x34);
                self.u64(patterns.len() as u64);
                for pattern in patterns {
                    self.pattern(pattern);
                }
            }
            Pattern::Map(fields) => {
                self.tag(0x35);
                self.u64(fields.len() as u64);
                for (key, pattern) in fields {
                    self.string(key);
                    self.pattern(pattern);
                }
            }
            Pattern::MapRest { fields, rest } => {
                self.tag(0x36);
                self.u64(fields.len() as u64);
                for (key, pattern) in fields {
                    self.string(key);
                    self.pattern(pattern);
                }
                self.string(rest);
            }
        }
    }
    fn instruction(&mut self, instruction: &Instruction) {
        macro_rules! simple {
            ($tag:expr) => {{
                self.tag($tag);
            }};
        }
        match instruction {
            Instruction::Constant(index) => {
                self.tag(0x40);
                self.u64(*index as u64);
            }
            Instruction::Load(name) => {
                self.tag(0x41);
                self.string(name);
            }
            Instruction::LoadOrNil(name) => {
                self.tag(0x42);
                self.string(name);
            }
            Instruction::Store(name) => {
                self.tag(0x43);
                self.string(name);
            }
            Instruction::Destructure(pattern) => {
                self.tag(0x44);
                self.pattern(pattern);
            }
            Instruction::Pop => simple!(0x45),
            Instruction::Dup => simple!(0x46),
            Instruction::Swap => simple!(0x47),
            Instruction::Rotate3 => simple!(0x48),
            Instruction::Neg => simple!(0x49),
            Instruction::Not => simple!(0x4a),
            Instruction::BitNot => simple!(0x4b),
            Instruction::Exists => simple!(0x4c),
            Instruction::Add => simple!(0x4d),
            Instruction::Sub => simple!(0x4e),
            Instruction::Mul => simple!(0x4f),
            Instruction::Div => simple!(0x50),
            Instruction::FloorDiv => simple!(0x51),
            Instruction::Rem => simple!(0x52),
            Instruction::Modulo => simple!(0x53),
            Instruction::BitAnd => simple!(0x54),
            Instruction::BitOr => simple!(0x55),
            Instruction::BitXor => simple!(0x56),
            Instruction::ShiftLeft => simple!(0x57),
            Instruction::ShiftRight => simple!(0x58),
            Instruction::ShiftRightUnsigned => simple!(0x59),
            Instruction::Pow => simple!(0x5a),
            Instruction::Eq => simple!(0x5b),
            Instruction::Ne => simple!(0x5c),
            Instruction::Lt => simple!(0x5d),
            Instruction::Le => simple!(0x5e),
            Instruction::Gt => simple!(0x5f),
            Instruction::Ge => simple!(0x60),
            Instruction::Contains => simple!(0x61),
            Instruction::HasKey => simple!(0x62),
            Instruction::Jump(offset) => {
                self.tag(0x63);
                self.i32(*offset);
            }
            Instruction::JumpIfFalse(offset) => {
                self.tag(0x64);
                self.i32(*offset);
            }
            Instruction::JumpIfNil(offset) => {
                self.tag(0x65);
                self.i32(*offset);
            }
            Instruction::Try { catch, name } => {
                self.tag(0x66);
                self.i32(*catch);
                self.string(name);
            }
            Instruction::EndTry => simple!(0x67),
            Instruction::Throw => simple!(0x68),
            Instruction::IterStartEnumerable => simple!(0x69),
            Instruction::IterStartMap => simple!(0x6a),
            Instruction::IterNext { patterns, end } => {
                self.tag(0x6b);
                self.u64(patterns.len() as u64);
                for pattern in patterns {
                    self.pattern(pattern);
                }
                self.i32(*end);
            }
            Instruction::IterEnd => simple!(0x6c),
            Instruction::MakeArray(count) => {
                self.tag(0x6d);
                self.u64(*count as u64);
            }
            Instruction::Append => simple!(0x6e),
            Instruction::MergeArrays(count) => {
                self.tag(0x6f);
                self.u64(*count as u64);
            }
            Instruction::MergeMaps(count) => {
                self.tag(0x70);
                self.u64(*count as u64);
            }
            Instruction::MakeRange(inclusive) => {
                self.tag(0x71);
                self.bool(*inclusive);
            }
            Instruction::MakeMap(keys) => {
                self.tag(0x72);
                self.u64(keys.len() as u64);
                for key in keys {
                    self.string(key);
                }
            }
            Instruction::Stringify => simple!(0x73),
            Instruction::Concat(count) => {
                self.tag(0x74);
                self.u64(*count as u64);
            }
            Instruction::Index => simple!(0x75),
            Instruction::Slice(inclusive) => {
                self.tag(0x76);
                self.bool(*inclusive);
            }
            Instruction::Member(name) => {
                self.tag(0x77);
                self.string(name);
            }
            Instruction::SetMember(name) => {
                self.tag(0x7e);
                self.string(name);
            }
            Instruction::LoadReceiver => simple!(0x84),
            Instruction::MemberCall { name, count } => {
                self.tag(0x7f);
                self.string(name);
                self.u64(*count as u64);
            }
            Instruction::MemberCallSpread(name) => {
                self.tag(0x82);
                self.string(name);
            }
            Instruction::SuperCall(count) => {
                self.tag(0x85);
                self.u64(*count as u64);
            }
            Instruction::SuperCallSpread => simple!(0x86),
            Instruction::Call(count) => {
                self.tag(0x78);
                self.u64(*count as u64);
            }
            Instruction::CallSpread => simple!(0x79),
            Instruction::Construct(count) => {
                self.tag(0x80);
                self.u64(*count as u64);
            }
            Instruction::ConstructSpread => simple!(0x83),
            Instruction::MakeFunction(index) => {
                self.tag(0x7a);
                self.u64(*index as u64);
            }
            Instruction::MakeBoundFunction(index) => {
                self.tag(0x87);
                self.u64(*index as u64);
            }
            Instruction::Return => simple!(0x7b),
            Instruction::Increment => simple!(0x7c),
            Instruction::Decrement => simple!(0x7d),
            Instruction::MakeClass {
                name,
                extends,
                constructor,
                instance_methods,
                static_methods,
            } => {
                self.tag(0x81);
                self.string(name);
                self.bool(*extends);
                self.bool(*constructor);
                self.u64(instance_methods.len() as u64);
                for method in instance_methods {
                    self.string(method);
                }
                self.u64(static_methods.len() as u64);
                for method in static_methods {
                    self.string(method);
                }
            }
        }
    }
}

fn jump_target(pc: usize, offset: i32) -> usize {
    (pc as i64 + 1 + offset as i64) as usize
}

fn validate_pattern(pattern: &Pattern) -> Result<(), Error> {
    validate_pattern_at(pattern, false)
}

fn validate_pattern_at(pattern: &Pattern, allow_rest: bool) -> Result<(), Error> {
    match pattern {
        Pattern::Ignore | Pattern::Bind(_) => Ok(()),
        Pattern::Rest(name) if allow_rest && !name.is_empty() && name != "_" => Ok(()),
        Pattern::Rest(_) => Err(Error::verify("array rest pattern must be final and named")),
        Pattern::Default { pattern, default } => {
            validate_pattern_at(pattern, allow_rest)?;
            default.verify()
        }
        Pattern::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                if matches!(item, Pattern::Rest(_)) && index + 1 != items.len() {
                    return Err(Error::verify("array rest pattern must be final"));
                }
                validate_pattern_at(item, true)?;
            }
            Ok(())
        }
        Pattern::Map(fields) => fields
            .iter()
            .try_for_each(|(_, pattern)| validate_pattern_at(pattern, false)),
        Pattern::MapRest { fields, rest } => {
            if rest.is_empty() || rest == "_" {
                return Err(Error::verify("map rest pattern must be named"));
            }
            fields
                .iter()
                .try_for_each(|(_, pattern)| validate_pattern_at(pattern, false))
        }
    }
}
