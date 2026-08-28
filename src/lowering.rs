use crate::{
    ast::{
        Binary, ClassMember, Expr, Item, MapItem, Param, Pattern as AstPattern, Stmt, Unary, Update,
    },
    bytecode::{Chunk, Constant, Instruction, Pattern, RECEIVER_NAME},
    lexer::TokenSpan,
    vm::{Decimal, Error, Integer, Value},
};
use std::{collections::BTreeMap, rc::Rc};

#[derive(Debug, Default)]
pub(crate) struct ChunkSourceMap {
    pub(crate) instructions: Vec<u32>,
    pub(crate) spans: Vec<TokenSpan>,
}
impl ChunkSourceMap {
    fn span(&self, pc: usize) -> Option<crate::SourceSpan> {
        let span_id = *self.instructions.get(pc)?;
        if span_id == 0 {
            return None;
        }
        self.spans
            .get(span_id as usize - 1)
            .copied()
            .map(TokenSpan::into_source_span)
    }
}

#[derive(Debug, Default)]
pub(crate) struct CompiledSourceMap {
    pub(crate) top: ChunkSourceMap,
    pub(crate) nested: Vec<(Rc<Chunk>, ChunkSourceMap)>,
    pub(crate) instruction_count: usize,
}
impl CompiledSourceMap {
    fn span(&self, top_chunk: usize, chunk: usize, pc: usize) -> Option<crate::SourceSpan> {
        if chunk == top_chunk {
            return self.top.span(pc);
        }
        self.nested
            .iter()
            .find(|(nested, _)| Rc::as_ptr(nested) as usize == chunk)
            .and_then(|(_, source_map)| source_map.span(pc))
    }
}

pub(crate) fn verify_mapped(chunk: &Chunk, source_map: &CompiledSourceMap) -> Result<(), Error> {
    let top_chunk = chunk as *const Chunk as usize;
    chunk.verify().map_err(|error| {
        let span = error
            .verification_site()
            .and_then(|(failed_chunk, pc)| source_map.span(top_chunk, failed_chunk, pc));
        error.with_span_if_missing(span)
    })
}

