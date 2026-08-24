use crate::{
    bytecode::{Chunk, Constant, Instruction, Pattern},
    compile,
    lowering::{self, ChunkSourceMap, CompiledSourceMap},
    parser,
};
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    fmt,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const MAX_RANGE_ITEMS: i128 = 1_000_000;
const MAX_REUSABLE_CALL_ARGUMENTS: usize = 16;

/// Stable type tag for values crossing the embedding boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    /// The sole empty value.
    Nil,
    /// A strict boolean.
    Bool,
    /// An IEEE-754 number.
    Number,
    /// An immutable UTF-8 string.
    String,
    /// An immutable array.
    Array,
    /// An immutable string-keyed map.
    Map,
    /// An opaque bytecode or native function.
    Function,
}

/// An immutable value crossing the QuickCoffee/host boundary.
#[derive(Clone)]
pub enum Value {
    /// The sole empty value.
    Nil,
    /// A strict boolean.
    Bool(bool),
    /// An IEEE-754 number used by the language.
    Number(f64),
    /// An immutable UTF-8 string.
    String(Rc<str>),
    /// An immutable array of values.
    Array(Rc<Vec<Value>>),
    /// An immutable map with string keys.
    Map(Rc<BTreeMap<String, Value>>),
    /// An opaque bytecode or native function.
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
    /// Returns a stable type tag without exposing the internal container representation.
    pub fn kind(&self) -> ValueKind {
        match self {
            Self::Nil => ValueKind::Nil,
            Self::Bool(_) => ValueKind::Bool,
            Self::Number(_) => ValueKind::Number,
            Self::String(_) => ValueKind::String,
            Self::Array(_) => ValueKind::Array,
            Self::Map(_) => ValueKind::Map,
            Self::Function(_) => ValueKind::Function,
        }
    }
    /// Returns whether this value is the language's `nil` value.
    pub fn is_nil(&self) -> bool {
        matches!(self, Self::Nil)
    }
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
    /// Returns the number, if this value is numeric.
    pub fn as_number(&self) -> Option<f64> {
        if let Self::Number(x) = self {
            Some(*x)
        } else {
            None
        }
    }
    /// Returns the boolean, if this value is boolean.
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(x) = self {
            Some(*x)
        } else {
            None
        }
    }
    /// Returns the UTF-8 view, if this value is a string.
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(x) = self {
            Some(x)
        } else {
            None
        }
    }
    /// Returns an immutable slice, if this value is an array.
    pub fn as_array(&self) -> Option<&[Value]> {
        if let Self::Array(values) = self {
            Some(values)
        } else {
            None
        }
    }
    /// Returns an immutable map view, if this value is a map.
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
    /// Lexing or parsing failed.
    Parse,
    /// Untrusted bytecode failed verification.
    Verify,
    /// Execution or a host callback failed.
    Runtime,
    /// Execution stopped because a configured resource boundary was reached.
    Resource,
}
/// Stable reason for a resource-boundary failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimit {
    /// The per-run instruction budget was exhausted.
    Fuel,
    /// A bytecode function call would exceed the configured nesting depth.
    CallDepth,
    /// The embedding host cancelled the current execution.
    Cancellation,
}
/// A source coordinate attached to a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    /// One-based line number.
    pub line: usize,
    /// One-based Unicode scalar column, or `None` when only the line is known.
    pub column: Option<usize>,
}
/// A half-open source range, or a line-only location when [`Self::end`] is absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    /// Opaque source name supplied by a CLI, module loader, or embedding host.
    pub source_name: Option<String>,
    /// Inclusive start coordinate.
    pub start: SourcePosition,
    /// Exclusive end coordinate, or `None` when the diagnostic is line-only.
    pub end: Option<SourcePosition>,
}
/// Role of a source label in a structured diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLabelKind {
    /// The source range that caused the error.
    Primary,
    /// A related source range that provides additional context.
    Secondary,
}
/// A display-independent source annotation attached to an [`Error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLabel {
    /// Whether this is the primary cause or related context.
    pub kind: DiagnosticLabelKind,
    /// Source range associated with this label.
    pub span: SourceSpan,
    /// Optional detail specific to this range, separate from [`Error::message`].
    pub message: Option<String>,
}
impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse => write!(f, "parse"),
            Self::Verify => write!(f, "verify"),
            Self::Runtime => write!(f, "runtime"),
            Self::Resource => write!(f, "resource"),
        }
    }
}
/// A structured error suitable for CLI display or host-side branching.
#[derive(Clone)]
pub struct Error {
    kind: ErrorKind,
    message: String,
    labels: Vec<DiagnosticLabel>,
    resource_limit: Option<ResourceLimit>,
    verification_site: Option<VerificationSite>,
}
#[derive(Debug, Clone, Copy)]
struct VerificationSite {
    chunk: Option<usize>,
    instruction: usize,
}
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("kind", &self.kind)
            .field("message", &self.message)
            .field("labels", &self.labels)
            .field("resource_limit", &self.resource_limit)
            .finish()
    }
}
impl Error {
    pub(crate) fn parse(m: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Parse,
            message: m.into(),
            labels: Vec::new(),
            resource_limit: None,
            verification_site: None,
        }
    }
    pub(crate) fn verify(m: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Verify,
            message: m.into(),
            labels: Vec::new(),
            resource_limit: None,
            verification_site: None,
        }
    }
    /// Creates a runtime error for a host callback to return across the VM boundary.
    pub fn runtime(m: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Runtime,
            message: m.into(),
            labels: Vec::new(),
            resource_limit: None,
            verification_site: None,
        }
    }
    fn resource(limit: ResourceLimit, message: impl Into<String>) -> Self {
        Self {
            kind: ErrorKind::Resource,
            message: message.into(),
            labels: Vec::new(),
            resource_limit: Some(limit),
            verification_site: None,
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
    /// Returns the primary label's start coordinate when the compiler knows it.
    ///
    /// This compatibility accessor returns the same line as before structured
    /// labels were introduced. Call [`Self::labels`] to inspect complete ranges.
    pub fn position(&self) -> Option<SourcePosition> {
        self.labels
            .iter()
            .find(|label| label.kind == DiagnosticLabelKind::Primary)
            .map(|label| label.span.start)
    }
    /// Returns ordered, display-independent source annotations.
    pub fn labels(&self) -> &[DiagnosticLabel] {
        &self.labels
    }
    /// Returns the crossed resource boundary for a resource error.
    pub fn resource_limit(&self) -> Option<ResourceLimit> {
        self.resource_limit
    }
    pub(crate) fn at_line(mut self, line: usize) -> Self {
        self.labels = vec![DiagnosticLabel {
            kind: DiagnosticLabelKind::Primary,
            span: SourceSpan {
                source_name: None,
                start: SourcePosition { line, column: None },
                end: None,
            },
            message: None,
        }];
        self
    }
    pub(crate) fn at_span(mut self, span: SourceSpan) -> Self {
        self.labels = vec![DiagnosticLabel {
            kind: DiagnosticLabelKind::Primary,
            span,
            message: None,
        }];
        self
    }
    pub(crate) fn with_source_name(mut self, source_name: &str) -> Self {
        for label in &mut self.labels {
            if label.span.source_name.is_none() {
                label.span.source_name = Some(source_name.to_owned());
            }
        }
        self
    }
    pub(crate) fn with_span_if_missing(mut self, span: Option<SourceSpan>) -> Self {
        if self.labels.is_empty() {
            if let Some(span) = span {
                self = self.at_span(span);
            }
        }
        self
    }
    pub(crate) fn with_secondary_span(mut self, span: Option<SourceSpan>) -> Self {
        if let Some(span) = span {
            self.labels.push(DiagnosticLabel {
                kind: DiagnosticLabelKind::Secondary,
                span,
                message: Some("called from here".to_owned()),
            });
        }
        self
    }
    pub(crate) fn at_instruction(mut self, instruction: usize) -> Self {
        if self.verification_site.is_none() {
            self.verification_site = Some(VerificationSite {
                chunk: None,
                instruction,
            });
        }
        self
    }
    pub(crate) fn with_verification_chunk(mut self, chunk: usize) -> Self {
        if let Some(site) = &mut self.verification_site {
            if site.chunk.is_none() {
                site.chunk = Some(chunk);
            }
        }
        self
    }
    pub(crate) fn verification_site(&self) -> Option<(usize, usize)> {
        self.verification_site
            .and_then(|site| site.chunk.map(|chunk| (chunk, site.instruction)))
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.kind == ErrorKind::Parse {
            if let Some(position) = self.position() {
                return write!(
                    f,
                    "{} error (line {}): {}",
                    self.kind, position.line, self.message
                );
            }
        }
        write!(f, "{} error: {}", self.kind, self.message)
    }
}
impl std::error::Error for Error {}

/// A cloneable, one-way cancellation signal owned by the embedding host.
///
/// Clones share state. Cancelling a token causes the next VM instruction check
/// in every context configured with it to stop with [`ResourceLimit::Cancellation`].
#[derive(Clone, Debug, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    /// Creates an uncancelled token.
    pub fn new() -> Self {
        Self::default()
    }
    /// Requests cancellation for all contexts sharing this token.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    /// Reports whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// A host callback callable from QuickCoffee code.
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
        debug_info: Option<Rc<ProgramDebugInfo>>,
        execution_plan: Option<Rc<ProgramExecutionPlan>>,
        fast_parameters: Option<Vec<Option<usize>>>,
        env: Env,
    },
    Native {
        function: NativeFunction,
        allocation_profile: Option<fn(&Value) -> u64>,
    },
}
type Env = Rc<RefCell<Environment>>;
struct Environment {
    indices: BTreeMap<Rc<str>, usize>,
    slots: Vec<(Rc<str>, Value)>,
    initialized: Vec<bool>,
    parent: Option<Env>,
}
// Pattern binding is atomic, so both the name index and its stable slots must
// roll back together when a nested/default pattern fails.
#[derive(Clone)]
struct EnvironmentSnapshot {
    indices: BTreeMap<Rc<str>, usize>,
    slots: Vec<(Rc<str>, Value)>,
    initialized: Vec<bool>,
}
impl Environment {
    fn is_initialized(&self, slot: usize) -> bool {
        self.initialized.get(slot).copied().unwrap_or(true)
    }

    fn get_local(&self, name: &str) -> Option<Value> {
        let slot = *self.indices.get(name)?;
        self.is_initialized(slot)
            .then(|| self.slots.get(slot).map(|(_, value)| value.clone()))?
    }

    fn get_local_with_slot(&self, name: &str) -> Option<(usize, Value)> {
        let slot = *self.indices.get(name)?;
        self.is_initialized(slot)
            .then(|| self.slots.get(slot).map(|(_, value)| (slot, value.clone())))?
    }

    fn get_cached(&self, name: &str, slot: usize) -> Option<Value> {
        self.slots
            .get(slot)
            .filter(|(stored, _)| stored.as_ref() == name)
            .filter(|_| self.is_initialized(slot))
            .map(|(_, value)| value.clone())
    }

    fn get_resolved(&self, slot: usize) -> Option<Option<Value>> {
        let (_, value) = self.slots.get(slot)?;
        Some(self.is_initialized(slot).then(|| value.clone()))
    }

    fn set_resolved(&mut self, slot: usize, value: Value) -> Result<(), Value> {
        if slot >= self.slots.len() {
            return Err(value);
        }
        if let Some(initialized) = self.initialized.get_mut(slot) {
            *initialized = true;
        }
        self.slots[slot].1 = value;
        Ok(())
    }

