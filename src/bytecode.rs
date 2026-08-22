use crate::{
    parser::{Binary, Expr, Item, Param, Stmt, Unary, Update},
    vm::{Error, Value},
};
use std::{
    collections::{BTreeMap, VecDeque},
    rc::Rc,
};

/// A strict, recursively shaped binding pattern used by destructuring assignment.
#[derive(Clone, Debug)]
pub enum Pattern {
    Ignore,
    Bind(String),
    Array(Vec<Pattern>),
    Map(Vec<(String, Pattern)>),
}

#[derive(Clone, Debug, Default)]
pub struct Chunk {
    pub constants: Vec<Constant>,
    pub code: Vec<Instruction>,
}
#[derive(Clone, Debug)]
pub enum Constant {
    Value(Value),
    Function {
        params: Vec<Pattern>,
        required: usize,
        rest: Option<String>,
        chunk: Rc<Chunk>,
    },
}
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
    Try { catch: i32, name: String },
    EndTry,
    Throw,
    IterStartArray,
    IterStartMap,
    IterNext { patterns: Vec<Pattern>, end: i32 },
    IterEnd,
    MakeArray(usize),
    Append,
    MergeArrays(usize),
    MakeRange(bool),
    MakeMap(Vec<String>),
    Stringify,
    Concat(usize),
    Index,
    Slice(bool),
    Member(String),
    Call(usize),
    CallSpread,
    MakeFunction(usize),
    Return,
}

impl Chunk {
    pub fn disassemble(&self) -> String {
        self.code
            .iter()
            .enumerate()
            .map(|(i, op)| format!("{i:04} {op:?}\n"))
            .collect()
    }
    pub fn verify(&self) -> Result<(), Error> {
        if self.code.is_empty() {
            return Err(Error::verify("chunk is empty"));
        }
        if !matches!(self.code.last(), Some(Instruction::Return)) {
            return Err(Error::verify("chunk does not end in Return"));
        }
        for (pc, op) in self.code.iter().enumerate() {
            match op {
                Instruction::Constant(i) => match self.constants.get(*i) {
                    Some(Constant::Value(_)) => {}
                    Some(_) => {
                        return Err(Error::verify(format!(
                            "constant {i} at instruction {pc} is not a value"
                        )));
                    }
                    None => {
                        return Err(Error::verify(format!(
                            "constant {i} at instruction {pc} is out of bounds"
                        )));
                    }
                },
                Instruction::MakeFunction(i) => match self.constants.get(*i) {
                    Some(Constant::Function { chunk, .. }) => chunk.verify()?,
                    Some(_) => {
                        return Err(Error::verify(format!(
                            "constant {i} at instruction {pc} is not a function"
                        )));
                    }
                    None => {
                        return Err(Error::verify(format!(
                            "constant {i} at instruction {pc} is out of bounds"
                        )));
                    }
                },
                Instruction::Jump(offset)
                | Instruction::JumpIfFalse(offset)
                | Instruction::JumpIfNil(offset)
                | Instruction::IterNext { end: offset, .. } => {
                    let target = pc as i64 + 1 + *offset as i64;
                    if target < 0 || target >= self.code.len() as i64 {
                        return Err(Error::verify(format!(
                            "jump at instruction {pc} leaves chunk"
                        )));
                    }
                }
                Instruction::Try { catch, .. } => {
                    let target = pc as i64 + 1 + *catch as i64;
                    if target < 0 || target >= self.code.len() as i64 {
                        return Err(Error::verify(format!(
                            "catch target at instruction {pc} leaves chunk"
                        )));
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
                    Err(Error::verify(format!(
                        "stack underflow at instruction {pc}"
                    )))
                } else {
                    Ok(())
                }
            };
            let mut next = |target: usize, successor: State| -> Result<(), Error> {
                match states[target] {
                    Some(existing) if existing != successor => Err(Error::verify(format!(
                        "inconsistent stack state at instruction {target}"
                    ))),
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
                | Instruction::MakeFunction(_) => {
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
                | Instruction::Stringify
                | Instruction::Member(_) => {
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
                Instruction::CallSpread => {
                    require(2)?;
                    next(fallthrough, (state.0 - 1, state.1, state.2))?;
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
                        )));
                    }
                    next(fallthrough, (state.0, state.1, state.2 - 1))?;
                }
                Instruction::Throw => {
                    require(1)?;
                }
                Instruction::IterStartArray => {
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
                        )));
                    }
                    next(fallthrough, state)?;
                    next(jump_target(pc, *end), (state.0, state.1 - 1, state.2))?;
                }
                Instruction::IterEnd => {
                    if state.1 == 0 {
                        return Err(Error::verify(format!(
                            "iterator stack underflow at instruction {pc}"
                        )));
                    }
                    next(fallthrough, (state.0, state.1 - 1, state.2))?;
                }
                Instruction::Return => {
                    require(1)?;
                    if state.1 != 0 {
                        return Err(Error::verify(format!(
                            "iterator leaked at Return instruction {pc}"
                        )));
                    }
                    if state.2 != 0 {
                        return Err(Error::verify(format!(
                            "handler leaked at Return instruction {pc}"
                        )));
                    }
                }
            }
        }
        Ok(())
    }
}