fn lower(program: &[Stmt], record_source_map: bool) -> Result<Compiler, Error> {
    let mut c = Compiler {
        record_source_map,
        ..Default::default()
    };
    if program.is_empty() {
        c.emit_const(Value::Nil);
    } else {
        for (i, stmt) in program.iter().enumerate() {
            if i + 1 != program.len() {
                if !c.stmt_discard(stmt)? {
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
    Ok(c)
}

pub(crate) fn compile_mapped(program: &[Stmt]) -> Result<(Chunk, CompiledSourceMap), Error> {
    let c = lower(program, true)?;
    debug_assert_eq!(c.chunk.code.len(), c.instruction_spans.len());
    let instruction_count = c
        .nested_source_maps
        .iter()
        .fold(c.chunk.code.len(), |count, (_, source_map)| {
            count.saturating_add(source_map.instructions.len())
        });
    Ok((
        c.chunk,
        CompiledSourceMap {
            top: ChunkSourceMap {
                instructions: c.instruction_spans,
                spans: c.span_table,
            },
            nested: c.nested_source_maps,
            instruction_count,
        },
    ))
}

pub(crate) fn compile(program: &[Stmt]) -> Result<(Chunk, usize), Error> {
    lower(program, false).map(|compiler| {
        let instruction_count = crate::vm::count_new(&compiler.chunk);
        (compiler.chunk, instruction_count)
    })
}

/// Evaluates only side-effect-free literal expressions at compile time. A
/// failed conversion deliberately returns `None`, leaving the original
/// expression on the normal VM path so strict runtime errors remain intact.
fn constant_value(expression: &Expr) -> Option<Value> {
    match expression {
        Expr::Located(_, expression) => constant_value(expression),
        Expr::Number(value) => Some(Value::Number(*value)),
        Expr::Integer(digits, radix) => Some(Value::Integer(Rc::new(Integer::parse_radix(
            digits, *radix,
        )?))),
        Expr::Decimal(source) => Some(Value::Decimal(Rc::new(Decimal::parse(source)?))),
        Expr::String(value) => Some(Value::String(Rc::from(value.as_str()))),
        Expr::Bool(value) => Some(Value::Bool(*value)),
        Expr::Nil => Some(Value::Nil),
        Expr::Interpolate(parts) => {
            let mut output = String::new();
            for part in parts {
                let value = constant_value(part)?;
                match value {
                    Value::Integer(value) => output.push_str(&value.to_decimal_string()),
                    Value::Decimal(value) => output.push_str(&value.to_plain_string()),
                    value => output.push_str(&value.to_string()),
                }
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
            for entry in entries {
                let MapItem::Entry(key, value) = entry else {
                    return None;
                };
                values.insert(key.clone(), constant_value(value)?);
            }
            Some(Value::Map(Rc::new(values)))
        }
        Expr::Unary(operator, operand) => {
            let value = constant_value(operand)?;
            match operator {
                Unary::Neg => match value {
                    Value::Number(value) => Some(Value::Number(-value)),
                    Value::Integer(value) => Integer::from_bigint(-value.inner())
                        .ok()
                        .map(|value| Value::Integer(Rc::new(value))),
                    Value::Decimal(value) => Decimal::from_bigint(-value.inner(), value.scale())
                        .ok()
                        .map(|value| Value::Decimal(Rc::new(value))),
                    _ => None,
                },
                Unary::Not => Some(Value::Bool(!value.as_bool()?)),
                Unary::BitNot => match value {
                    Value::Integer(value) => Integer::from_bigint(!value.inner())
                        .ok()
                        .map(|value| Value::Integer(Rc::new(value))),
                    value => Some(Value::Number((!constant_bit_integer(&value)?) as f64)),
                },
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
    if let (Value::Integer(left), Value::Integer(right)) = (&left, &right) {
        let integer = |value| {
            Integer::from_bigint(value)
                .ok()
                .map(|value| Value::Integer(Rc::new(value)))
        };
        match operator {
            Binary::Add => return integer(left.inner() + right.inner()),
            Binary::Sub => return integer(left.inner() - right.inner()),
            Binary::Mul => return integer(left.inner() * right.inner()),
            Binary::BitAnd => return integer(left.inner() & right.inner()),
            Binary::BitOr => return integer(left.inner() | right.inner()),
            Binary::BitXor => return integer(left.inner() ^ right.inner()),
            Binary::Lt => return Some(Value::Bool(left < right)),
            Binary::Le => return Some(Value::Bool(left <= right)),
            Binary::Gt => return Some(Value::Bool(left > right)),
            Binary::Ge => return Some(Value::Bool(left >= right)),
            _ => {}
        }
    }
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
        (Value::Integer(left), Value::Integer(right)) => left == right,
        (Value::Decimal(left), Value::Decimal(right)) => left == right,
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
    instruction_spans: Vec<u32>,
    span_table: Vec<TokenSpan>,
    nested_source_maps: Vec<(Rc<Chunk>, ChunkSourceMap)>,
    current_span: u32,
    record_source_map: bool,
    loops: Vec<LoopContext>,
    in_function: bool,
    receiver_context: bool,
    super_context: bool,
    return_cleanups: Vec<ReturnCleanup>,
}
struct LoopContext {
    breaks: Vec<usize>,
    continue_target: usize,
    owns_iterator: bool,
}
#[derive(Clone, Copy, Default)]
struct FunctionContext {
    receiver: bool,
    super_context: bool,
    receiver_bound: bool,
    capture_receiver: bool,
}
#[derive(Clone)]
struct ReturnCleanup {
    end_try: bool,
    finalizer: Option<Expr>,
}
impl Compiler {
    fn parse_error(&self, message: impl Into<String>) -> Error {
        let error = Error::parse(message);
        if self.current_span == 0 {
            return error;
        }
        self.span_table
            .get(self.current_span as usize - 1)
            .copied()
            .map_or(error.clone(), |span| error.at_span(span.into_source_span()))
    }
    fn enter_span(&mut self, span: TokenSpan) -> u32 {
        let previous = self.current_span;
        if !self.record_source_map {
            return previous;
        }
        self.span_table.push(span);
        self.current_span = u32::try_from(self.span_table.len())
            .expect("source span table exceeds u32::MAX entries");
        previous
    }

    fn stmt_discard(&mut self, statement: &Stmt) -> Result<bool, Error> {
        let Stmt::Expr(expression) = statement else {
            return Ok(false);
        };
        let Expr::For(patterns, map, iterable, step, filter, body) = expression.unspanned() else {
            return Ok(false);
        };
        let previous = expression.span().map(|span| self.enter_span(span));
        let result = self.for_discard(
            patterns,
            *map,
            iterable,
            step.as_deref(),
            filter.as_deref(),
            body,
        );
        if let Some(previous) = previous {
            self.current_span = previous;
        }
        result?;
        Ok(true)
    }

    fn compile_pattern(&mut self, pattern: &AstPattern) -> Result<Pattern, Error> {
        Ok(match pattern {
            AstPattern::Ignore => Pattern::Ignore,
            AstPattern::Bind(name) => Pattern::Bind(name.clone()),
            AstPattern::Rest(name) => Pattern::Rest(name.clone()),
            AstPattern::Array(items) => Pattern::Array(
                items
                    .iter()
                    .map(|pattern| self.compile_pattern(pattern))
                    .collect::<Result<_, _>>()?,
            ),
            AstPattern::Map(fields) => Pattern::Map(
                fields
                    .iter()
                    .map(|(key, pattern)| Ok((key.clone(), self.compile_pattern(pattern)?)))
                    .collect::<Result<_, Error>>()?,
            ),
            AstPattern::MapRest(fields, rest) => Pattern::MapRest {
                fields: fields
                    .iter()
                    .map(|(key, pattern)| Ok((key.clone(), self.compile_pattern(pattern)?)))
                    .collect::<Result<_, Error>>()?,
                rest: rest.clone(),
            },
            AstPattern::Default(inner, expr) => {
                let mut compiler = Compiler {
                    record_source_map: self.record_source_map,
                    ..Default::default()
                };
                compiler.expr(expr)?;
                compiler.emit(Instruction::Return);
                let default = Rc::new(compiler.chunk);
                if self.record_source_map {
                    self.nested_source_maps.extend(compiler.nested_source_maps);
                    self.nested_source_maps.push((
                        default.clone(),
                        ChunkSourceMap {
                            instructions: compiler.instruction_spans,
                            spans: compiler.span_table,
                        },
                    ));
                }
                Pattern::Default {
                    pattern: Box::new(self.compile_pattern(inner)?),
                    default,
                }
            }
        })
    }
    fn emit(&mut self, op: Instruction) -> usize {
        let i = self.chunk.code.len();
        self.chunk.code.push(op);
        if self.record_source_map {
            self.instruction_spans.push(self.current_span);
        }
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
            Stmt::Assign(n, e, span) => {
                self.expr(e)?;
                let previous = self.enter_span(*span);
                self.emit(Instruction::Store(n.clone()));
                self.current_span = previous;
            }
            Stmt::Destructure(pattern, e, span) => {
                self.expr(e)?;
                let compiled = self.compile_pattern(pattern)?;
                let previous = self.enter_span(*span);
                self.emit(Instruction::Destructure(compiled));
                self.current_span = previous;
            }
            Stmt::Import(_, _, span)
            | Stmt::ExportAssign(_, _, span)
            | Stmt::ExportNames(_, span) => {
                return Err(
                    Error::verify("module directives require Engine::compile_module")
                        .at_span(span.into_source_span()),
                );
            }
            Stmt::Expr(e) => self.expr(e)?,
        }
        Ok(())
    }
    fn for_discard(
        &mut self,
        patterns: &[AstPattern],
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
            self.emit(Instruction::IterStartEnumerable);
        }
        let start = self.chunk.code.len();
        let compiled_patterns = patterns
            .iter()
            .map(|pattern| self.compile_pattern(pattern))
            .collect::<Result<_, _>>()?;
        let exit = self.emit(Instruction::IterNext {
            patterns: compiled_patterns,
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
        if let Expr::Located(span, expression) = e {
            let previous = self.enter_span(*span);
            let result = self.expr(expression);
            self.current_span = previous;
            return result;
        }
        if let Some(value) = constant_value(e) {
            self.emit_const(value);
            return Ok(());
        }
        match e {
            Expr::Located(..) => unreachable!("handled before constant folding"),
            Expr::Number(n) => self.emit_const(Value::Number(*n)),
            Expr::Integer(digits, radix) => {
                let value = Integer::parse_radix(digits, *radix).ok_or_else(|| {
                    Error::parse("invalid integer literal or integer exceeds the size limit")
                })?;
                self.emit_const(Value::Integer(Rc::new(value)));
            }
            Expr::Decimal(source) => {
                let value = Decimal::parse(source).ok_or_else(|| {
                    Error::parse("invalid decimal literal or decimal exceeds its limits")
                })?;
                self.emit_const(Value::Decimal(Rc::new(value)));
            }
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
            Expr::This => {
                if !self.receiver_context {
                    return Err(self.parse_error("this and @ are valid only in class members"));
                }
                self.emit(Instruction::LoadReceiver);
            }
            Expr::Assign(n, value) => {
                self.expr(value)?;
                self.emit(Instruction::Store(n.clone()));
            }
            Expr::AssignMember(target, name, value) => {
                if !self.receiver_context || !matches!(target.unspanned(), Expr::This) {
                    return Err(self.parse_error(
                        "member assignment is allowed only through this or @ in a class member",
                    ));
                }
                self.expr(value)?;
                self.emit(Instruction::SetMember(name.clone()));
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
                self.emit(match update {
                    Update::Increment => Instruction::Increment,
                    Update::Decrement => Instruction::Decrement,
                });
                self.emit(Instruction::Store(name.clone()));
                if !prefix {
                    self.emit(Instruction::Pop);
                }
            }
            Expr::Destructure(pattern, value) => {
                self.expr(value)?;
                let compiled = self.compile_pattern(pattern)?;
                self.emit(Instruction::Destructure(compiled));
            }
            Expr::Array(items) => {
                self.items(items, true)?;
            }
            Expr::Range(start, end, inclusive) => {
                self.expr(start)?;
                self.expr(end)?;
                self.emit(Instruction::MakeRange(*inclusive));
            }
            Expr::Map(items) => {
                let has_splat = items.iter().any(|item| matches!(item, MapItem::Splat(_)));
                if !has_splat {
                    let mut keys = Vec::with_capacity(items.len());
                    for item in items {
                        let MapItem::Entry(key, value) = item else {
                            unreachable!()
                        };
                        keys.push(key.clone());
                        self.expr(value)?;
                    }
                    self.emit(Instruction::MakeMap(keys));
                } else {
                    for item in items {
                        match item {
                            MapItem::Entry(key, value) => {
                                self.expr(value)?;
                                self.emit(Instruction::MakeMap(vec![key.clone()]));
                            }
                            MapItem::Splat(value) => self.expr(value)?,
                        }
                    }
                    self.emit(Instruction::MergeMaps(items.len()));
                }
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
                if let Expr::SoakMember(target, name) = fun.unspanned() {
                    self.expr(target)?;
                    self.emit(Instruction::Dup);
                    let nil = self.emit(Instruction::JumpIfNil(0));
                    self.emit(Instruction::Pop);
                    if self.items(args, false)? {
                        self.emit(Instruction::MemberCallSpread(name.clone()));
                    } else {
                        self.emit(Instruction::MemberCall {
                            name: name.clone(),
                            count: args.len(),
                        });
                    }
                    let end = self.emit(Instruction::Jump(0));
                    self.patch_jump(nil);
                    self.emit(Instruction::Pop);
                    self.patch_jump(end);
                } else if let Expr::Member(target, name) = fun.unspanned() {
                    self.expr(target)?;
                    if self.items(args, false)? {
                        self.emit(Instruction::MemberCallSpread(name.clone()));
                    } else {
                        self.emit(Instruction::MemberCall {
                            name: name.clone(),
                            count: args.len(),
                        });
                    }
                } else {
                    self.expr(fun)?;
                    if self.items(args, false)? {
                        self.emit(Instruction::CallSpread);
                    } else {
                        self.emit(Instruction::Call(args.len()));
                    }
                }
            }
            Expr::New(class, args) => {
                self.expr(class)?;
                if self.items(args, false)? {
                    self.emit(Instruction::ConstructSpread);
                } else {
                    self.emit(Instruction::Construct(args.len()));
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
                    self.emit(Instruction::IterStartEnumerable);
                }
                let start = self.chunk.code.len();
                let compiled_patterns = patterns
                    .iter()
                    .map(|pattern| self.compile_pattern(pattern))
                    .collect::<Result<_, _>>()?;
                let exit = self.emit(Instruction::IterNext {
                    patterns: compiled_patterns,
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
            Expr::Break(span) => {
                let owns_iterator = self.loops.last().ok_or_else(|| {
                    Error::parse("break outside of loop").at_span(span.into_source_span())
                })?;
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
            Expr::Continue(span) => {
                let target = self
                    .loops
                    .last()
                    .ok_or_else(|| {
                        Error::parse("continue outside of loop").at_span(span.into_source_span())
                    })?
                    .continue_target;
                self.emit_back_jump(target);
            }
            Expr::Return(value, span) => {
                if !self.in_function {
                    return Err(
                        Error::parse("return outside of function").at_span(span.into_source_span())
                    );
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
            Expr::BoundFunction(function, arrow_span) => {
                if !self.receiver_context {
                    return Err(Error::parse(
                        "receiver-binding => is valid only in a class receiver context",
                    )
                    .at_span(arrow_span.into_source_span()));
                }
                let Expr::Function(params, rest, body) = function.unspanned() else {
                    return Err(Error::verify("bound function wrapper has no function"));
                };
                self.function_inner(
                    params,
                    rest.as_ref(),
                    body,
                    FunctionContext {
                        receiver: true,
                        receiver_bound: true,
                        capture_receiver: true,
                        ..Default::default()
                    },
                )?;
            }
            Expr::Class(name, parent, members) => {
                self.class(name, parent.as_deref(), members)?;
                self.emit(Instruction::Store(name.clone()));
            }
            Expr::Super(items, span) => {
                if !self.super_context {
                    let error = Error::parse("super is valid only in members of a derived class")
                        .at_span(span.into_source_span());
                    return Err(error);
                }
                let spread = self.items(items, false)?;
                self.emit(if spread {
                    Instruction::SuperCallSpread
                } else {
                    Instruction::SuperCall(items.len())
                });
            }
            Expr::Block(statements) => {
                if statements.is_empty() {
                    self.emit_const(Value::Nil);
                } else {
                    for (index, statement) in statements.iter().enumerate() {
                        if index + 1 != statements.len() {
                            if !self.stmt_discard(statement)? {
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
                let forwarded = match function.unspanned() {
                    Expr::Function(params, rest, _) if rest.is_none() => params
                        .iter()
                        .map(|param| match &param.pattern {
                            AstPattern::Bind(name) if param.default.is_none() => Some(name.clone()),
                            _ => None,
                        })
                        .collect::<Option<Vec<_>>>(),
                    Expr::BoundFunction(function, _) => match function.unspanned() {
                        Expr::Function(params, rest, _) if rest.is_none() => params
                            .iter()
                            .map(|param| match &param.pattern {
                                AstPattern::Bind(name) if param.default.is_none() => {
                                    Some(name.clone())
                                }
                                _ => None,
                            })
                            .collect::<Option<Vec<_>>>(),
                        _ => Some(vec![]),
                    },
                    _ => Some(vec![]),
                };
                let names = forwarded.ok_or_else(|| {
                    Error::verify("do function forwarding requires plain required name parameters")
                })?;
                for name in &names {
                    self.emit(Instruction::Load(name.clone()));
                }
                self.emit(Instruction::Call(names.len()));
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
        self.function_inner(params, rest, body, FunctionContext::default())
    }
    fn class(
        &mut self,
        name: &str,
        parent: Option<&Expr>,
        members: &[ClassMember],
    ) -> Result<(), Error> {
        let constructor = members
            .iter()
            .find(|member| !member.is_static && member.name == "constructor");
        let instance_methods: Vec<_> = members
            .iter()
            .filter(|member| !member.is_static && member.name != "constructor")
            .collect();
        let static_methods: Vec<_> = members.iter().filter(|member| member.is_static).collect();
        if let Some(parent) = parent {
            self.expr(parent)?;
        }
        if let Some(constructor) = constructor {
            let previous = self.enter_span(constructor.span);
            self.function_inner(
                &constructor.params,
                constructor.rest.as_ref(),
                &constructor.body,
                FunctionContext {
                    receiver: true,
                    super_context: parent.is_some(),
                    receiver_bound: constructor.receiver_bound,
                    capture_receiver: false,
                },
            )?;
            self.current_span = previous;
        }
        for member in instance_methods.iter().chain(static_methods.iter()) {
            let previous = self.enter_span(member.span);
            self.function_inner(
                &member.params,
                member.rest.as_ref(),
                &member.body,
                FunctionContext {
                    receiver: true,
                    super_context: parent.is_some(),
                    receiver_bound: member.receiver_bound,
                    capture_receiver: false,
                },
            )?;
            self.current_span = previous;
        }
        self.emit(Instruction::MakeClass {
            name: name.to_owned(),
            extends: parent.is_some(),
            constructor: constructor.is_some(),
            instance_methods: instance_methods
                .iter()
                .map(|member| member.name.clone())
                .collect(),
            static_methods: static_methods
                .iter()
                .map(|member| member.name.clone())
                .collect(),
        });
        Ok(())
    }
    fn function_inner(
        &mut self,
        params: &[Param],
        rest: Option<&String>,
        body: &Expr,
        context: FunctionContext,
    ) -> Result<(), Error> {
        let mut inner = Compiler {
            in_function: true,
            receiver_context: context.receiver,
            super_context: context.super_context,
            record_source_map: self.record_source_map,
            ..Default::default()
        };
        for param in params {
            let Some(default) = &param.default else {
                continue;
            };
            let AstPattern::Bind(name) = &param.pattern else {
                return Err(Error::parse("default parameter must be a name"));
            };
            inner.emit(Instruction::Load(name.clone()));
            let use_default = inner.emit(Instruction::JumpIfNil(0));
            inner.emit(Instruction::Pop);
            let done = inner.emit(Instruction::Jump(0));
            inner.patch_jump(use_default);
            inner.emit(Instruction::Pop);
            let receiver_context = inner.receiver_context;
            inner.receiver_context = false;
            let result = inner.expr(default);
            inner.receiver_context = receiver_context;
            result?;
            inner.emit(Instruction::Store(name.clone()));
            inner.emit(Instruction::Pop);
            inner.patch_jump(done);
        }
        if context.receiver {
            for param in params.iter().filter(|param| param.receiver) {
                let AstPattern::Bind(name) = &param.pattern else {
                    return Err(Error::parse(
                        "receiver parameter shorthand requires a plain name",
                    ));
                };
                inner.emit(Instruction::Load(name.clone()));
                inner.emit(Instruction::SetMember(name.clone()));
                inner.emit(Instruction::Pop);
            }
        }
        inner.expr(body)?;
        inner.emit(Instruction::Return);
        let idx = self.chunk.constants.len();
        let mut compiled_params = Vec::with_capacity(params.len() + usize::from(context.receiver));
        if context.receiver {
            compiled_params.push(Pattern::Bind(RECEIVER_NAME.to_owned()));
        }
        compiled_params.extend(
            params
                .iter()
                .map(|param| inner.compile_pattern(&param.pattern))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let function_chunk = Rc::new(inner.chunk);
        if self.record_source_map {
            self.nested_source_maps.extend(inner.nested_source_maps);
            self.nested_source_maps.push((
                function_chunk.clone(),
                ChunkSourceMap {
                    instructions: inner.instruction_spans,
                    spans: inner.span_table,
                },
            ));
        }
        self.chunk.constants.push(Constant::Function {
            params: compiled_params,
            required: usize::from(context.receiver)
                + params
                    .iter()
                    .take_while(|param| param.default.is_none())
                    .count(),
            rest: rest.cloned(),
            receiver: context.receiver,
            receiver_bound: context.receiver_bound,
            chunk: function_chunk,
        });
        self.emit(if context.capture_receiver {
            Instruction::MakeBoundFunction(idx)
        } else {
            Instruction::MakeFunction(idx)
        });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorKind, parser};

    #[test]
    fn exact_integer_arithmetic_is_constant_folded() {
        let expression = Expr::Binary(
            Box::new(Expr::Integer("1".to_owned(), 10)),
            Binary::Add,
            Box::new(Expr::Integer("2".to_owned(), 10)),
        );
        let value = constant_value(&expression).expect("integer addition folds");
        assert_eq!(value.to_string(), "3n");
        let program = parser::parse("1n + 2n").unwrap();
        let (chunk, _) = compile(&program).unwrap();
        assert_eq!(chunk.code.len(), 2);
        assert!(!chunk.disassemble().contains("Add"));
        let public_chunk = crate::compile("1n + 2n").unwrap();
        assert_eq!(public_chunk.code.len(), 2);
    }

    #[test]
    fn emitted_instruction_count_covers_nested_functions_and_pattern_defaults() {
        let program =
            parser::parse("outer = (x = 1) ->\n  [y = 2] = []\n  -> x + y\nouter()()").unwrap();
        let (chunk, instruction_count) = compile(&program).unwrap();
        assert_eq!(instruction_count, crate::vm::count_bcs(&chunk));

        let (mapped_chunk, source_map) = compile_mapped(&program).unwrap();
        assert_eq!(
            source_map.instruction_count,
            crate::vm::count_bcs(&mapped_chunk)
        );
    }

    fn replace_mapped_chunk(
        source_map: &mut CompiledSourceMap,
        original: &Rc<Chunk>,
        replacement: Rc<Chunk>,
    ) {
        let (mapped, _) = source_map
            .nested
            .iter_mut()
            .find(|(mapped, _)| Rc::ptr_eq(mapped, original))
            .expect("compiled nested chunk has a source map");
        *mapped = replacement;
    }

    #[test]
    fn mapped_verification_errors_use_the_failing_top_level_instruction_span() {
        let ast = parser::parse("value = 1\nvalue").unwrap();
        let (mut chunk, source_map) = compile_mapped(&ast).unwrap();
        chunk.code[3] = Instruction::Pop;

        let error = verify_mapped(&chunk, &source_map).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Verify);
        assert_eq!(
            error.to_string(),
            "verify error: stack underflow at instruction 3"
        );
        let span = &error.labels()[0].span;
        assert_eq!(span.start.line, 2);
        assert_eq!(span.start.column, Some(1));

        let raw = chunk.verify().unwrap_err();
        assert!(raw.labels().is_empty());
    }

    #[test]
    fn mapped_verification_errors_follow_nested_function_and_default_chunks() {
        let ast = parser::parse("fail = ->\n  missing\nfail()").unwrap();
        let (mut chunk, mut source_map) = compile_mapped(&ast).unwrap();
        let Constant::Function { chunk: inner, .. } = &mut chunk.constants[0] else {
            panic!("function template is the first constant");
        };
        let original = inner.clone();
        let mut invalid = (**inner).clone();
        invalid.code[0] = Instruction::Pop;
        let replacement = Rc::new(invalid);
        *inner = replacement.clone();
        replace_mapped_chunk(&mut source_map, &original, replacement);

        let error = verify_mapped(&chunk, &source_map).unwrap_err();
        let span = &error.labels()[0].span;
        assert_eq!(span.start.line, 2);
        assert_eq!(span.start.column, Some(3));

        let ast = parser::parse("[value = missing] = []\n42").unwrap();
        let (mut chunk, mut source_map) = compile_mapped(&ast).unwrap();
        let default = chunk
            .code
            .iter_mut()
            .find_map(|instruction| match instruction {
                Instruction::Destructure(Pattern::Array(patterns)) => {
                    patterns.iter_mut().find_map(|pattern| match pattern {
                        Pattern::Default { default, .. } => Some(default),
                        _ => None,
                    })
                }
                _ => None,
            })
            .expect("destructuring pattern carries its compiled default chunk");
        let original = default.clone();
        let mut invalid = (**default).clone();
        invalid.code[0] = Instruction::Pop;
        let replacement = Rc::new(invalid);
        *default = replacement.clone();
        replace_mapped_chunk(&mut source_map, &original, replacement);

        let error = verify_mapped(&chunk, &source_map).unwrap_err();
        let span = &error.labels()[0].span;
        assert_eq!(span.start.line, 1);
        assert_eq!(span.start.column, Some(10));
    }
}