    fn set_local(&mut self, name: &str, value: Value) -> usize {
        if let Some(slot) = self.indices.get(name).copied() {
            if let Some(initialized) = self.initialized.get_mut(slot) {
                *initialized = true;
            }
            self.slots[slot].1 = value;
            return slot;
        }
        let slot = self.slots.len();
        let name: Rc<str> = Rc::from(name);
        self.indices.insert(name.clone(), slot);
        self.slots.push((name, value));
        if !self.initialized.is_empty() {
            self.initialized.push(true);
        }
        slot
    }

    fn set_cached(&mut self, name: &str, slot: usize, value: Value) -> Result<(), Value> {
        if self
            .slots
            .get(slot)
            .is_none_or(|(stored, _)| stored.as_ref() != name)
        {
            return Err(value);
        }
        if let Some(initialized) = self.initialized.get_mut(slot) {
            *initialized = true;
        }
        self.slots[slot].1 = value;
        Ok(())
    }

    fn snapshot(&self) -> EnvironmentSnapshot {
        EnvironmentSnapshot {
            indices: self.indices.clone(),
            slots: self.slots.clone(),
            initialized: self.initialized.clone(),
        }
    }

    fn restore(&mut self, snapshot: EnvironmentSnapshot) {
        self.indices = snapshot.indices;
        self.slots = snapshot.slots;
        self.initialized = snapshot.initialized;
    }
}
fn env(parent: Option<Env>) -> Env {
    Rc::new(RefCell::new(Environment {
        indices: BTreeMap::new(),
        slots: vec![],
        initialized: vec![],
        parent,
    }))
}
fn env_with_unset_slots(parent: Env, names: &[Rc<str>]) -> Env {
    let indices = names
        .iter()
        .cloned()
        .enumerate()
        .map(|(slot, name)| (name, slot))
        .collect();
    let slots = names
        .iter()
        .cloned()
        .map(|name| (name, Value::Nil))
        .collect();
    Rc::new(RefCell::new(Environment {
        indices,
        slots,
        initialized: vec![false; names.len()],
        parent: Some(parent),
    }))
}
fn lookup(e: &Env, n: &str) -> Option<Value> {
    let b = e.borrow();
    if let Some(value) = b.get_local(n) {
        return Some(value);
    }
    let p = b.parent.clone();
    drop(b);
    p.and_then(|p| lookup(&p, n))
}

/// A reusable compiler that does not hold execution state.
#[derive(Clone, Default)]
pub struct Engine;
/// A reference-counted compiled program for repeated execution.
///
/// The shared storage is private so embedding callers do not need to manage
/// `Rc` themselves; cloning a `Program` is cheap and does not copy bytecode.
/// Programs produced by [`Engine::compile_program`] are verified immediately;
/// programs wrapped from a raw [`Chunk`] verify on their first execution.
#[derive(Debug)]
struct ProgramInner {
    chunk: Rc<Chunk>,
    verified: Cell<bool>,
    debug_info: Option<Rc<ProgramDebugInfo>>,
    execution_plan: Option<Rc<ProgramExecutionPlan>>,
}
#[derive(Debug)]
struct ProgramExecutionPlan {
    chunks: BTreeMap<usize, Rc<ChunkBindingSlots>>,
}
#[derive(Debug)]
struct ChunkBindingSlots {
    local_names: Vec<Rc<str>>,
    local_by_pc: Vec<Option<usize>>,
    isolated_frame: bool,
    shared_environment: bool,
    // Unresolved/global names retain the guarded hint path because Programs
    // are shared across Contexts with independently ordered environments.
    cached_by_pc: Vec<Cell<Option<usize>>>,
}
impl ChunkBindingSlots {
    fn fast_parameter_slots(
        &self,
        params: &[Pattern],
        required: usize,
        rest: Option<&str>,
    ) -> Option<Vec<Option<usize>>> {
        if !self.isolated_frame || required != params.len() || rest.is_some() {
            return None;
        }
        let mut slots = Vec::with_capacity(params.len());
        for pattern in params {
            match pattern {
                Pattern::Bind(name) => slots.push(Some(
                    self.local_names
                        .binary_search_by(|candidate| candidate.as_ref().cmp(name))
                        .ok()?,
                )),
                Pattern::Ignore => slots.push(None),
                _ => return None,
            }
        }
        Some(slots)
    }
}
impl ProgramExecutionPlan {
    fn new(chunk: &Rc<Chunk>) -> Self {
        let mut chunks = BTreeMap::new();
        Self::register_chunk(chunk, BTreeSet::new(), &mut chunks);
        Self { chunks }
    }