fn jump_target(pc: usize, offset: i32) -> usize {
    (pc as i64 + 1 + offset as i64) as usize
}

pub(crate) fn compile(program: &[Stmt]) -> Result<Chunk, Error> {
    let mut c = Compiler::default();
    if program.is_empty() {
        c.emit_const(Value::Nil);
    } else {
        for (i, stmt) in program.iter().enumerate() {
            if i + 1 != program.len() {
                if let Stmt::Expr(Expr::For(patterns, map, iterable, step, filter, body)) = stmt {
                    c.for_discard(
                        patterns,
                        *map,
                        iterable,
                        step.as_deref(),
                        filter.as_deref(),
                        body,
                    )?;
                } else {
                    c.stmt(stmt)?;
                }
            } else {
                c.stmt(stmt)?;
            }
            if i + 1 != program.len() {
                c.emit(Instruction::Pop);
            }
        }
    }
    c.emit(Instruction::Return);
    Ok(c.chunk)
}

/// Evaluates only side-effect-free literal expressions at compile time. A
/// failed conversion deliberately returns `None`, leaving the original
/// expression on the normal VM path so strict runtime errors remain intact.
fn constant_value(expression: &Expr) -> Option<Value> {
    match expression {
        Expr::Number(value) => Some(Value::Number(*value)),
        Expr::String(value) => Some(Value::String(Rc::from(value.as_str()))),
        Expr::Bool(value) => Some(Value::Bool(*value)),
        Expr::Nil => Some(Value::Nil),
        Expr::Interpolate(parts) => {
            let mut output = String::new();
            for part in parts {
                output.push_str(&constant_value(part)?.to_string());
            }
            Some(Value::String(Rc::from(output)))
        }
        Expr::Array(items) => {
            if items.iter().any(|item| matches!(item, Item::Splat(_))) {
                return None;
            }
            let values = items
                .iter()
                .map(|item| match item {
                    Item::Expr(item) => constant_value(item),
                    Item::Splat(_) => None,
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Value::Array(Rc::new(values)))
        }
        Expr::Map(entries) => {
            let mut values = BTreeMap::new();
            for (key, value) in entries {
                values.insert(key.clone(), constant_value(value)?);
            }
            Some(Value::Map(Rc::new(values)))
        }
        Expr::Unary(operator, operand) => {
            let value = constant_value(operand)?;
            match operator {
                Unary::Neg => Some(Value::Number(-value.as_number()?)),
                Unary::Not => Some(Value::Bool(!value.as_bool()?)),
                Unary::BitNot => Some(Value::Number((!constant_bit_integer(&value)?) as f64)),
            }
        }
        Expr::Exists(operand) => Some(Value::Bool(!matches!(constant_value(operand)?, Value::Nil))),
        Expr::Binary(left, operator, right) => {
            let left = constant_value(left)?;
            let right = constant_value(right)?;
            constant_binary(*operator, left, right)
        }
        Expr::If(condition, yes, no) => match constant_value(condition)? {
            Value::Bool(true) => constant_value(yes),
            Value::Bool(false) => constant_value(no),
            _ => None,
        },
        _ => None,
    }
}

fn constant_binary(operator: Binary, left: Value, right: Value) -> Option<Value> {
    match operator {
        Binary::Coalesce => Some(if matches!(left, Value::Nil) {
            right
        } else {
            left
        }),
        Binary::And => Some(if !left.as_bool()? { left } else { right }),
        Binary::Or => Some(if left.as_bool()? { left } else { right }),
        Binary::Add => Some(Value::Number(left.as_number()? + right.as_number()?)),
        Binary::Sub => Some(Value::Number(left.as_number()? - right.as_number()?)),
        Binary::Mul => Some(Value::Number(left.as_number()? * right.as_number()?)),
        Binary::Div => Some(Value::Number(left.as_number()? / right.as_number()?)),
        Binary::FloorDiv => Some(Value::Number(
            (left.as_number()? / right.as_number()?).floor(),
        )),
        Binary::Rem => Some(Value::Number(left.as_number()? % right.as_number()?)),
        Binary::Modulo => {
            let left = left.as_number()?;
            let right = right.as_number()?;
            Some(Value::Number((left % right + right) % right))
        }
        Binary::Pow => Some(Value::Number(left.as_number()?.powf(right.as_number()?))),
        Binary::BitAnd => constant_bit_binary(left, right, |a, b| a & b),
        Binary::BitOr => constant_bit_binary(left, right, |a, b| a | b),
        Binary::BitXor => constant_bit_binary(left, right, |a, b| a ^ b),
        Binary::ShiftLeft => constant_shift(left, right, |a, b| a.wrapping_shl(b)),
        Binary::ShiftRight => constant_shift(left, right, |a, b| a.wrapping_shr(b)),
        Binary::ShiftRightUnsigned => {
            constant_shift(left, right, |a, b| ((a as u32).wrapping_shr(b)) as i32)
        }
        Binary::Eq => Some(Value::Bool(constant_equal(&left, &right))),
        Binary::Ne => Some(Value::Bool(!constant_equal(&left, &right))),
        Binary::Lt => constant_order(left, right, |a, b| a < b),
        Binary::Le => constant_order(left, right, |a, b| a <= b),
        Binary::Gt => constant_order(left, right, |a, b| a > b),
        Binary::Ge => constant_order(left, right, |a, b| a >= b),
        Binary::In | Binary::NotIn => {
            let Value::Array(values) = right else {
                return None;
            };
            let found = values.iter().any(|value| constant_equal(&left, value));
            Some(Value::Bool(if matches!(operator, Binary::In) {
                found
            } else {
                !found
            }))
        }
        Binary::Of | Binary::NotOf => {
            let (Value::String(key), Value::Map(values)) = (left, right) else {
                return None;
            };
            let found = values.contains_key(key.as_ref());
            Some(Value::Bool(if matches!(operator, Binary::Of) {
                found
            } else {
                !found
            }))
        }
    }
}

fn constant_order(
    left: Value,
    right: Value,
    operation: impl FnOnce(f64, f64) -> bool,
) -> Option<Value> {
    Some(Value::Bool(operation(
        left.as_number()?,
        right.as_number()?,
    )))
}

fn constant_bit_integer(value: &Value) -> Option<i32> {
    let value = value.as_number()?;
    if !value.is_finite()
        || value.fract() != 0.
        || !(-2_147_483_648.0..=2_147_483_647.0).contains(&value)
    {
        return None;
    }
    Some(value as i32)
}

fn constant_bit_binary(
    left: Value,
    right: Value,
    operation: impl FnOnce(i32, i32) -> i32,
) -> Option<Value> {
    Some(Value::Number(
        operation(constant_bit_integer(&left)?, constant_bit_integer(&right)?) as f64,
    ))
}

fn constant_shift(
    left: Value,
    right: Value,
    operation: impl FnOnce(i32, u32) -> i32,
) -> Option<Value> {
    let shift = constant_bit_integer(&right)?;
    if !(0..32).contains(&shift) {
        return None;
    }
    Some(Value::Number(
        operation(constant_bit_integer(&left)?, shift as u32) as f64,
    ))
}

fn constant_equal(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Nil, Value::Nil) => true,
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Number(left), Value::Number(right)) => left == right,
        (Value::String(left), Value::String(right)) => left == right,
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right.iter())
                    .all(|(left, right)| constant_equal(left, right))
        }
        (Value::Map(left), Value::Map(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, left)| {
                    right
                        .get(key)
                        .is_some_and(|right| constant_equal(left, right))
                })
        }
        _ => false,
    }
}

