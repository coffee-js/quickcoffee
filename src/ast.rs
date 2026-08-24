use crate::lexer::TokenSpan;

#[derive(Clone, Debug)]
pub(crate) enum Expr {
    Located(TokenSpan, Box<Expr>),
    Number(f64),
    String(String),
    Interpolate(Vec<Expr>),
    Bool(bool),
    Nil,
    Name(String),
    Assign(String, Box<Expr>),
    AssignIfNil(String, Box<Expr>),
    Update(String, Update, bool),
    Destructure(Pattern, Box<Expr>),
    Array(Vec<Item>),
    Range(Box<Expr>, Box<Expr>, bool),
    Map(Vec<MapItem>),
    Unary(Unary, Box<Expr>),
    Exists(Box<Expr>),
    Binary(Box<Expr>, Binary, Box<Expr>),
    CompareChain(Vec<Expr>, Vec<Binary>),
    Index(Box<Expr>, Box<Expr>),
    Slice(Box<Expr>, Box<Expr>, Box<Expr>, bool),
    Member(Box<Expr>, String),
    Call(Box<Expr>, Vec<Item>),
    SoakIndex(Box<Expr>, Box<Expr>),
    SoakSlice(Box<Expr>, Box<Expr>, Box<Expr>, bool),
    SoakMember(Box<Expr>, String),
    SoakCall(Box<Expr>, Vec<Item>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    While(Box<Expr>, Box<Expr>),
    For(
        Vec<Pattern>,
        bool,
        Box<Expr>,
        Option<Box<Expr>>,
        Option<Box<Expr>>,
        Box<Expr>,
    ),
    Break(TokenSpan),
    Continue(TokenSpan),
    Return(Option<Box<Expr>>, TokenSpan),
    Function(Vec<Param>, Option<String>, Box<Expr>),
    Class(String, Vec<Param>, Box<Expr>),
    Block(Vec<Stmt>),
    Switch(Box<Expr>, Vec<(Vec<Expr>, Expr)>, Option<Box<Expr>>),
    Try(Box<Expr>, String, Box<Expr>, Option<Box<Expr>>),
    Throw(Box<Expr>),
    Do(Box<Expr>),
}
impl Expr {
    pub(crate) fn unspanned(&self) -> &Self {
        match self {
            Self::Located(_, expression) => expression.unspanned(),
            expression => expression,
        }
    }
    pub(crate) fn span(&self) -> Option<TokenSpan> {
        match self {
            Self::Located(span, _) => Some(*span),
            _ => None,
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) enum MapItem {
    Entry(String, Expr),
    Splat(Expr),
}
#[derive(Clone, Debug)]
pub(crate) enum Pattern {
    Ignore,
    Bind(String),
    Rest(String),
    Default(Box<Pattern>, Box<Expr>),
    Array(Vec<Pattern>),
    Map(Vec<(String, Pattern)>),
    MapRest(Vec<(String, Pattern)>, String),
}
#[derive(Clone, Debug)]
pub(crate) enum Item {
    Expr(Expr),
    Splat(Expr),
}
#[derive(Clone, Debug)]
pub(crate) struct Param {
    pub pattern: Pattern,
    pub default: Option<Expr>,
}
#[derive(Clone, Debug)]
pub(crate) enum Stmt {
    Assign(String, Expr, TokenSpan),
    Destructure(Pattern, Expr, TokenSpan),
    Import(Vec<(String, String)>, String, TokenSpan),
    ExportAssign(String, Expr, TokenSpan),
    ExportNames(Vec<(String, String)>, TokenSpan),
    Expr(Expr),
}
/// Parsed module directives plus the executable module body.
#[derive(Clone, Debug)]
pub(crate) struct ModuleSyntax {
    pub imports: Vec<(Vec<(String, String)>, String)>,
    pub exports: Vec<(String, String, TokenSpan)>,
    pub body: Vec<Stmt>,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Unary {
    Neg,
    Not,
    BitNot,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Update {
    Increment,
    Decrement,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum Binary {
    Coalesce,
    In,
    Of,
    NotIn,
    NotOf,
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
    And,
    Or,
}