    fn register_chunk(
        chunk: &Rc<Chunk>,
        mut local_names: BTreeSet<String>,
        chunks: &mut BTreeMap<usize, Rc<ChunkBindingSlots>>,
    ) {
        let key = Rc::as_ptr(chunk) as usize;
        if chunks.contains_key(&key) {
            return;
        }
        for instruction in &chunk.code {
            match instruction {
                Instruction::Store(name) => {
                    local_names.insert(name.clone());
                }
                Instruction::Try { name, .. } => {
                    if name != "_" {
                        local_names.insert(name.clone());
                    }
                }
                Instruction::Destructure(pattern) => {
                    Self::collect_pattern_bindings(pattern, &mut local_names)
                }
                Instruction::IterNext { patterns, .. } => {
                    for pattern in patterns {
                        Self::collect_pattern_bindings(pattern, &mut local_names);
                    }
                }
                _ => {}
            }
        }
        let local_names = local_names.into_iter().map(Rc::from).collect::<Vec<_>>();
        let local_indices = local_names
            .iter()
            .cloned()
            .enumerate()
            .map(|(slot, name)| (name, slot))
            .collect::<BTreeMap<Rc<str>, usize>>();
        let local_by_pc = chunk
            .code
            .iter()
            .map(|instruction| match instruction {
                Instruction::Load(name)
                | Instruction::LoadOrNil(name)
                | Instruction::Store(name) => local_indices.get(name.as_str()).copied(),
                _ => None,
            })
            .collect();
        // A direct call cannot observe its caller's lexical frame: bytecode
        // functions carry the environment captured where they were created,
        // and native functions receive values only. Keep spread calls out of
        // this first extension while their argument carrier is investigated.
        let shared_environment = chunk
            .code
            .iter()
            .any(|instruction| matches!(instruction, Instruction::MakeFunction(_)));
        let isolated_frame = !chunk.code.iter().any(|instruction| {
            matches!(
                instruction,
                Instruction::Destructure(_)
                    | Instruction::IterNext { .. }
                    | Instruction::Try { .. }
                    | Instruction::EndTry
                    | Instruction::MakeFunction(_)
                    | Instruction::CallSpread
            )
        });
        chunks.insert(
            key,
            Rc::new(ChunkBindingSlots {
                local_names,
                local_by_pc,
                isolated_frame,
                shared_environment,
                cached_by_pc: (0..chunk.code.len()).map(|_| Cell::new(None)).collect(),
            }),
        );
        for constant in &chunk.constants {
            if let Constant::Function {
                params,
                rest,
                chunk,
                ..
            } = constant
            {
                let mut function_locals = BTreeSet::new();
                for pattern in params {
                    Self::collect_pattern_bindings(pattern, &mut function_locals);
                    Self::register_pattern(pattern, chunks);
                }
                if let Some(rest) = rest {
                    function_locals.insert(rest.clone());
                }
                Self::register_chunk(chunk, function_locals, chunks);
            }
        }
        for instruction in &chunk.code {
            match instruction {
                Instruction::Destructure(pattern) => Self::register_pattern(pattern, chunks),
                Instruction::IterNext { patterns, .. } => {
                    for pattern in patterns {
                        Self::register_pattern(pattern, chunks);
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_pattern_bindings(pattern: &Pattern, names: &mut BTreeSet<String>) {
        match pattern {
            Pattern::Bind(name) | Pattern::Rest(name) => {
                if name != "_" {
                    names.insert(name.clone());
                }
            }
            Pattern::Default { pattern, .. } => Self::collect_pattern_bindings(pattern, names),
            Pattern::Array(patterns) => {
                for pattern in patterns {
                    Self::collect_pattern_bindings(pattern, names);
                }
            }
            Pattern::Map(fields) => {
                for (_, pattern) in fields {
                    Self::collect_pattern_bindings(pattern, names);
                }
            }
            Pattern::MapRest { fields, rest } => {
                for (_, pattern) in fields {
                    Self::collect_pattern_bindings(pattern, names);
                }
                if rest != "_" {
                    names.insert(rest.clone());
                }
            }
            Pattern::Ignore => {}
        }
    }

    fn register_pattern(pattern: &Pattern, chunks: &mut BTreeMap<usize, Rc<ChunkBindingSlots>>) {
        match pattern {
            Pattern::Default { pattern, default } => {
                Self::register_pattern(pattern, chunks);
                Self::register_chunk(default, BTreeSet::new(), chunks);
            }
            Pattern::Array(patterns) => {
                for pattern in patterns {
                    Self::register_pattern(pattern, chunks);
                }
            }
            Pattern::Map(fields) | Pattern::MapRest { fields, .. } => {
                for (_, pattern) in fields {
                    Self::register_pattern(pattern, chunks);
                }
            }
            Pattern::Ignore | Pattern::Bind(_) | Pattern::Rest(_) => {}
        }
    }

    fn slots(&self, chunk: &Rc<Chunk>) -> Option<Rc<ChunkBindingSlots>> {
        self.chunks
            .get(&(Rc::as_ptr(chunk) as usize))
            .map(Rc::clone)
    }
}
#[derive(Debug)]
struct ProgramDebugInfo {
    source_name: Option<Rc<str>>,
    instruction_spans: BTreeMap<usize, ChunkSourceMap>,
}
impl ProgramDebugInfo {
    fn new(chunk: &Rc<Chunk>, source_map: CompiledSourceMap, source_name: Option<&str>) -> Self {
        let mut instruction_spans = BTreeMap::new();
        instruction_spans.insert(Rc::as_ptr(chunk) as usize, source_map.top);
        for (nested, source_map) in source_map.nested {
            instruction_spans.insert(Rc::as_ptr(&nested) as usize, source_map);
        }
        Self {
            source_name: source_name.map(Rc::from),
            instruction_spans,
        }
    }
    fn span(&self, chunk: &Rc<Chunk>, pc: usize) -> Option<SourceSpan> {
        let source_map = self.instruction_spans.get(&(Rc::as_ptr(chunk) as usize))?;
        let span_id = *source_map.instructions.get(pc)?;
        if span_id == 0 {
            return None;
        }
        let span = *source_map.spans.get(span_id as usize - 1)?;
        let mut span = span.into_source_span();
        span.source_name = self.source_name.as_deref().map(str::to_owned);
        Some(span)
    }
}
/// A cheaply cloneable, verified bytecode program for repeated execution.
#[derive(Clone, Debug)]
pub struct Program(Rc<ProgramInner>);
impl From<Chunk> for Program {
    fn from(chunk: Chunk) -> Self {
        Self(Rc::new(ProgramInner {
            chunk: Rc::new(chunk),
            verified: Cell::new(false),
            debug_info: None,
            execution_plan: None,
        }))
    }
}
impl Program {
    pub(crate) fn from_compiled(
        chunk: Chunk,
        source_map: CompiledSourceMap,
        source_name: Option<&str>,
    ) -> Self {
        let chunk = Rc::new(chunk);
        let debug_info = Rc::new(ProgramDebugInfo::new(&chunk, source_map, source_name));
        let execution_plan = Rc::new(ProgramExecutionPlan::new(&chunk));
        Self(Rc::new(ProgramInner {
            chunk,
            verified: Cell::new(true),
            debug_info: Some(debug_info),
            execution_plan: Some(execution_plan),
        }))
    }
    #[cfg(test)]
    fn without_binding_slots(&self) -> Self {
        Self(Rc::new(ProgramInner {
            chunk: Rc::clone(&self.0.chunk),
            verified: Cell::new(self.0.verified.get()),
            debug_info: self.0.debug_info.clone(),
            execution_plan: None,
        }))
    }
    /// Verifies the program and caches a successful result.
    pub fn verify(&self) -> Result<(), Error> {
        let result = self.0.chunk.verify();
        if result.is_ok() {
            self.0.verified.set(true);
        }
        result
    }
    /// Returns a human-readable disassembly of the shared bytecode.
    pub fn disassemble(&self) -> String {
        self.0.chunk.disassemble()
    }
    /// Returns the deterministic fingerprint of the shared bytecode.
    pub fn fingerprint(&self) -> u64 {
        self.0.chunk.fingerprint()
    }
    fn ensure_verified(&self) -> Result<(), Error> {
        if self.0.verified.get() {
            Ok(())
        } else {
            self.verify()
        }
    }
}
impl Engine {
    /// Creates a stateless compiler.
    pub fn new() -> Self {
        Self
    }
    /// Compiles and verifies source into an owned bytecode chunk.
    pub fn compile(&self, source: &str) -> Result<Chunk, Error> {
        compile(source)
    }
    /// Compiles and verifies source while attaching an opaque host-provided
    /// name to any source labels produced on failure.
    pub fn compile_named(&self, source_name: &str, source: &str) -> Result<Chunk, Error> {
        crate::compile_named(source_name, source)
    }
    /// Compiles source into cheaply cloneable shared bytecode.
    pub fn compile_program(&self, source: &str) -> Result<Program, Error> {
        self.compile_program_source(None, source)
    }
    /// Compiles named source into cheaply cloneable shared bytecode.
    pub fn compile_program_named(&self, source_name: &str, source: &str) -> Result<Program, Error> {
        self.compile_program_source(Some(source_name), source)
    }
    /// Compiles and verifies source without executing it, collecting every
    /// parser error recoverable at a top-level statement boundary.
    ///
    /// Lexing, lowering, and verification stop at their first error. Existing
    /// [`Self::compile_program`] methods retain their first-error behavior.
    pub fn check_program(&self, source: &str) -> Result<(), Vec<Error>> {
        self.check_program_source(None, source)
    }
    /// Like [`Self::check_program`], while attaching the caller-provided
    /// opaque source name to every returned diagnostic label.
    pub fn check_program_named(&self, source_name: &str, source: &str) -> Result<(), Vec<Error>> {
        self.check_program_source(Some(source_name), source)
    }
    fn compile_program_source(
        &self,
        source_name: Option<&str>,
        source: &str,
    ) -> Result<Program, Error> {
        let attach_name = |error: Error| match source_name {
            Some(source_name) => error.with_source_name(source_name),
            None => error,
        };
        let ast = parser::parse(source).map_err(attach_name)?;
        let (chunk, source_map) = lowering::compile_mapped(&ast).map_err(attach_name)?;
        lowering::verify_mapped(&chunk, &source_map).map_err(attach_name)?;
        Ok(Program::from_compiled(chunk, source_map, source_name))
    }
    fn check_program_source(
        &self,
        source_name: Option<&str>,
        source: &str,
    ) -> Result<(), Vec<Error>> {
        let attach_name = |error: Error| match source_name {
            Some(source_name) => error.with_source_name(source_name),
            None => error,
        };
        let ast = parser::parse_recover(source)
            .map_err(|errors| errors.into_iter().map(attach_name).collect::<Vec<Error>>())?;
        let (chunk, source_map) =
            lowering::compile_mapped(&ast).map_err(|error| vec![attach_name(error)])?;
        lowering::verify_mapped(&chunk, &source_map).map_err(|error| vec![attach_name(error)])?;
        Ok(())
    }
}
/// Public counters for the most recent bytecode execution in a context.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExecutionStats {
    /// Number of VM instructions attempted, including the instruction that
    /// produced a runtime error.
    pub instructions: u64,
    /// Fuel left after the execution stopped.
    pub fuel_remaining: u64,
    /// Greatest nested QuickCoffee function-call depth reached during the run.
    ///
    /// The top-level program does not count toward this value.
    pub call_depth_peak: usize,
    /// Bytecode name loads attempted during the run.
    pub name_loads: u64,
    /// Bytecode name stores attempted during the run.
    pub name_stores: u64,
    /// Bytecode call instructions attempted during the run.
    pub calls: u64,
    /// Bytecode container construction and access instructions attempted during the run.
    pub container_ops: u64,
    /// Bytecode iterator setup, advancement, and cleanup instructions attempted during the run.
    pub iterator_ops: u64,
    /// Bytecode exception-handler and throw instructions attempted during the run.
    pub exception_ops: u64,
    /// Fresh reference-counted value backings created during the run.
    ///
    /// This includes VM and standard-library strings, arrays, maps, and bytecode
    /// functions, but excludes compile-time constants and values allocated by
    /// embedding-host callbacks.
    pub value_allocations: u64,
    /// Lexical environments allocated for QuickCoffee function calls during the run.
    pub environment_allocations: u64,
}
/// An execution context containing globals, builtins, and per-run resource limits.
pub struct Context {
    engine: Engine,
    global: Env,
    fuel: u64,
    max_call_depth: usize,
    cancellation: Option<CancellationToken>,
    last_execution: ExecutionStats,
}

thread_local! {
    // Native builtin functions are immutable. Sharing their template avoids
    // rebuilding names and closures while each Context still owns its slots.
    static BUILTIN_ENVIRONMENT: EnvironmentSnapshot = {
        let global = env(None);
        let mut context = Context {
            engine: Engine::new(),
            global: global.clone(),
            fuel: 1_000_000,
            max_call_depth: 1_024,
            cancellation: None,
            last_execution: ExecutionStats::default(),
        };
        context.install_builtins();
        global.borrow().snapshot()
    };
    // Keeping the pool outside `Context`, `Vm`, and `Vm::run` preserves the
    // layout of unrelated dispatch paths. Borrows never cross a script call.
    static REUSABLE_CALL_ARGUMENTS: RefCell<Vec<Value>> = const { RefCell::new(Vec::new()) };
}

fn one_value_allocation(_: &Value) -> u64 {
    1
}

fn array_and_element_allocations(value: &Value) -> u64 {
    match value {
        Value::Array(values) => values.len() as u64 + 1,
        _ => 0,
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
impl Context {
    /// Creates a context with standard library builtins and the default fuel budget.
    pub fn new() -> Self {
        let global = env(None);
        BUILTIN_ENVIRONMENT.with(|builtins| {
            global.borrow_mut().restore(builtins.clone());
        });
        Self {
            engine: Engine::new(),
            global,
            fuel: 1_000_000,
            max_call_depth: 1_024,
            cancellation: None,
            last_execution: ExecutionStats::default(),
        }
    }
    /// Returns a builder-style context with the supplied fuel budget.
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.set_fuel(fuel);
        self
    }
    /// Sets the instruction budget used by the next and subsequent runs.
    ///
    /// A context keeps its globals and registered native functions, so an
    /// embedding host can adjust a budget between runs without rebuilding it.
    pub fn set_fuel(&mut self, fuel: u64) {
        self.fuel = fuel;
    }
    /// Returns the instruction budget configured for each new run.
    pub fn fuel(&self) -> u64 {
        self.fuel
    }
    /// Returns this context with a maximum nested QuickCoffee function-call depth.
    ///
    /// A value of zero permits top-level code but rejects every bytecode function
    /// call. Native host callbacks do not add a QuickCoffee call frame.
    pub fn with_max_call_depth(mut self, max_call_depth: usize) -> Self {
        self.set_max_call_depth(max_call_depth);
        self
    }
    /// Sets the maximum nested QuickCoffee function-call depth for future runs.
    pub fn set_max_call_depth(&mut self, max_call_depth: usize) {
        self.max_call_depth = max_call_depth;
    }
    /// Returns the maximum nested QuickCoffee function-call depth for each run.
    pub fn max_call_depth(&self) -> usize {
        self.max_call_depth
    }
    /// Returns this context configured to observe an embedding-host cancellation token.
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.set_cancellation_token(token);
        self
    }
    /// Sets or replaces the cancellation token observed by future runs.
    pub fn set_cancellation_token(&mut self, token: CancellationToken) {
        self.cancellation = Some(token);
    }
    /// Removes the configured cancellation token from future runs.
    pub fn clear_cancellation_token(&mut self) {
        self.cancellation = None;
    }
    /// Returns counters from the most recent successful or failed execution.
    /// Compilation and verification errors do not replace the previous record.
    pub fn last_execution(&self) -> ExecutionStats {
        self.last_execution
    }
    /// Installs or replaces an immutable global value visible to later runs.
    pub fn set_global(&mut self, name: impl Into<String>, value: Value) {
        let name = name.into();
        self.global.borrow_mut().set_local(&name, value);
    }
    /// Returns this context after installing an immutable global value.
    ///
    /// This builder-style form is equivalent to [`Context::set_global`] and
    /// is convenient when configuring an embedding context inline.
    pub fn with_global(mut self, name: impl Into<String>, value: Value) -> Self {
        self.set_global(name, value);
        self
    }
    /// Reads a global value without exposing the VM environment or running code.
    pub fn get_global(&self, name: &str) -> Option<Value> {
        lookup(&self.global, name)
    }
    /// Registers a host callback as an opaque callable global.
    pub fn add_native<F>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(&[Value]) -> Result<Value, Error> + 'static,
    {
        self.set_global(
            name,
            Value::Function(Rc::new(Function {
                inner: FunctionKind::Native {
                    function: Rc::new(f),
                    allocation_profile: None,
                },
            })),
        );
    }
    fn add_builtin<F>(
        &mut self,
        name: impl Into<String>,
        f: F,
        allocation_profile: fn(&Value) -> u64,
    ) where
        F: Fn(&[Value]) -> Result<Value, Error> + 'static,
    {
        self.set_global(
            name,
            Value::Function(Rc::new(Function {
                inner: FunctionKind::Native {
                    function: Rc::new(f),
                    allocation_profile: Some(allocation_profile),
                },
            })),
        );
    }
    /// Returns this context after registering a host callback as a global.
    ///
    /// This builder-style form is equivalent to [`Context::add_native`].
    pub fn with_native<F>(mut self, name: impl Into<String>, f: F) -> Self
    where
        F: Fn(&[Value]) -> Result<Value, Error> + 'static,
    {
        self.add_native(name, f);
        self
    }
    /// Compiles, verifies, and executes source in this context.
    pub fn eval(&mut self, source: &str) -> Result<Value, Error> {
        let program = self.engine.compile_program(source)?;
        self.run_program(&program)
    }
    /// Compiles, verifies, and executes source while attaching an opaque
    /// host-provided name to compile-time and runtime source labels.
    pub fn eval_named(&mut self, source_name: &str, source: &str) -> Result<Value, Error> {
        let program = self.engine.compile_program_named(source_name, source)?;
        self.run_program(&program)
    }
    /// Verifies and executes an owned bytecode chunk.
    pub fn run(&mut self, chunk: Chunk) -> Result<Value, Error> {
        self.run_program(&chunk.into())
    }
    /// Runs shared compiled bytecode without cloning its instruction stream.
    pub fn run_program(&mut self, program: &Program) -> Result<Value, Error> {
        program.ensure_verified()?;
        let mut vm = Vm {
            fuel: self.fuel,
            instructions: 0,
            max_call_depth: self.max_call_depth,
            call_depth: 0,
            call_depth_peak: 0,
            cancellation: self.cancellation.clone(),
            name_loads: 0,
            name_stores: 0,
            calls: 0,
            container_ops: 0,
            iterator_ops: 0,
            exception_ops: 0,
            value_allocations: 0,
            environment_allocations: 0,
            initial_debug_info: program.0.debug_info.clone(),
            execution_plan: program.0.execution_plan.clone(),
        };
        let result = vm.run(Rc::clone(&program.0.chunk), self.global.clone());
        self.last_execution = vm.stats();
        result
    }
    pub(crate) fn module_child(&self) -> Self {
        Self {
            engine: self.engine.clone(),
            global: env(Some(self.global.clone())),
            fuel: self.fuel,
            max_call_depth: self.max_call_depth,
            cancellation: self.cancellation.clone(),
            last_execution: ExecutionStats::default(),
        }
    }
    pub(crate) fn get_local(&self, name: &str) -> Option<Value> {
        self.global.borrow().get_local(name)
    }
    pub(crate) fn set_execution_stats(&mut self, stats: ExecutionStats) {
        self.last_execution = stats;
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
                Value::String(x) => x.chars().count(),
                Value::Array(x) => x.len(),
                Value::Map(x) => x.len(),
                _ => return Err(Error::runtime("len expects string, array, or map")),
            };
            Ok(Value::Number(n as f64))
        });
        self.add_builtin(
            "type",
            |xs| {
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
            },
            one_value_allocation,
        );
        self.add_builtin(
            "range",
            |xs| {
                if xs.len() != 2 {
                    return Err(Error::runtime("range expects two arguments"));
                }
                let (a, b) = numbers(xs)?;
                numeric_range(a, b, false)
            },
            one_value_allocation,
        );
        self.add_builtin(
            "str",
            |xs| {
                if xs.len() != 1 {
                    return Err(Error::runtime("str expects one argument"));
                }
                Ok(Value::String(Rc::from(xs[0].to_string())))
            },
            one_value_allocation,
        );
        self.add_native("abs", |xs| {
            if xs.len() != 1 {
                return Err(Error::runtime("abs expects one number"));
            }
            let value = number(xs[0].clone())?;
            if !value.is_finite() {
                return Err(Error::runtime("abs expects a finite number"));
            }
            Ok(Value::Number(value.abs()))
        });
        self.add_native("sum", |xs| {
            let values = numeric_array(xs, "sum")?;
            Ok(Value::Number(values.into_iter().sum()))
        });
        self.add_native("min", |xs| {
            let values = numeric_array(xs, "min")?;
            let Some(value) = values.into_iter().reduce(f64::min) else {
                return Err(Error::runtime("min expects a non-empty array"));
            };
            Ok(Value::Number(value))
        });
        self.add_native("max", |xs| {
            let values = numeric_array(xs, "max")?;
            let Some(value) = values.into_iter().reduce(f64::max) else {
                return Err(Error::runtime("max expects a non-empty array"));
            };
            Ok(Value::Number(value))
        });
        self.add_builtin(
            "keys",
            |xs| {
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
            },
            array_and_element_allocations,
        );
        self.add_builtin(
            "values",
            |xs| {
                if xs.len() != 1 {
                    return Err(Error::runtime("values expects one argument"));
                }
                let Value::Map(map) = &xs[0] else {
                    return Err(Error::runtime("values expects a map"));
                };
                Ok(Value::Array(Rc::new(map.values().cloned().collect())))
            },
            one_value_allocation,
        );
        self.add_builtin(
            "join",
            |xs| {
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
            },
            one_value_allocation,
        );
        self.add_builtin(
            "split",
            |xs| {
                if xs.len() != 2 {
                    return Err(Error::runtime("split expects string and separator"));
                }
                let Value::String(input) = &xs[0] else {
                    return Err(Error::runtime("split expects a string"));
                };
                let Value::String(separator) = &xs[1] else {
                    return Err(Error::runtime("split separator must be string"));
                };
                let parts: Vec<_> = input
                    .split(separator.as_ref())
                    .map(|part| Value::String(Rc::from(part)))
                    .collect();
                Ok(Value::Array(Rc::new(parts)))
            },
            array_and_element_allocations,
        );
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
    debug_info: Option<Rc<ProgramDebugInfo>>,
    execution_plan: Option<Rc<ProgramExecutionPlan>>,
    env: Env,
    bindings: FrameBindings,
}
enum FrameBindings {
    Raw,
    Guarded(Rc<ChunkBindingSlots>),
    Fast {
        slots: Rc<ChunkBindingSlots>,
        locals: Vec<Option<Value>>,
    },
    Shared(Rc<ChunkBindingSlots>),
}
fn lookup_environment(
    environment: &Env,
    cached: Option<&Cell<Option<usize>>>,
    name: &str,
) -> Option<Value> {
    let environment = environment.borrow();
    if let Some(slot) = cached.and_then(Cell::get) {
        if let Some(value) = environment.get_cached(name, slot) {
            return Some(value);
        }
    }
    if let Some((slot, value)) = environment.get_local_with_slot(name) {
        if let Some(cached) = cached {
            cached.set(Some(slot));
        }
        return Some(value);
    }
    let parent = environment.parent.clone();
    drop(environment);
    parent.and_then(|parent| lookup(&parent, name))
}
fn store_environment(
    environment: &Env,
    cached: Option<&Cell<Option<usize>>>,
    name: &str,
    value: Value,
) {
    let mut environment = environment.borrow_mut();
    if let Some(slot) = cached.and_then(Cell::get) {
        match environment.set_cached(name, slot, value) {
            Ok(()) => return,
            Err(value) => {
                let slot = environment.set_local(name, value);
                if let Some(cached) = cached {
                    cached.set(Some(slot));
                }
                return;
            }
        }
    }
    let slot = environment.set_local(name, value);
    if let Some(cached) = cached {
        cached.set(Some(slot));
    }
}
fn lookup_frame(frame: &Frame, pc: usize, name: &str) -> Option<Value> {
    match &frame.bindings {
        FrameBindings::Fast { slots, locals } => {
            if let Some(slot) = slots.local_by_pc.get(pc).copied().flatten() {
                if let Some(value) = locals.get(slot)?.clone() {
                    return Some(value);
                }
                let parent = frame.env.borrow().parent.clone();
                return parent.and_then(|parent| lookup(&parent, name));
            }
            lookup_environment(&frame.env, slots.cached_by_pc.get(pc), name)
        }
        FrameBindings::Guarded(slots) => {
            lookup_environment(&frame.env, slots.cached_by_pc.get(pc), name)
        }
        FrameBindings::Shared(slots) => {
            if let Some(slot) = slots.local_by_pc.get(pc).copied().flatten() {
                let environment = frame.env.borrow();
                match environment.get_resolved(slot) {
                    Some(Some(value)) => return Some(value),
                    Some(None) => {
                        let parent = environment.parent.clone();
                        drop(environment);
                        return parent.and_then(|parent| lookup(&parent, name));
                    }
                    None => drop(environment),
                }
            }
            lookup_environment(&frame.env, slots.cached_by_pc.get(pc), name)
        }
        FrameBindings::Raw => lookup_environment(&frame.env, None, name),
    }
}
fn store_frame(frame: &mut Frame, pc: usize, name: &str, value: Value) {
    match &mut frame.bindings {
        FrameBindings::Fast { slots, locals } => {
            if let Some(slot) = slots.local_by_pc.get(pc).copied().flatten() {
                locals[slot] = Some(value);
                return;
            }
            store_environment(&frame.env, slots.cached_by_pc.get(pc), name, value);
        }
        FrameBindings::Guarded(slots) => {
            store_environment(&frame.env, slots.cached_by_pc.get(pc), name, value);
        }
        FrameBindings::Shared(slots) => {
            if let Some(slot) = slots.local_by_pc.get(pc).copied().flatten() {
                if let Err(value) = frame.env.borrow_mut().set_resolved(slot, value) {
                    store_environment(&frame.env, slots.cached_by_pc.get(pc), name, value);
                }
                return;
            }
            store_environment(&frame.env, slots.cached_by_pc.get(pc), name, value);
        }
        FrameBindings::Raw => store_environment(&frame.env, None, name, value),
    }
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
        step: i64,
    },
    String {
        values: Rc<Vec<Value>>,
        position: usize,
        step: i64,
    },
    Map {
        entries: Vec<(String, Value)>,
        position: usize,
    },
}
struct Vm {
    fuel: u64,
    instructions: u64,
    max_call_depth: usize,
    call_depth: usize,
    call_depth_peak: usize,
    cancellation: Option<CancellationToken>,
    name_loads: u64,
    name_stores: u64,
    calls: u64,
    container_ops: u64,
    iterator_ops: u64,
    exception_ops: u64,
    value_allocations: u64,
    environment_allocations: u64,
    initial_debug_info: Option<Rc<ProgramDebugInfo>>,
    execution_plan: Option<Rc<ProgramExecutionPlan>>,
}
enum Step {
    Continue,
    Return(Value),
    Call { callee: Value, args: Vec<Value> },
}
impl Vm {
    fn source_span(frame: &Frame, pc: usize) -> Option<SourceSpan> {
        frame
            .debug_info
            .as_ref()
            .and_then(|debug_info| debug_info.span(&frame.chunk, pc))
    }