#[derive(Default)]
struct Compiler {
    chunk: Chunk,
    loops: Vec<LoopContext>,
    in_function: bool,
    return_cleanups: Vec<ReturnCleanup>,
}
struct LoopContext {
    breaks: Vec<usize>,
    continue_target: usize,
    owns_iterator: bool,
}
#[derive(Clone)]
struct ReturnCleanup {
    end_try: bool,
    finalizer: Option<Expr>,
}
impl Compiler {
    fn emit(&mut self, op: Instruction) -> usize {
        let i = self.chunk.code.len();
        self.chunk.code.push(op);
        i
    }
    fn emit_const(&mut self, v: Value) {
        let i = self.chunk.constants.len();
        self.chunk.constants.push(Constant::Value(v));
        self.emit(Instruction::Constant(i));
    }
    fn patch_jump(&mut self, at: usize) {
        let offset = self.chunk.code.len() as i64 - at as i64 - 1;
        let op = &mut self.chunk.code[at];
        match op {
            Instruction::Jump(x) | Instruction::JumpIfFalse(x) | Instruction::JumpIfNil(x) => {
                *x = offset as i32
            }
            Instruction::IterNext { end, .. } => *end = offset as i32,
            Instruction::Try { catch, .. } => *catch = offset as i32,
            _ => unreachable!(),
        }
    }
    fn patch_jump_to(&mut self, at: usize, target: usize) {
        let offset = target as i64 - at as i64 - 1;
        let op = &mut self.chunk.code[at];
        match op {
            Instruction::Jump(x) | Instruction::JumpIfFalse(x) | Instruction::JumpIfNil(x) => {
                *x = offset as i32
            }
            Instruction::IterNext { end, .. } => *end = offset as i32,
            Instruction::Try { catch, .. } => *catch = offset as i32,
            _ => unreachable!(),
        }
    }
    fn emit_back_jump(&mut self, target: usize) {
        self.emit(Instruction::Jump(
            target as i32 - self.chunk.code.len() as i32 - 1,
        ));
    }
    fn stmt(&mut self, s: &Stmt) -> Result<(), Error> {
        match s {
            Stmt::Assign(n, e) => {
                self.expr(e)?;
                self.emit(Instruction::Store(n.clone()));
            }
            Stmt::Destructure(pattern, e) => {
                self.expr(e)?;
                self.emit(Instruction::Destructure(pattern.clone()));
            }
            Stmt::Expr(e) => self.expr(e)?,
        }
        Ok(())
    }
    fn for_discard(
        &mut self,
        patterns: &[Pattern],
        map: bool,
        iterable: &Expr,
        step: Option<&Expr>,
        filter: Option<&Expr>,
        body: &Expr,
    ) -> Result<(), Error> {
        self.expr(iterable)?;
        if map {
            self.emit(Instruction::IterStartMap);
        } else {
            if let Some(step) = step {
                self.expr(step)?;
            } else {
                self.emit_const(Value::Number(1.));
            }
            self.emit(Instruction::IterStartArray);
        }
        let start = self.chunk.code.len();
        let exit = self.emit(Instruction::IterNext {
            patterns: patterns.to_vec(),
            end: 0,
        });
        self.loops.push(LoopContext {
            breaks: vec![],
            continue_target: start,
            owns_iterator: true,
        });
        let filtered = if let Some(filter) = filter {
            self.expr(filter)?;
            let skip = self.emit(Instruction::JumpIfFalse(0));
            self.emit(Instruction::Pop);
            Some(skip)
        } else {
            None
        };
        self.expr(body)?;
        self.emit(Instruction::Pop);
        let loop_context = self.loops.pop().expect("loop context");
        self.emit_back_jump(start);
        if let Some(skip) = filtered {
            self.patch_jump(skip);
            self.emit(Instruction::Pop);
            self.emit_back_jump(start);
        }
        self.patch_jump(exit);
        let break_target = self.chunk.code.len();
        self.emit_const(Value::Nil);
        for jump in loop_context.breaks {
            self.patch_jump_to(jump, break_target);
        }
        Ok(())
    }
    fn expr(&mut self, e: &Expr) -> Result<(), Error> {
        if let Some(value) = constant_value(e) {
            self.emit_const(value);
            return Ok(());
        }
        match e {
            Expr::Number(n) => self.emit_const(Value::Number(*n)),
            Expr::String(s) => self.emit_const(Value::String(Rc::from(s.as_str()))),
            Expr::Interpolate(parts) => {
                for part in parts {
                    self.expr(part)?;
                    self.emit(Instruction::Stringify);
                }
                self.emit(Instruction::Concat(parts.len()));
            }
            Expr::Bool(b) => self.emit_const(Value::Bool(*b)),
            Expr::Nil => self.emit_const(Value::Nil),
            Expr::Name(n) => {
                self.emit(Instruction::Load(n.clone()));
            }
            Expr::Assign(n, value) => {
                self.expr(value)?;
                self.emit(Instruction::Store(n.clone()));
            }
            Expr::AssignIfNil(name, value) => {
                self.emit(Instruction::LoadOrNil(name.clone()));
                self.emit(Instruction::Dup);
                let nil = self.emit(Instruction::JumpIfNil(0));
                self.emit(Instruction::Pop);
                let end = self.emit(Instruction::Jump(0));
                self.patch_jump(nil);
                self.emit(Instruction::Pop);
                self.emit(Instruction::Pop);
                self.expr(value)?;
                self.emit(Instruction::Store(name.clone()));
                self.patch_jump(end);
            }
            Expr::Update(name, update, prefix) => {
                self.emit(Instruction::Load(name.clone()));
                if !prefix {
                    self.emit(Instruction::Dup);
                }
                self.emit_const(Value::Number(1.));
                self.emit(match update {
                    Update::Increment => Instruction::Add,
                    Update::Decrement => Instruction::Sub,
                });
                self.emit(Instruction::Store(name.clone()));
                if !prefix {
                    self.emit(Instruction::Pop);
                }
            }
            Expr::Destructure(pattern, value) => {
                self.expr(value)?;
                self.emit(Instruction::Destructure(pattern.clone()));
            }
            Expr::Array(items) => {
                self.items(items, true)?;
            }
            Expr::Range(start, end, inclusive) => {
                self.expr(start)?;
                self.expr(end)?;
                self.emit(Instruction::MakeRange(*inclusive));
            }
            Expr::Map(map) => {
                for x in map.values() {
                    self.expr(x)?
                }
                self.emit(Instruction::MakeMap(map.keys().cloned().collect()));
            }
            Expr::Unary(op, x) => {
                self.expr(x)?;
                self.emit(match op {
                    Unary::Neg => Instruction::Neg,
                    Unary::Not => Instruction::Not,
                    Unary::BitNot => Instruction::BitNot,
                });
            }
            Expr::Exists(value) => {
                self.expr(value)?;
                self.emit(Instruction::Exists);
            }
            Expr::Binary(a, op, b) => match op {
                Binary::And => self.short_circuit(a, b, false)?,
                Binary::Or => self.short_circuit(a, b, true)?,
                Binary::Coalesce => self.coalesce(a, b)?,
                _ => {
                    self.expr(a)?;
                    self.expr(b)?;
                    self.emit(match op {
                        Binary::Add => Instruction::Add,
                        Binary::Sub => Instruction::Sub,
                        Binary::Mul => Instruction::Mul,
                        Binary::Div => Instruction::Div,
                        Binary::FloorDiv => Instruction::FloorDiv,
                        Binary::Rem => Instruction::Rem,
                        Binary::Modulo => Instruction::Modulo,
                        Binary::BitAnd => Instruction::BitAnd,
                        Binary::BitOr => Instruction::BitOr,
                        Binary::BitXor => Instruction::BitXor,
                        Binary::ShiftLeft => Instruction::ShiftLeft,
                        Binary::ShiftRight => Instruction::ShiftRight,
                        Binary::ShiftRightUnsigned => Instruction::ShiftRightUnsigned,
                        Binary::Pow => Instruction::Pow,
                        Binary::Eq => Instruction::Eq,
                        Binary::Ne => Instruction::Ne,
                        Binary::Lt => Instruction::Lt,
                        Binary::Le => Instruction::Le,
                        Binary::Gt => Instruction::Gt,
                        Binary::Ge => Instruction::Ge,
                        Binary::In => Instruction::Contains,
                        Binary::Of => Instruction::HasKey,
                        Binary::NotIn => Instruction::Contains,
                        Binary::NotOf => Instruction::HasKey,
                        Binary::Coalesce => unreachable!(),
                        _ => unreachable!(),
                    });
                    if matches!(op, Binary::NotIn | Binary::NotOf) {
                        self.emit(Instruction::Not);
                    }
                }
            },
            Expr::CompareChain(operands, operators) => {
                self.compare_chain(operands, operators)?;
            }
            Expr::Index(a, b) => {
                self.expr(a)?;
                self.expr(b)?;
                self.emit(Instruction::Index);
            }
            Expr::Slice(target, start, end, inclusive) => {
                self.expr(target)?;
                self.expr(start)?;
                self.expr(end)?;
                self.emit(Instruction::Slice(*inclusive));
            }
            Expr::Member(target, name) => {
                self.expr(target)?;
                self.emit(Instruction::Member(name.clone()));
            }
            Expr::Call(fun, args) => {
                self.expr(fun)?;
                if self.items(args, false)? {
                    self.emit(Instruction::CallSpread);
                } else {
                    self.emit(Instruction::Call(args.len()));
                }
            }
            Expr::SoakIndex(target, key) => {
                self.expr(target)?;
                self.emit(Instruction::Dup);
                let nil = self.emit(Instruction::JumpIfNil(0));
                self.emit(Instruction::Pop);
                self.expr(key)?;
                self.emit(Instruction::Index);
                let end = self.emit(Instruction::Jump(0));
                self.patch_jump(nil);
                self.emit(Instruction::Pop);
                self.patch_jump(end);
            }
            Expr::SoakSlice(target, start, finish, inclusive) => {
                self.expr(target)?;
                self.emit(Instruction::Dup);
                let nil = self.emit(Instruction::JumpIfNil(0));
                self.emit(Instruction::Pop);
                self.expr(start)?;
                self.expr(finish)?;
                self.emit(Instruction::Slice(*inclusive));
                let end = self.emit(Instruction::Jump(0));
                self.patch_jump(nil);
                self.emit(Instruction::Pop);
                self.patch_jump(end);
            }
            Expr::SoakMember(target, name) => {
                self.expr(target)?;
                self.emit(Instruction::Dup);
                let nil = self.emit(Instruction::JumpIfNil(0));
                self.emit(Instruction::Pop);
                self.emit(Instruction::Member(name.clone()));
                let end = self.emit(Instruction::Jump(0));
                self.patch_jump(nil);
                self.emit(Instruction::Pop);
                self.patch_jump(end);
            }
            Expr::SoakCall(fun, args) => {
                self.expr(fun)?;
                self.emit(Instruction::Dup);
                let nil = self.emit(Instruction::JumpIfNil(0));
                self.emit(Instruction::Pop);
                if self.items(args, false)? {
                    self.emit(Instruction::CallSpread);
                } else {
                    self.emit(Instruction::Call(args.len()));
                }
                let end = self.emit(Instruction::Jump(0));
                self.patch_jump(nil);
                self.emit(Instruction::Pop);
                self.patch_jump(end);
            }
            Expr::If(cond, yes, no) => {
                self.expr(cond)?;
                let jf = self.emit(Instruction::JumpIfFalse(0));
                self.emit(Instruction::Pop);
                self.expr(yes)?;
                let end = self.emit(Instruction::Jump(0));
                self.patch_jump(jf);
                self.emit(Instruction::Pop);
                self.expr(no)?;
                self.patch_jump(end);
            }
            Expr::While(cond, body) => {
                let start = self.chunk.code.len();
                self.expr(cond)?;
                let exit = self.emit(Instruction::JumpIfFalse(0));
                self.emit(Instruction::Pop);
                self.loops.push(LoopContext {
                    breaks: vec![],
                    continue_target: start,
                    owns_iterator: false,
                });
                self.expr(body)?;
                self.emit(Instruction::Pop);
                let loop_context = self.loops.pop().expect("loop context");
                self.emit_back_jump(start);
                self.patch_jump(exit);
                self.emit(Instruction::Pop);
                let break_target = self.chunk.code.len();
                self.emit_const(Value::Nil);
                for jump in loop_context.breaks {
                    self.patch_jump_to(jump, break_target);
                }
            }
            Expr::For(patterns, map, iterable, step, filter, body) => {
                self.emit(Instruction::MakeArray(0));
                self.expr(iterable)?;
                if *map {
                    self.emit(Instruction::IterStartMap);
                } else {
                    if let Some(step) = step {
                        self.expr(step)?;
                    } else {
                        self.emit_const(Value::Number(1.));
                    }
                    self.emit(Instruction::IterStartArray);
                }
                let start = self.chunk.code.len();
                let exit = self.emit(Instruction::IterNext {
                    patterns: patterns.clone(),
                    end: 0,
                });
                self.loops.push(LoopContext {
                    breaks: vec![],
                    continue_target: start,
                    owns_iterator: true,
                });
                let filtered = if let Some(filter) = filter {
                    self.expr(filter)?;
                    let skip = self.emit(Instruction::JumpIfFalse(0));
                    self.emit(Instruction::Pop);
                    Some(skip)
                } else {
                    None
                };
                self.expr(body)?;
                self.emit(Instruction::Append);
                let loop_context = self.loops.pop().expect("loop context");
                self.emit_back_jump(start);
                if let Some(skip) = filtered {
                    self.patch_jump(skip);
                    self.emit(Instruction::Pop);
                    self.emit_back_jump(start);
                }
                self.patch_jump(exit);
                let break_target = self.chunk.code.len();
                for jump in loop_context.breaks {
                    self.patch_jump_to(jump, break_target);
                }
            }
            Expr::Break => {
                let owns_iterator = self
                    .loops
                    .last()
                    .ok_or_else(|| Error::parse("break outside of loop"))?;
                if owns_iterator.owns_iterator {
                    self.emit(Instruction::IterEnd);
                }
                let jump = self.emit(Instruction::Jump(0));
                self.loops
                    .last_mut()
                    .expect("loop context")
                    .breaks
                    .push(jump);
            }
            Expr::Continue => {
                let target = self
                    .loops
                    .last()
                    .ok_or_else(|| Error::parse("continue outside of loop"))?
                    .continue_target;
                self.emit_back_jump(target);
            }
            Expr::Return(value) => {
                if !self.in_function {
                    return Err(Error::parse("return outside of function"));
                }
                if let Some(value) = value {
                    self.expr(value)?;
                } else {
                    self.emit_const(Value::Nil);
                }
                let iterators_to_close = self
                    .loops
                    .iter()
                    .filter(|loop_context| loop_context.owns_iterator)
                    .count();
                for _ in 0..iterators_to_close {
                    self.emit(Instruction::IterEnd);
                }
                let cleanups = self.return_cleanups.clone();
                for index in (0..cleanups.len()).rev() {
                    let cleanup = &cleanups[index];
                    if cleanup.end_try {
                        self.emit(Instruction::EndTry);
                    }
                    if let Some(finalizer) = &cleanup.finalizer {
                        let saved = std::mem::replace(
                            &mut self.return_cleanups,
                            cleanups[..index].to_vec(),
                        );
                        self.expr(finalizer)?;
                        self.return_cleanups = saved;
                        self.emit(Instruction::Pop);
                    }
                }
                self.emit(Instruction::Return);
            }
            Expr::Function(params, rest, body) => {
                self.function(params, rest.as_ref(), body)?;
            }
            Expr::Class(name, params, body) => {
                self.function(params, None, body)?;
                self.emit(Instruction::Store(name.clone()));
            }
            Expr::Block(statements) => {
                if statements.is_empty() {
                    self.emit_const(Value::Nil);
                } else {
                    for (index, statement) in statements.iter().enumerate() {
                        if index + 1 != statements.len() {
                            if let Stmt::Expr(Expr::For(
                                patterns,
                                map,
                                iterable,
                                step,
                                filter,
                                body,
                            )) = statement
                            {
                                self.for_discard(
                                    patterns,
                                    *map,
                                    iterable,
                                    step.as_deref(),
                                    filter.as_deref(),
                                    body,
                                )?;
                            } else {
                                self.stmt(statement)?;
                            }
                        } else {
                            self.stmt(statement)?;
                        }
                        if index + 1 != statements.len() {
                            self.emit(Instruction::Pop);
                        }
                    }
                }
            }
            Expr::Switch(subject, cases, fallback) => {
                self.expr(subject)?;
                let mut end_jumps = vec![];
                for (patterns, body) in cases {
                    for pattern in patterns {
                        self.emit(Instruction::Dup);
                        self.expr(pattern)?;
                        self.emit(Instruction::Eq);
                        let next_pattern = self.emit(Instruction::JumpIfFalse(0));
                        self.emit(Instruction::Pop);
                        self.emit(Instruction::Pop);
                        self.expr(body)?;
                        end_jumps.push(self.emit(Instruction::Jump(0)));
                        self.patch_jump(next_pattern);
                        self.emit(Instruction::Pop);
                    }
                }
                self.emit(Instruction::Pop);
                if let Some(fallback) = fallback {
                    self.expr(fallback)?;
                } else {
                    self.emit_const(Value::Nil);
                }
                for jump in end_jumps {
                    self.patch_jump(jump);
                }
            }
            Expr::Try(body, name, handler, finalizer) => {
                let catch = self.emit(Instruction::Try {
                    catch: 0,
                    name: name.clone(),
                });
                self.return_cleanups.push(ReturnCleanup {
                    end_try: true,
                    finalizer: finalizer.as_deref().cloned(),
                });
                self.expr(body)?;
                self.return_cleanups.pop();
                self.emit(Instruction::EndTry);
                if let Some(finalizer) = finalizer {
                    self.expr(finalizer)?;
                    self.emit(Instruction::Pop);
                }
                let end = self.emit(Instruction::Jump(0));
                self.patch_jump(catch);
                if finalizer.is_some() {
                    self.return_cleanups.push(ReturnCleanup {
                        end_try: false,
                        finalizer: finalizer.as_deref().cloned(),
                    });
                }
                self.expr(handler)?;
                if finalizer.is_some() {
                    self.return_cleanups.pop();
                }
                if let Some(finalizer) = finalizer {
                    self.expr(finalizer)?;
                    self.emit(Instruction::Pop);
                }
                self.patch_jump(end);
            }
            Expr::Throw(value) => {
                self.expr(value)?;
                self.emit(Instruction::Throw);
            }
            Expr::Do(function) => {
                self.expr(function)?;
                self.emit(Instruction::Call(0));
            }
        }
        Ok(())
    }
    fn compare_chain(&mut self, operands: &[Expr], operators: &[Binary]) -> Result<(), Error> {
        debug_assert_eq!(operands.len(), operators.len() + 1);
        self.expr(&operands[0])?;
        let mut failures = vec![];
        for (index, op) in operators.iter().enumerate() {
            self.expr(&operands[index + 1])?;
            if index + 1 != operators.len() {
                self.emit(Instruction::Dup);
                self.emit(Instruction::Rotate3);
                self.emit(Instruction::Swap);
            }
            self.emit(match op {
                Binary::Eq => Instruction::Eq,
                Binary::Ne => Instruction::Ne,
                Binary::Lt => Instruction::Lt,
                Binary::Le => Instruction::Le,
                Binary::Gt => Instruction::Gt,
                Binary::Ge => Instruction::Ge,
                _ => return Err(Error::parse("invalid chained comparison operator")),
            });
            if index + 1 != operators.len() {
                failures.push(self.emit(Instruction::JumpIfFalse(0)));
                self.emit(Instruction::Pop);
            }
        }
        let end = self.emit(Instruction::Jump(0));
        for failure in failures {
            self.patch_jump(failure);
        }
        self.emit(Instruction::Swap);
        self.emit(Instruction::Pop);
        self.patch_jump(end);
        Ok(())
    }
    fn function(
        &mut self,
        params: &[Param],
        rest: Option<&String>,
        body: &Expr,
    ) -> Result<(), Error> {
        let mut inner = Compiler {
            in_function: true,
            ..Default::default()
        };
        for param in params {
            let Some(default) = &param.default else {
                continue;
            };
            let Pattern::Bind(name) = &param.pattern else {
                return Err(Error::parse("default parameter must be a name"));
            };
            inner.emit(Instruction::Load(name.clone()));
            let use_default = inner.emit(Instruction::JumpIfNil(0));
            inner.emit(Instruction::Pop);
            let done = inner.emit(Instruction::Jump(0));
            inner.patch_jump(use_default);
            inner.emit(Instruction::Pop);
            inner.expr(default)?;
            inner.emit(Instruction::Store(name.clone()));
            inner.emit(Instruction::Pop);
            inner.patch_jump(done);
        }
        inner.expr(body)?;
        inner.emit(Instruction::Return);
        let idx = self.chunk.constants.len();
        self.chunk.constants.push(Constant::Function {
            params: params.iter().map(|param| param.pattern.clone()).collect(),
            required: params
                .iter()
                .take_while(|param| param.default.is_none())
                .count(),
            rest: rest.cloned(),
            chunk: Rc::new(inner.chunk),
        });
        self.emit(Instruction::MakeFunction(idx));
        Ok(())
    }
    /// Compiles plain items directly. If any item splats, each ordinary item is
    /// wrapped as a one-item array and all segments are merged at runtime.
    fn items(&mut self, items: &[Item], make_array_for_plain: bool) -> Result<bool, Error> {
        if !items.iter().any(|item| matches!(item, Item::Splat(_))) {
            for item in items {
                let Item::Expr(expr) = item else {
                    unreachable!("checked for splats above");
                };
                self.expr(expr)?;
            }
            if make_array_for_plain {
                self.emit(Instruction::MakeArray(items.len()));
            }
            return Ok(false);
        }
        for item in items {
            match item {
                Item::Expr(expr) => {
                    self.expr(expr)?;
                    self.emit(Instruction::MakeArray(1));
                }
                Item::Splat(expr) => self.expr(expr)?,
            }
        }
        self.emit(Instruction::MergeArrays(items.len()));
        Ok(true)
    }
    fn short_circuit(&mut self, a: &Expr, b: &Expr, is_or: bool) -> Result<(), Error> {
        self.expr(a)?;
        let jump = self.emit(Instruction::JumpIfFalse(0));
        if is_or {
            let end = self.emit(Instruction::Jump(0));
            self.patch_jump(jump);
            self.emit(Instruction::Pop);
            self.expr(b)?;
            self.patch_jump(end);
        } else {
            self.emit(Instruction::Pop);
            self.expr(b)?;
            self.patch_jump(jump);
        }
        Ok(())
    }
    fn coalesce(&mut self, a: &Expr, b: &Expr) -> Result<(), Error> {
        self.expr(a)?;
        self.emit(Instruction::Dup);
        let fallback = self.emit(Instruction::JumpIfNil(0));
        self.emit(Instruction::Pop);
        let end = self.emit(Instruction::Jump(0));
        self.patch_jump(fallback);
        self.emit(Instruction::Pop);
        self.emit(Instruction::Pop);
        self.expr(b)?;
        self.patch_jump(end);
        Ok(())
    }
}