    fn with_call_stack(mut error: Error, frames: &[Frame], include_current_call: bool) -> Error {
        let skip = usize::from(!include_current_call);
        for frame in frames.iter().rev().skip(skip) {
            error = error.with_secondary_span(Self::source_span(frame, frame.pc.saturating_sub(1)));
        }
        error
    }

    fn record_value_allocations(&mut self, count: u64) {
        self.value_allocations = self.value_allocations.saturating_add(count);
    }

    fn record_environment_allocation(&mut self) {
        self.environment_allocations = self.environment_allocations.saturating_add(1);
    }

    // This boundary keeps argument-buffer plumbing out of the monolithic
    // instruction dispatch loop, where code layout affects unrelated workloads.
    #[inline(never)]
    fn direct_call(frame: &mut Frame, argument_count: usize) -> Result<Step, Error> {
        if frame.stack.len() < argument_count {
            return Err(Error::runtime("stack underflow"));
        }
        let mut args =
            REUSABLE_CALL_ARGUMENTS.with(|reusable| std::mem::take(&mut *reusable.borrow_mut()));
        args.clear();
        let argument_start = frame.stack.len() - argument_count;
        args.extend(frame.stack.drain(argument_start..));
        let callee = match pop(frame) {
            Ok(callee) => callee,
            Err(error) => {
                Self::recycle_call_arguments(args);
                return Err(error);
            }
        };
        Ok(Step::Call { callee, args })
    }

    fn recycle_call_arguments(mut args: Vec<Value>) {
        args.clear();
        REUSABLE_CALL_ARGUMENTS.with(|reusable| {
            let mut reusable = reusable.borrow_mut();
            if args.capacity() > MAX_REUSABLE_CALL_ARGUMENTS {
                if reusable.capacity() == 0 {
                    *reusable = Vec::with_capacity(MAX_REUSABLE_CALL_ARGUMENTS);
                }
            } else if args.capacity() > reusable.capacity() {
                *reusable = args;
            }
        });
    }

    fn record_profile(&mut self, instruction: &Instruction) {
        match instruction {
            Instruction::Load(_) | Instruction::LoadOrNil(_) => self.name_loads += 1,
            Instruction::Store(_) => self.name_stores += 1,
            Instruction::Call(_) | Instruction::CallSpread => self.calls += 1,
            Instruction::MakeArray(_)
            | Instruction::Append
            | Instruction::MergeArrays(_)
            | Instruction::MergeMaps(_)
            | Instruction::MakeRange(_)
            | Instruction::MakeMap(_)
            | Instruction::Index
            | Instruction::Slice(_)
            | Instruction::Member(_)
            | Instruction::Contains
            | Instruction::HasKey => self.container_ops += 1,
            Instruction::IterStartEnumerable
            | Instruction::IterStartMap
            | Instruction::IterNext { .. }
            | Instruction::IterEnd => self.iterator_ops += 1,
            Instruction::Try { .. } | Instruction::EndTry | Instruction::Throw => {
                self.exception_ops += 1
            }
            _ => {}
        }
    }
    fn eval_default(
        &mut self,
        chunk: Rc<Chunk>,
        env: Env,
        debug_info: Option<Rc<ProgramDebugInfo>>,
        execution_plan: Option<Rc<ProgramExecutionPlan>>,
    ) -> Result<Value, Error> {
        let mut nested = Vm {
            fuel: self.fuel,
            instructions: self.instructions,
            max_call_depth: self.max_call_depth,
            call_depth: self.call_depth,
            call_depth_peak: self.call_depth_peak,
            cancellation: self.cancellation.clone(),
            name_loads: self.name_loads,
            name_stores: self.name_stores,
            calls: self.calls,
            container_ops: self.container_ops,
            iterator_ops: self.iterator_ops,
            exception_ops: self.exception_ops,
            value_allocations: self.value_allocations,
            environment_allocations: self.environment_allocations,
            initial_debug_info: debug_info,
            execution_plan,
        };
        let result = nested.run(chunk, env);
        self.fuel = nested.fuel;
        self.instructions = nested.instructions;
        self.call_depth = nested.call_depth;
        self.call_depth_peak = nested.call_depth_peak;
        self.name_loads = nested.name_loads;
        self.name_stores = nested.name_stores;
        self.calls = nested.calls;
        self.container_ops = nested.container_ops;
        self.iterator_ops = nested.iterator_ops;
        self.exception_ops = nested.exception_ops;
        self.value_allocations = nested.value_allocations;
        self.environment_allocations = nested.environment_allocations;
        result
    }
    fn stats(&self) -> ExecutionStats {
        ExecutionStats {
            instructions: self.instructions,
            fuel_remaining: self.fuel,
            call_depth_peak: self.call_depth_peak,
            name_loads: self.name_loads,
            name_stores: self.name_stores,
            calls: self.calls,
            container_ops: self.container_ops,
            iterator_ops: self.iterator_ops,
            exception_ops: self.exception_ops,
            value_allocations: self.value_allocations,
            environment_allocations: self.environment_allocations,
        }
    }
    fn run(&mut self, chunk: Rc<Chunk>, global: Env) -> Result<Value, Error> {
        let execution_plan = self.execution_plan.clone();
        let binding_slots = execution_plan.as_ref().and_then(|plan| plan.slots(&chunk));
        let mut frames = vec![Frame {
            chunk,
            pc: 0,
            stack: vec![],
            iterators: vec![],
            handlers: vec![],
            debug_info: self.initial_debug_info.clone(),
            execution_plan,
            env: global,
            bindings: binding_slots
                .map(FrameBindings::Guarded)
                .unwrap_or(FrameBindings::Raw),
        }];
        loop {
            if self
                .cancellation
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled)
            {
                let span = frames
                    .last()
                    .and_then(|frame| Self::source_span(frame, frame.pc));
                let error =
                    Error::resource(ResourceLimit::Cancellation, "execution cancelled by host")
                        .with_span_if_missing(span);
                return Err(Self::with_call_stack(error, &frames, false));
            }
            if self.fuel == 0 {
                let span = frames
                    .last()
                    .and_then(|frame| Self::source_span(frame, frame.pc));
                let error = Error::resource(ResourceLimit::Fuel, "execution fuel exhausted")
                    .with_span_if_missing(span);
                return Err(Self::with_call_stack(error, &frames, false));
            }
            self.fuel -= 1;
            self.instructions += 1;
            let step = (|| -> Result<Step, Error> {
                let frame = frames.last_mut().expect("VM has an initial frame");
                let instruction_pc = frame.pc;
                let chunk = frame.chunk.clone();
                let op = chunk
                    .code
                    .get(instruction_pc)
                    .ok_or_else(|| Error::runtime("instruction pointer escaped chunk"))?;
                frame.pc += 1;
                self.record_profile(op);
                match op {
                    Instruction::Constant(i) => match frame
                        .chunk
                        .constants
                        .get(*i)
                        .ok_or_else(|| Error::runtime("invalid constant"))?
                    {
                        Constant::Value(v) => frame.stack.push(v.clone()),
                        _ => {
                            return Err(Error::runtime("function template used as value constant"));
                        }
                    },
                    Instruction::Load(n) => frame.stack.push(
                        lookup_frame(frame, instruction_pc, n)
                            .ok_or_else(|| Error::runtime(format!("unknown name '{n}'")))?,
                    ),
                    Instruction::LoadOrNil(n) => frame
                        .stack
                        .push(lookup_frame(frame, instruction_pc, n).unwrap_or(Value::Nil)),
                    Instruction::Store(n) => {
                        let v = pop(frame)?;
                        store_frame(frame, instruction_pc, n, v.clone());
                        frame.stack.push(v)
                    }
                    Instruction::Destructure(pattern) => {
                        let value = pop(frame)?;
                        let mut bindings = vec![];
                        let env = frame.env.clone();
                        let snapshot = env.borrow().snapshot();
                        if let Err(error) = bind_pattern(
                            self,
                            pattern,
                            Some(&value),
                            &mut bindings,
                            &env,
                            frame.debug_info.as_ref(),
                            frame.execution_plan.as_ref(),
                        ) {
                            env.borrow_mut().restore(snapshot);
                            return Err(error);
                        }
                        let mut environment = frame.env.borrow_mut();
                        for (name, item) in bindings {
                            if name != "_" {
                                environment.set_local(&name, item);
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
                    Instruction::Jump(delta) => jump(frame, *delta)?,
                    Instruction::JumpIfFalse(delta) => {
                        if !truth(
                            frame
                                .stack
                                .last()
                                .cloned()
                                .ok_or_else(|| Error::runtime("stack underflow"))?,
                        )? {
                            jump(frame, *delta)?
                        }
                    }
                    Instruction::JumpIfNil(delta) => {
                        if matches!(frame.stack.last(), Some(Value::Nil)) {
                            jump(frame, *delta)?
                        }
                    }
                    Instruction::Try { catch, name } => frame.handlers.push(Handler {
                        catch_pc: (frame.pc as i64 + *catch as i64)
                            .try_into()
                            .map_err(|_| Error::runtime("invalid catch target"))?,
                        stack_depth: frame.stack.len(),
                        iterator_depth: frame.iterators.len(),
                        name: name.clone(),
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
                    Instruction::IterStartEnumerable => {
                        let step = array_iteration_step(pop(frame)?)?;
                        match pop(frame)? {
                            Value::Array(values) => {
                                // Negative steps traverse from the final element so the
                                // optional index remains the actual array position.
                                let position = if step < 0 {
                                    values.len().saturating_sub(1)
                                } else {
                                    0
                                };
                                frame.iterators.push(Iteration {
                                    kind: IterationKind::Array {
                                        values,
                                        position,
                                        step,
                                    },
                                });
                            }
                            Value::String(value) => {
                                let values: Vec<_> = value
                                    .chars()
                                    .map(|character| Value::String(Rc::from(character.to_string())))
                                    .collect();
                                self.record_value_allocations(values.len() as u64 + 1);
                                frame.iterators.push(Iteration {
                                    kind: IterationKind::String {
                                        values: Rc::new(values),
                                        position: if step < 0 {
                                            value.chars().count().saturating_sub(1)
                                        } else {
                                            0
                                        },
                                        step,
                                    },
                                });
                            }
                            _ => {
                                return Err(Error::runtime(
                                    "for expects an array or string iterable",
                                ));
                            }
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
                                        advance_position(position, *step);
                                    }
                                    value
                                }
                                IterationKind::String {
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
                                        advance_position(position, *step);
                                    }
                                    value
                                }
                                IterationKind::Map { entries, position } => {
                                    let value = entries.get(*position).map(|(key, value)| {
                                        vec![Value::String(Rc::from(key.as_str())), value.clone()]
                                    });
                                    if value.is_some() {
                                        self.record_value_allocations(1);
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
                                for (pattern, value) in patterns.iter().zip(values) {
                                    if let Pattern::Bind(name) = pattern {
                                        environment.set_local(name, value);
                                    }
                                }
                            } else {
                                let mut bindings = vec![];
                                let snapshot = frame.env.borrow().snapshot();
                                for (pattern, value) in patterns.iter().zip(values.iter()) {
                                    let env = frame.env.clone();
                                    if let Err(error) = bind_pattern(
                                        self,
                                        pattern,
                                        Some(value),
                                        &mut bindings,
                                        &env,
                                        frame.debug_info.as_ref(),
                                        frame.execution_plan.as_ref(),
                                    ) {
                                        frame.env.borrow_mut().restore(snapshot);
                                        return Err(error);
                                    }
                                }
                                let mut environment = frame.env.borrow_mut();
                                for (name, value) in bindings {
                                    environment.set_local(&name, value);
                                }
                            }
                        } else {
                            frame.iterators.pop();
                            jump(frame, *end)?;
                        }
                    }
                    Instruction::IterEnd => {
                        frame
                            .iterators
                            .pop()
                            .ok_or_else(|| Error::runtime("iterator stack underflow"))?;
                    }
                    Instruction::MakeArray(n) => {
                        let v = take(frame, *n)?;
                        frame.stack.push(Value::Array(Rc::new(v)));
                        self.record_value_allocations(1);
                    }
                    Instruction::Append => {
                        let value = pop(frame)?;
                        let Value::Array(mut values) = pop(frame)? else {
                            return Err(Error::runtime("append expects an array"));
                        };
                        let cloned_backing = Rc::strong_count(&values) > 1;
                        Rc::make_mut(&mut values).push(value);
                        if cloned_backing {
                            self.record_value_allocations(1);
                        }
                        frame.stack.push(Value::Array(values));
                    }
                    Instruction::MergeArrays(n) => {
                        let segments = take(frame, *n)?;
                        let mut values = vec![];
                        for segment in segments {
                            let Value::Array(segment) = segment else {
                                return Err(Error::runtime("splat expects an array"));
                            };
                            values.extend(segment.iter().cloned());
                        }
                        frame.stack.push(Value::Array(Rc::new(values)));
                        self.record_value_allocations(1);
                    }
                    Instruction::MergeMaps(n) => {
                        let segments = take(frame, *n)?;
                        let mut values = BTreeMap::new();
                        for segment in segments {
                            let Value::Map(segment) = segment else {
                                return Err(Error::runtime("map spread expects a map"));
                            };
                            values.extend(
                                segment
                                    .iter()
                                    .map(|(key, value)| (key.clone(), value.clone())),
                            );
                        }
                        frame.stack.push(Value::Map(Rc::new(values)));
                        self.record_value_allocations(1);
                    }
                    Instruction::MakeRange(inclusive) => {
                        let end = pop(frame)?;
                        let start = pop(frame)?;
                        let (Value::Number(start), Value::Number(end)) = (start, end) else {
                            return Err(Error::runtime("range bounds must be numbers"));
                        };
                        frame.stack.push(numeric_range(start, end, *inclusive)?);
                        self.record_value_allocations(1);
                    }
                    Instruction::MakeMap(keys) => {
                        let v = take(frame, keys.len())?;
                        frame
                            .stack
                            .push(Value::Map(Rc::new(keys.iter().cloned().zip(v).collect())));
                        self.record_value_allocations(1);
                    }
                    Instruction::Stringify => {
                        let value = pop(frame)?;
                        frame.stack.push(Value::String(Rc::from(value.to_string())));
                        self.record_value_allocations(1);
                    }
                    Instruction::Concat(n) => {
                        let values = take(frame, *n)?;
                        let mut output = String::new();
                        for value in values {
                            let Value::String(value) = value else {
                                return Err(Error::runtime("concat received non-string"));
                            };
                            output.push_str(&value);
                        }
                        frame.stack.push(Value::String(Rc::from(output)));
                        self.record_value_allocations(1);
                    }
                    Instruction::Index => {
                        let key = pop(frame)?;
                        let target = pop(frame)?;
                        frame.stack.push(index(self, target, key)?)
                    }
                    Instruction::Slice(inclusive) => {
                        let end = pop(frame)?;
                        let start = pop(frame)?;
                        let target = pop(frame)?;
                        frame
                            .stack
                            .push(slice(self, target, start, end, *inclusive)?)
                    }
                    Instruction::Member(name) => {
                        match pop(frame)? {
                            Value::Map(map) => {
                                frame.stack.push(map.get(name.as_str()).cloned().ok_or_else(
                                    || Error::runtime(format!("map key '{name}' not found")),
                                )?)
                            }
                            _ => return Err(Error::runtime("member access expects a map")),
                        }
                    }
                    Instruction::MakeFunction(i) => match frame
                        .chunk
                        .constants
                        .get(*i)
                        .ok_or_else(|| Error::runtime("invalid function template"))?
                    {
                        Constant::Function {
                            params,
                            required,
                            rest,
                            chunk,
                        } => {
                            let debug_info = frame.debug_info.clone();
                            let fast_parameters = frame
                                .execution_plan
                                .as_ref()
                                .and_then(|plan| plan.slots(chunk))
                                .and_then(|slots| {
                                    slots.fast_parameter_slots(params, *required, rest.as_deref())
                                });
                            frame.stack.push(Value::Function(Rc::new(Function {
                                inner: FunctionKind::Bytecode {
                                    params: params.clone(),
                                    required: *required,
                                    rest: rest.clone(),
                                    chunk: chunk.clone(),
                                    debug_info,
                                    execution_plan: frame.execution_plan.clone(),
                                    fast_parameters,
                                    env: frame.env.clone(),
                                },
                            })));
                            self.record_value_allocations(1);
                        }
                        _ => return Err(Error::runtime("value used as function template")),
                    },
                    Instruction::Call(n) => {
                        return Self::direct_call(frame, *n);
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
                    if frames.len() > 1 {
                        self.call_depth = self.call_depth.saturating_sub(1);
                    }
                    frames.pop();
                    if let Some(parent) = frames.last_mut() {
                        parent.stack.push(value);
                    } else {
                        return Ok(value);
                    }
                }
                Ok(Step::Call { callee, args }) => {
                    let result = call(self, &mut frames, callee, &args);
                    Self::recycle_call_arguments(args);
                    match result {
                        Ok(()) => {}
                        Err(error) => {
                            let include_current_call = !error.labels().is_empty();
                            let span = frames.last().and_then(|frame| {
                                Self::source_span(frame, frame.pc.saturating_sub(1))
                            });
                            let error = error.with_span_if_missing(span);
                            let error = Self::with_call_stack(error, &frames, include_current_call);
                            if !handle_error(self, &mut frames, &error) {
                                return Err(error);
                            }
                        }
                    }
                }
                Err(error) => {
                    let span = frames
                        .last()
                        .and_then(|frame| Self::source_span(frame, frame.pc.saturating_sub(1)));
                    let error = error.with_span_if_missing(span);
                    let error = Self::with_call_stack(error, &frames, false);
                    if !handle_error(self, &mut frames, &error) {
                        return Err(error);
                    }
                }
            }
        }
    }
}
fn call(vm: &mut Vm, frames: &mut Vec<Frame>, callee: Value, args: &[Value]) -> Result<(), Error> {
    match callee {
        Value::Function(function) => match &function.inner {
            FunctionKind::Native {
                function,
                allocation_profile,
            } => {
                let value = function(args)?;
                if let Some(allocation_profile) = allocation_profile {
                    vm.record_value_allocations(allocation_profile(&value));
                }
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
                debug_info,
                execution_plan,
                fast_parameters,
                env: captured,
            } => {
                if vm.call_depth >= vm.max_call_depth {
                    return Err(Error::resource(
                        ResourceLimit::CallDepth,
                        format!(
                            "maximum QuickCoffee call depth of {} exceeded",
                            vm.max_call_depth
                        ),
                    ));
                }
                if args.len() < *required || (rest.is_none() && args.len() > params.len()) {
                    return Err(Error::runtime(format!(
                        "expected {}{} arguments, got {}",
                        required,
                        if rest.is_some() { " or more" } else { "" },
                        args.len()
                    )));
                }
                let binding_slots = execution_plan.as_ref().and_then(|plan| plan.slots(chunk));
                let fast_locals = fast_parameters.as_ref().zip(binding_slots.as_ref()).map(
                    |(parameter_slots, binding_slots)| {
                        let mut values = vec![None; binding_slots.local_names.len()];
                        for (slot, value) in parameter_slots.iter().zip(args) {
                            if let Some(slot) = slot {
                                values[*slot] = Some(value.clone());
                            }
                        }
                        values
                    },
                );
                let environment_elidable = fast_locals
                    .as_ref()
                    .is_some_and(|locals| locals.iter().all(Option::is_some));
                let shared_environment = fast_locals.is_none()
                    && binding_slots
                        .as_ref()
                        .is_some_and(|slots| slots.shared_environment);
                let local = if environment_elidable {
                    captured.clone()
                } else if shared_environment {
                    env_with_unset_slots(
                        captured.clone(),
                        &binding_slots
                            .as_ref()
                            .expect("shared environments require binding slots")
                            .local_names,
                    )
                } else {
                    env(Some(captured.clone()))
                };
                // ExecutionStats models one logical lexical frame per bytecode
                // call even when an isolated fast frame needs no physical Env.
                vm.record_environment_allocation();
                if fast_locals.is_none() {
                    for (index, pattern) in params.iter().enumerate() {
                        let value = args.get(index).cloned().unwrap_or(Value::Nil);
                        let mut bindings = vec![];
                        let snapshot = local.borrow().snapshot();
                        if let Err(error) = bind_pattern(
                            vm,
                            pattern,
                            Some(&value),
                            &mut bindings,
                            &local,
                            debug_info.as_ref(),
                            execution_plan.as_ref(),
                        ) {
                            local.borrow_mut().restore(snapshot);
                            return Err(error);
                        }
                        let mut environment = local.borrow_mut();
                        for (key, value) in bindings {
                            environment.set_local(&key, value);
                        }
                    }
                    if let Some(rest) = rest {
                        local
                            .borrow_mut()
                            .set_local(rest, Value::Array(Rc::new(args[params.len()..].to_vec())));
                        vm.record_value_allocations(1);
                    }
                }
                let bindings = match (binding_slots, fast_locals) {
                    (Some(slots), Some(locals)) => FrameBindings::Fast { slots, locals },
                    (Some(slots), None) if slots.shared_environment => FrameBindings::Shared(slots),
                    (Some(slots), None) => FrameBindings::Guarded(slots),
                    (None, None) => FrameBindings::Raw,
                    (None, Some(_)) => unreachable!("fast locals require a binding plan"),
                };
                frames.push(Frame {
                    chunk: chunk.clone(),
                    pc: 0,
                    stack: vec![],
                    iterators: vec![],
                    handlers: vec![],
                    debug_info: debug_info.clone(),
                    execution_plan: execution_plan.clone(),
                    env: local,
                    bindings,
                });
                vm.call_depth += 1;
                vm.call_depth_peak = vm.call_depth_peak.max(vm.call_depth);
            }
        },
        _ => return Err(Error::runtime("attempted to call a non-function")),
    }
    Ok(())
}
fn handle_error(vm: &mut Vm, frames: &mut Vec<Frame>, error: &Error) -> bool {
    if error.kind() == ErrorKind::Resource {
        return false;
    }
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
                .set_local(&handler.name, Value::String(Rc::from(error.to_string())));
            vm.record_value_allocations(1);
            frame.pc = handler.catch_pc;
            return true;
        }
        if frames.len() > 1 {
            vm.call_depth = vm.call_depth.saturating_sub(1);
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
fn numeric_array(xs: &[Value], name: &str) -> Result<Vec<f64>, Error> {
    if xs.len() != 1 {
        return Err(Error::runtime(format!("{name} expects one array")));
    }
    let Value::Array(values) = &xs[0] else {
        return Err(Error::runtime(format!("{name} expects an array")));
    };
    values
        .iter()
        .map(|value| {
            let value = number(value.clone())?;
            if value.is_finite() {
                Ok(value)
            } else {
                Err(Error::runtime(format!(
                    "{name} expects finite numeric elements"
                )))
            }
        })
        .collect()
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
    let start_i = i128::from(start);
    let end_i = i128::from(end);
    let (count, direction) = if start <= end {
        let limit = if inclusive { end_i + 1 } else { end_i };
        (limit - start_i, 1_i128)
    } else {
        let limit = if inclusive { end_i - 1 } else { end_i };
        (start_i - limit, -1_i128)
    };
    if count > MAX_RANGE_ITEMS {
        return Err(Error::runtime("range is too large"));
    }
    Ok(Value::Array(Rc::new(
        (0..count as usize)
            .map(|offset| Value::Number((start_i + offset as i128 * direction) as f64))
            .collect(),
    )))
}
fn array_iteration_step(value: Value) -> Result<i64, Error> {
    let Value::Number(step) = value else {
        return Err(Error::runtime(
            "for by step must be a non-zero finite integer",
        ));
    };
    if !step.is_finite()
        || step.fract() != 0.
        || step == 0.
        || step < i64::MIN as f64
        || step > i64::MAX as f64
    {
        return Err(Error::runtime(
            "for by step must be a non-zero finite integer",
        ));
    }
    Ok(step as i64)
}
fn advance_position(position: &mut usize, step: i64) {
    if step >= 0 {
        *position = position.saturating_add(step as usize);
    } else {
        let amount = step.unsigned_abs() as usize;
        *position = position.checked_sub(amount).unwrap_or(usize::MAX);
    }
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
    vm: &mut Vm,
    pattern: &Pattern,
    value: Option<&Value>,
    bindings: &mut Vec<(String, Value)>,
    env: &Env,
    debug_info: Option<&Rc<ProgramDebugInfo>>,
    execution_plan: Option<&Rc<ProgramExecutionPlan>>,
) -> Result<(), Error> {
    match pattern {
        Pattern::Ignore => value.map_or_else(
            || Err(Error::runtime("missing value for pattern")),
            |_| Ok(()),
        ),
        Pattern::Bind(name) => {
            let value = value.ok_or_else(|| Error::runtime("missing value for pattern"))?;
            bindings.push((name.clone(), value.clone()));
            if name != "_" {
                env.borrow_mut().set_local(name, value.clone());
            }
            Ok(())
        }
        Pattern::Rest(name) => {
            let value = value.ok_or_else(|| Error::runtime("missing value for rest pattern"))?;
            bindings.push((name.clone(), value.clone()));
            if name != "_" {
                env.borrow_mut().set_local(name, value.clone());
            }
            Ok(())
        }
        Pattern::Default { pattern, default } => {
            let value = match value {
                Some(value) if !matches!(value, Value::Nil) => value.clone(),
                _ => vm.eval_default(
                    default.clone(),
                    env.clone(),
                    debug_info.cloned(),
                    execution_plan.cloned(),
                )?,
            };
            bind_pattern(
                vm,
                pattern,
                Some(&value),
                bindings,
                env,
                debug_info,
                execution_plan,
            )
        }
        Pattern::Array(patterns) => {
            let Some(Value::Array(values)) = value else {
                return Err(Error::runtime("array destructuring expects an array"));
            };
            let rest_index = patterns
                .iter()
                .position(|pattern| matches!(pattern, Pattern::Rest(_)));
            let required_len = patterns
                .iter()
                .enumerate()
                .filter(|(_, pattern)| {
                    !matches!(pattern, Pattern::Default { .. } | Pattern::Rest(_))
                })
                .map(|(index, _)| index + 1)
                .max()
                .unwrap_or(0);
            let fixed_len = rest_index.unwrap_or(required_len);
            if values.len() < fixed_len || (rest_index.is_none() && values.len() > patterns.len()) {
                return Err(Error::runtime(format!(
                    "array destructuring expected {} values, got {}",
                    if rest_index.is_some() {
                        format!("at least {fixed_len}")
                    } else {
                        patterns.len().to_string()
                    },
                    values.len()
                )));
            }
            for (index, pattern) in patterns.iter().enumerate() {
                if let Pattern::Rest(name) = pattern {
                    let rest = Value::Array(Rc::new(values[index..].to_vec()));
                    vm.record_value_allocations(1);
                    bindings.push((name.clone(), rest.clone()));
                    if name != "_" {
                        env.borrow_mut().set_local(name, rest);
                    }
                    break;
                }
                bind_pattern(
                    vm,
                    pattern,
                    values.get(index),
                    bindings,
                    env,
                    debug_info,
                    execution_plan,
                )?;
            }
            Ok(())
        }
        Pattern::Map(fields) => {
            let Some(Value::Map(map)) = value else {
                return Err(Error::runtime("map destructuring expects a map"));
            };
            for (key, pattern) in fields {
                bind_pattern(
                    vm,
                    pattern,
                    map.get(key),
                    bindings,
                    env,
                    debug_info,
                    execution_plan,
                )
                .map_err(|error| {
                    if map.contains_key(key) {
                        error
                    } else {
                        Error::runtime(format!("map key '{key}' not found"))
                    }
                })?;
            }
            Ok(())
        }
        Pattern::MapRest { fields, rest } => {
            let Some(Value::Map(map)) = value else {
                return Err(Error::runtime("map destructuring expects a map"));
            };
            for (key, pattern) in fields.iter() {
                bind_pattern(
                    vm,
                    pattern,
                    map.get(key),
                    bindings,
                    env,
                    debug_info,
                    execution_plan,
                )
                .map_err(|error| {
                    if map.contains_key(key) {
                        error
                    } else {
                        Error::runtime(format!("map key '{key}' not found"))
                    }
                })?;
            }
            let explicit_fields: BTreeSet<&str> =
                fields.iter().map(|(field, _)| field.as_str()).collect();
            let mut remaining = BTreeMap::new();
            for (key, item) in map.iter() {
                if !explicit_fields.contains(key.as_str()) {
                    remaining.insert(key.clone(), item.clone());
                }
            }
            let rest_value = Value::Map(Rc::new(remaining));
            vm.record_value_allocations(1);
            bindings.push((rest.clone(), rest_value.clone()));
            if rest != "_" {
                env.borrow_mut().set_local(rest, rest_value);
            }
            Ok(())
        }
    }
}
fn index(vm: &mut Vm, target: Value, key: Value) -> Result<Value, Error> {
    match (target, key) {
        (Value::Array(xs), Value::Number(i)) if i.is_finite() && i.fract() == 0. => xs
            .get(sequence_index(i, xs.len(), "array")?)
            .cloned()
            .ok_or_else(|| Error::runtime("array index out of range")),
        (Value::String(text), Value::Number(i)) if i.is_finite() && i.fract() == 0. => {
            let character = text
                .chars()
                .nth(sequence_index(i, text.chars().count(), "string")?)
                .ok_or_else(|| Error::runtime("string index out of range"))?;
            vm.record_value_allocations(1);
            Ok(Value::String(Rc::from(character.to_string())))
        }
        (Value::Map(m), Value::String(k)) => m
            .get(k.as_ref())
            .cloned()
            .ok_or_else(|| Error::runtime("map key not found")),
        _ => Err(Error::runtime("invalid index operation")),
    }
}
fn sequence_index(index: f64, len: usize, kind: &str) -> Result<usize, Error> {
    let index = index as i128;
    let len = len as i128;
    let resolved = if index < 0 { len + index } else { index };
    if resolved < 0 || resolved >= len {
        return Err(Error::runtime(format!("{kind} index out of range")));
    }
    Ok(resolved as usize)
}
fn slice(
    vm: &mut Vm,
    target: Value,
    start: Value,
    end: Value,
    inclusive: bool,
) -> Result<Value, Error> {
    match target {
        Value::Array(values) => {
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
            let value = Value::Array(Rc::new(values[start..end].to_vec()));
            vm.record_value_allocations(1);
            Ok(value)
        }
        Value::String(text) => {
            let scalar_len = text.chars().count();
            let start = slice_bound(start, scalar_len, "slice start")?;
            let mut end = slice_bound(end, scalar_len, "slice end")?;
            if inclusive {
                end = end
                    .checked_add(1)
                    .ok_or_else(|| Error::runtime("inclusive slice end is too large"))?;
            }
            if start > end || end > scalar_len {
                return Err(Error::runtime("slice bounds out of range"));
            }
            let start_byte = text
                .char_indices()
                .nth(start)
                .map_or(text.len(), |(offset, _)| offset);
            let end_byte = text
                .char_indices()
                .nth(end)
                .map_or(text.len(), |(offset, _)| offset);
            let value = Value::String(Rc::from(&text[start_byte..end_byte]));
            vm.record_value_allocations(1);
            Ok(value)
        }
        _ => Err(Error::runtime("slice expects an array or string")),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_result_equal(left: Result<Value, Error>, right: Result<Value, Error>) {
        match (left, right) {
            (Ok(left), Ok(right)) => assert!(
                equal(&left, &right),
                "optimized value {left:?} differs from reference {right:?}"
            ),
            (Err(left), Err(right)) => {
                assert_eq!(left.kind(), right.kind());
                assert_eq!(left.message(), right.message());
                assert_eq!(left.labels(), right.labels());
                assert_eq!(left.resource_limit(), right.resource_limit());
            }
            (left, right) => panic!("optimized/reference result mismatch: {left:?} vs {right:?}"),
        }
    }

    fn assert_cached_and_reference(source: &str, fuel: u64) {
        let optimized = Engine::new()
            .compile_program_named("differential.qc", source)
            .expect("differential source must compile");
        let reference = optimized.without_binding_slots();
        assert_eq!(optimized.disassemble(), reference.disassemble());
        assert_eq!(optimized.fingerprint(), reference.fingerprint());
        let mut optimized_context = Context::new().with_fuel(fuel);
        let mut reference_context = Context::new().with_fuel(fuel);
        let optimized_result = optimized_context.run_program(&optimized);
        let reference_result = reference_context.run_program(&reference);
        assert_result_equal(optimized_result, reference_result);
        assert_eq!(
            optimized_context.last_execution(),
            reference_context.last_execution()
        );
    }

    #[test]
    fn reusable_call_arguments_preserve_mixed_calls_and_failure_recovery() {
        fn reusable_capacity() -> usize {
            REUSABLE_CALL_ARGUMENTS.with(|reusable| reusable.borrow().capacity())
        }
        fn reusable_is_empty() -> bool {
            REUSABLE_CALL_ARGUMENTS.with(|reusable| reusable.borrow().is_empty())
        }

        REUSABLE_CALL_ARGUMENTS.with(|reusable| *reusable.borrow_mut() = Vec::new());
        let engine = Engine::new();
        let mixed_calls = engine
            .compile_program(
                "zero = -> 1\none = (value) -> value + 1\nmany = (head, tail...) -> head + sum(tail)\n[zero(), one(1), many(1, 2, 3), many([1, 2, 3]...), len([1, 2, 3])]",
            )
            .unwrap();
        let bad_call = engine
            .compile_program("one = (value) -> value + 1\none()")
            .unwrap();
        let large_native_call = engine
            .compile_program(
                "count_args(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17)",
            )
            .unwrap();
        let mut context = Context::new();
        context.add_native("count_args", |args| Ok(Value::from(args.len() as i64)));

        assert_eq!(
            context.run_program(&mixed_calls).unwrap().to_string(),
            "[1, 2, 6, 6, 3]"
        );
        let successful_stats = context.last_execution();
        assert!(reusable_is_empty());
        assert!(reusable_capacity() <= MAX_REUSABLE_CALL_ARGUMENTS);
        REUSABLE_CALL_ARGUMENTS.with(|reusable| {
            *reusable.borrow_mut() = Vec::with_capacity(4);
        });
        let initial_capacity = reusable_capacity();
        assert!(initial_capacity > 0);

        assert_eq!(
            context.run_program(&large_native_call).unwrap().to_string(),
            "17"
        );
        let post_large_capacity = reusable_capacity();
        assert!(post_large_capacity >= initial_capacity);
        assert!(post_large_capacity <= MAX_REUSABLE_CALL_ARGUMENTS);

        let error = context.run_program(&bad_call).unwrap_err();
        assert_eq!(error.message(), "expected 1 arguments, got 0");
        assert!(reusable_is_empty());
        assert!(reusable_capacity() <= MAX_REUSABLE_CALL_ARGUMENTS);

        assert_eq!(
            context.run_program(&mixed_calls).unwrap().to_string(),
            "[1, 2, 6, 6, 3]"
        );
        assert_eq!(context.last_execution(), successful_stats);
        assert_eq!(reusable_capacity(), post_large_capacity);
        REUSABLE_CALL_ARGUMENTS.with(|reusable| *reusable.borrow_mut() = Vec::new());
    }

    #[test]
    fn binding_slots_match_reference_for_dynamic_shadowing_and_layouts() {
        assert_cached_and_reference(
            "x = 1\nf = ->\n  i = 0\n  out = []\n  while i < 2\n    out = [out..., x]\n    x = 2 if i == 0\n    i++\n  out\nf()",
            10_000,
        );
        assert_cached_and_reference(
            "f = (flag) ->\n  extra = 1 if flag\n  x = 2\n  extra ?= 0\n  x + extra\n[f(false), f(true), f(false)]",
            10_000,
        );
    }

    #[test]
    fn binding_slots_match_reference_for_patterns_closures_and_handlers() {
        assert_cached_and_reference(
            "base = 10\nmake = (offset = 1) ->\n  ([left, right], {factor}) ->\n    sum = 0\n    for value, index in [left, right]\n      sum += value * factor + index + base + offset\n    try\n      throw 'boom' if sum < 0\n      sum\n    catch error\n      0\nfn = make(2)\nfn([1, 2], {factor: 3})",
            20_000,
        );
        assert_cached_and_reference(
            "x = 1\ni = 0\nwhile i < 2\n  try\n    {x, missing} = {x: i + 2}\n  catch error\n    nil\n  i++\nx",
            10_000,
        );
        assert_cached_and_reference(
            "outer = 40\nf = ->\n  try\n    [outer, missing] = [1]\n  catch error\n    outer\nf()",
            10_000,
        );
        assert_cached_and_reference(
            "side = 40\nf = ->\n  try\n    [local = (side = 1), {needed}] = [nil, {}]\n  catch error\n    side\nf()",
            10_000,
        );
        assert_cached_and_reference(
            "f = (flag, first = (padding = 1 if flag), value = (x ?= 2)) -> value\n[f(false), f(true), f(false)]",
            10_000,
        );
    }

    #[test]
    fn shared_environment_slots_preserve_closure_fallback_sharing_and_rollback() {
        let cases = [
            (
                "make = ->\n  value = 1\n  read = -> value\n  value = 2\n  read\nmake()()",
                "2",
                1_000,
            ),
            (
                "value = 40\nmake = ->\n  read = -> value\n  before = read()\n  value = 2\n  [before, read()]\nmake()",
                "[40, 2]",
                1_000,
            ),
            (
                "make = ->\n  value = 1\n  left = -> value\n  right = -> value\n  value = 3\n  [left, right]\nreaders = make()\n[readers[0](), readers[1]()]",
                "[3, 3]",
                1_000,
            ),
            (
                "local = 40\nmake = ->\n  read = -> local\n  try\n    [local, missing] = [1]\n  catch error\n    read()\nmake()",
                "40",
                1_000,
            ),
        ];
        for (source, expected, fuel) in cases {
            assert_cached_and_reference(source, fuel);
            assert_eq!(Context::new().eval(source).unwrap().to_string(), expected);
        }

        assert_cached_and_reference(
            "make = ->\n  value = 1\n  fail = -> missing + value\n  fail\nmake()()",
            1_000,
        );
        assert_cached_and_reference(
            "make = (limit) ->\n  value = 0\n  read = -> value\n  while value < limit then value++\n  read\nmake(1000)()",
            50,
        );

        let program = Engine::new()
            .compile_program(
                "make = ->\n  value = 0\n  read = -> value\n  value++\n  read\nmake()()",
            )
            .unwrap();
        let Constant::Function { chunk, .. } = program
            .0
            .chunk
            .constants
            .iter()
            .find(|constant| {
                matches!(
                    constant,
                    Constant::Function { chunk, .. }
                        if chunk.code.iter().any(|instruction| matches!(instruction, Instruction::MakeFunction(_)))
                )
            })
            .expect("capturing function template")
        else {
            unreachable!("filtered to function constants")
        };
        let slots = program
            .0
            .execution_plan
            .as_ref()
            .and_then(|plan| plan.slots(chunk))
            .unwrap();
        assert!(!slots.isolated_frame);
        assert!(slots.shared_environment);
        assert!(
            slots
                .local_names
                .iter()
                .any(|name| name.as_ref() == "value")
        );
    }

    #[test]
    fn binding_slots_preserve_errors_labels_fuel_and_raw_chunks() {
        assert_cached_and_reference("known = 1\nknown + missing", 100);
        assert_cached_and_reference("i = 0\nloop\n  i++", 50);
        assert_cached_and_reference("[left, right] = [1]", 100);
        assert_cached_and_reference(
            "increment = (value) -> value + 1\nrun = (limit) ->\n  value = 0\n  while value < limit then value = increment(value)\n  value\nrun(100)",
            50,
        );
        assert_cached_and_reference(
            "fail = -> missing\nrun = (value) -> fail() + value\nrun(1)",
            100,
        );

        let chunk = Engine::new().compile("value = 1\nvalue + 1").unwrap();
        let raw = Program::from(chunk);
        assert!(raw.0.execution_plan.is_none());
        assert_eq!(Context::new().run_program(&raw).unwrap().to_string(), "2");
    }

    #[test]
    fn retained_functions_keep_their_program_binding_plan_across_eval() {
        let engine = Engine::new();
        let define = engine
            .compile_program("base = 40\nadd = (value) -> value + base\nadd")
            .unwrap();
        let call = engine.compile_program("base = 41\nadd(1)").unwrap();
        let define_reference = define.without_binding_slots();
        let call_reference = call.without_binding_slots();

        let mut optimized = Context::new();
        let mut reference = Context::new();
        optimized.run_program(&define).unwrap();
        reference.run_program(&define_reference).unwrap();
        assert_result_equal(
            optimized.run_program(&call),
            reference.run_program(&call_reference),
        );
        assert_eq!(optimized.last_execution(), reference.last_execution());
        assert_eq!(optimized.get_global("base").unwrap().to_string(), "41");
        assert_eq!(reference.get_global("base").unwrap().to_string(), "41");

        let define = engine
            .compile_program(
                "make = ->\n  value = 1\n  read = -> value\n  value = 2\n  read\nreader = make()\nreader",
            )
            .unwrap();
        let call = engine.compile_program("reader()").unwrap();
        let define_reference = define.without_binding_slots();
        let call_reference = call.without_binding_slots();
        let mut optimized = Context::new();
        let mut reference = Context::new();
        optimized.run_program(&define).unwrap();
        reference.run_program(&define_reference).unwrap();
        assert_result_equal(
            optimized.run_program(&call),
            reference.run_program(&call_reference),
        );
        assert_eq!(optimized.last_execution(), reference.last_execution());
        assert_eq!(
            optimized.get_global("reader").unwrap().to_string(),
            "<function>"
        );
        assert_eq!(
            reference.get_global("reader").unwrap().to_string(),
            "<function>"
        );
    }

    #[test]
    fn shared_program_binding_slots_are_guarded_across_context_layouts() {
        let optimized = Engine::new()
            .compile_program(
                "f = (flag) ->\n  extra = 1 if flag\n  local = 2\n  extra ?= 0\n  local + extra\n[f(flag), x]",
            )
            .unwrap();
        let reference = optimized.without_binding_slots();

        let mut optimized_a = Context::new();
        optimized_a.set_global("x", Value::Number(40.));
        optimized_a.set_global("flag", Value::Bool(false));
        let mut optimized_b = Context::new();
        optimized_b.set_global("padding", Value::Nil);
        optimized_b.set_global("x", Value::Number(41.));
        optimized_b.set_global("flag", Value::Bool(true));

        let mut reference_a = Context::new();
        reference_a.set_global("x", Value::Number(40.));
        reference_a.set_global("flag", Value::Bool(false));
        let mut reference_b = Context::new();
        reference_b.set_global("padding", Value::Nil);
        reference_b.set_global("x", Value::Number(41.));
        reference_b.set_global("flag", Value::Bool(true));

        fn run_pair(
            optimized: &Program,
            reference: &Program,
            optimized_context: &mut Context,
            reference_context: &mut Context,
        ) {
            assert_result_equal(
                optimized_context.run_program(optimized),
                reference_context.run_program(reference),
            );
            assert_eq!(
                optimized_context.last_execution(),
                reference_context.last_execution()
            );
        }
        run_pair(&optimized, &reference, &mut optimized_a, &mut reference_a);
        run_pair(&optimized, &reference, &mut optimized_b, &mut reference_b);
        run_pair(&optimized, &reference, &mut optimized_a, &mut reference_a);

        let captured = Engine::new()
            .compile_program("read = (value) -> value + host\nread(1)")
            .unwrap();
        let captured_reference = captured.without_binding_slots();
        optimized_a.set_global("host", Value::Number(40.));
        optimized_b.set_global("host", Value::Number(41.));
        reference_a.set_global("host", Value::Number(40.));
        reference_b.set_global("host", Value::Number(41.));
        run_pair(
            &captured,
            &captured_reference,
            &mut optimized_a,
            &mut reference_a,
        );

        let shared = Engine::new()
            .compile_program(
                "make = (flag) ->\n  read = -> value\n  value = host if flag\n  [read(), host]\nmake(flag)",
            )
            .unwrap();
        let shared_reference = shared.without_binding_slots();
        optimized_a.set_global("value", Value::Number(40.));
        optimized_a.set_global("host", Value::Number(2.));
        optimized_a.set_global("flag", Value::Bool(false));
        optimized_b.set_global("value", Value::Number(41.));
        optimized_b.set_global("host", Value::Number(3.));
        optimized_b.set_global("flag", Value::Bool(true));
        reference_a.set_global("value", Value::Number(40.));
        reference_a.set_global("host", Value::Number(2.));
        reference_a.set_global("flag", Value::Bool(false));
        reference_b.set_global("value", Value::Number(41.));
        reference_b.set_global("host", Value::Number(3.));
        reference_b.set_global("flag", Value::Bool(true));
        run_pair(
            &shared,
            &shared_reference,
            &mut optimized_a,
            &mut reference_a,
        );
        run_pair(
            &shared,
            &shared_reference,
            &mut optimized_b,
            &mut reference_b,
        );
        run_pair(
            &shared,
            &shared_reference,
            &mut optimized_a,
            &mut reference_a,
        );
        run_pair(
            &captured,
            &captured_reference,
            &mut optimized_b,
            &mut reference_b,
        );
        run_pair(
            &captured,
            &captured_reference,
            &mut optimized_a,
            &mut reference_a,
        );
    }

    #[test]
    fn compiler_resolved_frame_slots_preserve_leaf_calls_persistence_and_failure_state() {
        assert_cached_and_reference(
            "increment = (value) -> value + 1\nsum = 0\ni = 0\nwhile i < 100\n  sum = increment(sum)\n  i++\nsum",
            10_000,
        );
        assert_cached_and_reference(
            "increment = (value) -> value + 1\nrun = (limit) ->\n  sum = 0\n  i = 0\n  while i < limit\n    sum = increment(sum)\n    i++\n  sum\nrun(100)",
            10_000,
        );
        assert_cached_and_reference(
            "factorial = (n) -> if n == 0 then 1 else n * factorial(n - 1)\nfactorial(8)",
            10_000,
        );
        assert_cached_and_reference(
            "outer = 7\nread = (condition) ->\n  if condition then outer = 9\n  outer\n[read(false), read(true), outer]",
            1_000,
        );
        let leaf = Engine::new()
            .compile_program("increment = (value) -> value + 1\nincrement(41)")
            .unwrap();
        let Constant::Function {
            params,
            required,
            rest,
            chunk,
        } = &leaf.0.chunk.constants[0]
        else {
            panic!("first constant is the leaf function template");
        };
        let leaf_slots = leaf
            .0
            .execution_plan
            .as_ref()
            .and_then(|plan| plan.slots(chunk))
            .unwrap();
        assert!(
            leaf_slots
                .fast_parameter_slots(params, *required, rest.as_deref())
                .is_some()
        );

        let caller = Engine::new()
            .compile_program(
                "increment = (value) -> value + 1\nrun = (limit) -> increment(limit)\nrun(41)",
            )
            .unwrap();
        let Constant::Function {
            params,
            required,
            rest,
            chunk,
        } = caller
            .0
            .chunk
            .constants
            .iter()
            .find(|constant| {
                matches!(
                    constant,
                    Constant::Function { chunk, .. }
                        if chunk.code.iter().any(|instruction| matches!(instruction, Instruction::Call(_)))
                )
            })
            .expect("caller function template")
        else {
            unreachable!("filtered to function constants")
        };
        let caller_slots = caller
            .0
            .execution_plan
            .as_ref()
            .and_then(|plan| plan.slots(chunk))
            .unwrap();
        assert!(caller_slots.isolated_frame);
        assert!(
            caller_slots
                .fast_parameter_slots(params, *required, rest.as_deref())
                .is_some()
        );

        let repeated = Engine::new()
            .compile_program("i ?= 0\nlimit = i + 3\nwhile i < limit then i++\ni")
            .unwrap();
        let repeated_reference = repeated.without_binding_slots();
        let slots = repeated
            .0
            .execution_plan
            .as_ref()
            .and_then(|plan| plan.slots(&repeated.0.chunk))
            .unwrap();
        assert!(slots.isolated_frame);
        let mut optimized_context = Context::new();
        let mut reference_context = Context::new();
        for expected in ["3", "6"] {
            assert_eq!(
                optimized_context
                    .run_program(&repeated)
                    .unwrap()
                    .to_string(),
                expected
            );
            assert_eq!(
                reference_context
                    .run_program(&repeated_reference)
                    .unwrap()
                    .to_string(),
                expected
            );
            assert_eq!(
                optimized_context.last_execution(),
                reference_context.last_execution()
            );
        }

        let exhausted = Engine::new()
            .compile_program(
                "spin = (limit) ->\n  i = 0\n  while i < limit\n    i++\n  i\nspin(1000)",
            )
            .unwrap();
        let exhausted_reference = exhausted.without_binding_slots();
        let mut optimized_context = Context::new().with_fuel(50);
        let mut reference_context = Context::new().with_fuel(50);
        assert_result_equal(
            optimized_context.run_program(&exhausted),
            reference_context.run_program(&exhausted_reference),
        );
        assert_eq!(
            optimized_context.last_execution(),
            reference_context.last_execution()
        );
        assert!(optimized_context.get_global("i").is_none());
        assert!(reference_context.get_global("i").is_none());
    }
}
