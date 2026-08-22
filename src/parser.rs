use crate::{
    bytecode::Pattern,
    lexer::{Token, lex_spanned},
    vm::Error,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(crate) enum Expr {
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
    Map(BTreeMap<String, Expr>),
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
    Break,
    Continue,
    Return(Option<Box<Expr>>),
    Function(Vec<Param>, Option<String>, Box<Expr>),
    Class(String, Vec<Param>, Box<Expr>),
    Block(Vec<Stmt>),
    Switch(Box<Expr>, Vec<(Vec<Expr>, Expr)>, Option<Box<Expr>>),
    Try(Box<Expr>, String, Box<Expr>, Option<Box<Expr>>),
    Throw(Box<Expr>),
    Do(Box<Expr>),
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
    Assign(String, Expr),
    Destructure(Pattern, Expr),
    Expr(Expr),
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
type SwitchCases = (Vec<(Vec<Expr>, Expr)>, Option<Expr>);
type LambdaParams = (Vec<Param>, Option<String>);
type ForHeader = (
    Vec<Pattern>,
    bool,
    Expr,
    Option<Box<Expr>>,
    Option<Box<Expr>>,
);

pub(crate) fn parse(source: &str) -> Result<Vec<Stmt>, Error> {
    let (tokens, lines) = lex_spanned(source)?;
    Parser {
        tokens,
        lines,
        at: 0,
    }
    .program()
}
struct Parser {
    tokens: Vec<Token>,
    lines: Vec<usize>,
    at: usize,
}
impl Parser {
    fn parse_error(&self, message: impl Into<String>) -> Error {
        let error = Error::parse(message);
        self.lines
            .get(self.at)
            .or_else(|| self.lines.last())
            .copied()
            .map_or(error.clone(), |line| error.at_line(line))
    }
    fn peek(&self) -> &Token {
        &self.tokens[self.at]
    }
    fn next(&mut self) -> Token {
        let x = self.tokens[self.at].clone();
        self.at += 1;
        x
    }
    fn eat(&mut self, expected: &Token) -> bool {
        if self.peek() == expected {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, expected: &Token) -> Result<(), Error> {
        if self.eat(expected) {
            Ok(())
        } else {
            Err(self.parse_error(format!("expected {expected:?}, found {:?}", self.peek())))
        }
    }
    fn program(mut self) -> Result<Vec<Stmt>, Error> {
        let mut out = vec![];
        while !matches!(self.peek(), Token::Eof) {
            while self.eat(&Token::Semi) {}
            if matches!(self.peek(), Token::Eof) {
                break;
            };
            let stmt = self.statement()?;
            out.push(stmt);
            if !matches!(self.peek(), Token::Semi | Token::Eof) {
                return Err(self.parse_error("expected statement separator"));
            }
        }
        Ok(out)
    }
    fn statement(&mut self) -> Result<Stmt, Error> {
        if let Some(pattern) = self.assignment_pattern() {
            let value = self.expr(0)?;
            if matches!(self.peek(), Token::While | Token::Until) {
                let body = match pattern {
                    Pattern::Bind(name) => Expr::Assign(name, Box::new(value)),
                    pattern => Expr::Destructure(pattern, Box::new(value)),
                };
                return Ok(Stmt::Expr(self.postfix_loop(body)?));
            }
            if let Pattern::Bind(name) = pattern {
                Ok(Stmt::Assign(name, value))
            } else {
                Ok(Stmt::Destructure(pattern, value))
            }
        } else {
            let expression = self.expr(0)?;
            if matches!(self.peek(), Token::While | Token::Until) {
                Ok(Stmt::Expr(self.postfix_loop(expression)?))
            } else {
                Ok(Stmt::Expr(expression))
            }
        }
    }
    fn postfix_loop(&mut self, body: Expr) -> Result<Expr, Error> {
        let inverted = if self.eat(&Token::While) {
            false
        } else if self.eat(&Token::Until) {
            true
        } else {
            return Err(self.parse_error("expected while or until"));
        };
        let condition = self.expr(0)?;
        let condition = if inverted {
            Expr::Unary(Unary::Not, Box::new(condition))
        } else {
            condition
        };
        Ok(Expr::While(Box::new(condition), Box::new(body)))
    }
    fn assignment_pattern(&mut self) -> Option<Pattern> {
        let saved = self.at;
        let Some(first) = self.pattern() else {
            self.at = saved;
            return None;
        };
        let pattern = if self.eat(&Token::Comma) {
            let mut patterns = vec![first];
            loop {
                let Some(pattern) = self.pattern() else {
                    self.at = saved;
                    return None;
                };
                patterns.push(pattern);
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            Pattern::Array(patterns)
        } else {
            first
        };
        if !self.eat(&Token::Assign) {
            self.at = saved;
            return None;
        }
        Some(pattern)
    }
    fn pattern(&mut self) -> Option<Pattern> {
        let saved = self.at;
        let pattern = match self.next() {
            Token::Ident(name) if name == "_" => Pattern::Ignore,
            Token::Ident(name) => Pattern::Bind(name),
            Token::LBracket => {
                let mut items = vec![];
                if !self.eat(&Token::RBracket) {
                    loop {
                        let Some(pattern) = self.pattern() else {
                            self.at = saved;
                            return None;
                        };
                        items.push(pattern);
                        if self.eat(&Token::RBracket) {
                            break;
                        }
                        if !self.eat(&Token::Comma) {
                            self.at = saved;
                            return None;
                        }
                    }
                }
                Pattern::Array(items)
            }
            Token::LBrace => {
                let mut fields = vec![];
                if !self.eat(&Token::RBrace) {
                    loop {
                        let key = match self.next() {
                            Token::Ident(key) => key,
                            _ => {
                                self.at = saved;
                                return None;
                            }
                        };
                        let value = if self.eat(&Token::Colon) {
                            let Some(pattern) = self.pattern() else {
                                self.at = saved;
                                return None;
                            };
                            pattern
                        } else if key == "_" {
                            Pattern::Ignore
                        } else {
                            Pattern::Bind(key.clone())
                        };
                        fields.push((key, value));
                        if self.eat(&Token::RBrace) {
                            break;
                        }
                        if !self.eat(&Token::Comma) {
                            self.at = saved;
                            return None;
                        }
                    }
                }
                Pattern::Map(fields)
            }
            _ => {
                self.at = saved;
                return None;
            }
        };
        Some(pattern)
    }
    fn body(&mut self) -> Result<Expr, Error> {
        self.eat(&Token::Then);
        if self.eat(&Token::Semi) {
            while self.eat(&Token::Semi) {}
            return self.layout_block();
        }
        self.expr(0)
    }
    fn body_after_arrow(&mut self) -> Result<Expr, Error> {
        self.body()
    }
    fn else_body(&mut self) -> Result<Option<Expr>, Error> {
        let saved = self.at;
        while self.eat(&Token::Semi) {}
        if self.eat(&Token::Else) {
            Ok(Some(self.body()?))
        } else {
            self.at = saved;
            Ok(None)
        }
    }
    fn finally_body(&mut self) -> Result<Option<Expr>, Error> {
        let saved = self.at;
        while self.eat(&Token::Semi) {}
        if self.eat(&Token::Finally) {
            Ok(Some(self.body()?))
        } else {
            self.at = saved;
            Ok(None)
        }
    }
    fn layout_block(&mut self) -> Result<Expr, Error> {
        self.expect(&Token::Indent)?;
        let mut statements = vec![];
        loop {
            while self.eat(&Token::Semi) {}
            if self.eat(&Token::Dedent) {
                break;
            }
            if matches!(self.peek(), Token::Eof) {
                return Err(self.parse_error("unterminated indentation block"));
            }
            statements.push(self.statement()?);
            if !matches!(self.peek(), Token::Semi | Token::Dedent) {
                return Err(self.parse_error("expected statement separator in indentation block"));
            }
        }
        Ok(Expr::Block(statements))
    }
    fn switch_cases(&mut self) -> Result<SwitchCases, Error> {
        self.expect(&Token::Semi)?;
        while self.eat(&Token::Semi) {}
        self.expect(&Token::Indent)?;
        let mut cases = vec![];
        let mut fallback = None;
        loop {
            while self.eat(&Token::Semi) {}
            if self.eat(&Token::Dedent) {
                break;
            }
            if self.eat(&Token::When) {
                if fallback.is_some() {
                    return Err(self.parse_error("when cannot follow else in switch"));
                }
                let mut patterns = vec![self.expr(0)?];
                while self.eat(&Token::Comma) {
                    patterns.push(self.expr(0)?);
                }
                let body = self.body()?;
                cases.push((patterns, body));
                continue;
            }
            if self.eat(&Token::Else) {
                if fallback.is_some() {
                    return Err(self.parse_error("duplicate else in switch"));
                }
                fallback = Some(self.body()?);
                continue;
            }
            return Err(self.parse_error("expected when or else in switch"));
        }
        if cases.is_empty() {
            return Err(self.parse_error("switch requires at least one when case"));
        }
        Ok((cases, fallback))
    }
    fn expr(&mut self, min: u8) -> Result<Expr, Error> {
        let mut left = self.prefix()?;
        loop {
            if self.eat(&Token::LParen) {
                let args = self.arguments(Token::RParen)?;
                left = Expr::Call(Box::new(left), args);
                continue;
            }
            if self.eat(&Token::LBracket) {
                let start = self.expr(0)?;
                let inclusive = if self.eat(&Token::RangeInclusive) {
                    Some(true)
                } else if self.eat(&Token::Ellipsis) {
                    Some(false)
                } else {
                    None
                };
                if let Some(inclusive) = inclusive {
                    let end = self.expr(0)?;
                    self.expect(&Token::RBracket)?;
                    left = Expr::Slice(Box::new(left), Box::new(start), Box::new(end), inclusive);
                    continue;
                }
                self.expect(&Token::RBracket)?;
                left = Expr::Index(Box::new(left), Box::new(start));
                continue;
            }
            if self.eat(&Token::Dot) {
                let name = match self.next() {
                    Token::Ident(name) => name,
                    got => {
                        return Err(
                            self.parse_error(format!("expected member name, found {got:?}"))
                        );
                    }
                };
                left = Expr::Member(Box::new(left), name);
                continue;
            }
            if let Some(update) = postfix_update(self.peek()) {
                self.next();
                let Expr::Name(name) = left else {
                    return Err(self.parse_error("increment and decrement require a plain name"));
                };
                left = Expr::Update(name, update, false);
                continue;
            }
            if matches!(self.peek(), Token::Question)
                && matches!(
                    self.tokens.get(self.at + 1),
                    Some(Token::LParen | Token::LBracket | Token::Dot)
                )
            {
                self.at += 1;
                match self.next() {
                    Token::LParen => {
                        let args = self.arguments(Token::RParen)?;
                        left = Expr::SoakCall(Box::new(left), args);
                    }
                    Token::LBracket => {
                        let start = self.expr(0)?;
                        let inclusive = if self.eat(&Token::RangeInclusive) {
                            Some(true)
                        } else if self.eat(&Token::Ellipsis) {
                            Some(false)
                        } else {
                            None
                        };
                        if let Some(inclusive) = inclusive {
                            let end = self.expr(0)?;
                            self.expect(&Token::RBracket)?;
                            left = Expr::SoakSlice(
                                Box::new(left),
                                Box::new(start),
                                Box::new(end),
                                inclusive,
                            );
                            continue;
                        }
                        self.expect(&Token::RBracket)?;
                        left = Expr::SoakIndex(Box::new(left), Box::new(start));
                    }
                    Token::Dot => {
                        let name = match self.next() {
                            Token::Ident(name) => name,
                            got => {
                                return Err(self
                                    .parse_error(format!("expected member name, found {got:?}")));
                            }
                        };
                        left = Expr::SoakMember(Box::new(left), name);
                    }
                    _ => unreachable!("guarded soak suffix"),
                }
                continue;
            }
            if matches!(self.peek(), Token::Question)
                && !self.question_starts_expression(self.at + 1)
            {
                self.at += 1;
                left = Expr::Exists(Box::new(left));
                continue;
            }
            if min == 0 && self.eat(&Token::If) {
                let condition = self.expr(0)?;
                left = Expr::If(Box::new(condition), Box::new(left), Box::new(Expr::Nil));
                continue;
            }
            if min == 0 && self.eat(&Token::Unless) {
                let condition = self.expr(0)?;
                left = Expr::If(
                    Box::new(Expr::Unary(Unary::Not, Box::new(condition))),
                    Box::new(left),
                    Box::new(Expr::Nil),
                );
                continue;
            }
            if min == 0 && self.eat(&Token::For) {
                left = self.for_tail(left)?;
                continue;
            }
            let negated_membership = match (self.peek(), self.tokens.get(self.at + 1)) {
                (Token::Not, Some(Token::In)) => Some(Binary::NotIn),
                (Token::Not, Some(Token::Of)) => Some(Binary::NotOf),
                _ => None,
            };
            if let Some(op) = negated_membership {
                const MEMBERSHIP_PRECEDENCE: u8 = 7;
                if MEMBERSHIP_PRECEDENCE < min {
                    break;
                }
                self.at += 2;
                let right = self.expr(MEMBERSHIP_PRECEDENCE + 1)?;
                left = Expr::Binary(Box::new(left), op, Box::new(right));
                continue;
            }
            let (op, prec) = match self.peek() {
                Token::Question => (Binary::Coalesce, 0),
                Token::Or => (Binary::Or, 1),
                Token::And => (Binary::And, 2),
                Token::Pipe => (Binary::BitOr, 3),
                Token::Caret => (Binary::BitXor, 4),
                Token::Amp => (Binary::BitAnd, 5),
                Token::EqEq => (Binary::Eq, 6),
                Token::NotEq => (Binary::Ne, 6),
                Token::Lt => (Binary::Lt, 7),
                Token::LtEq => (Binary::Le, 7),
                Token::Gt => (Binary::Gt, 7),
                Token::GtEq => (Binary::Ge, 7),
                Token::In => (Binary::In, 7),
                Token::Of => (Binary::Of, 7),
                Token::ShiftLeft => (Binary::ShiftLeft, 8),
                Token::ShiftRight => (Binary::ShiftRight, 8),
                Token::ShiftRightUnsigned => (Binary::ShiftRightUnsigned, 8),
                Token::Plus => (Binary::Add, 9),
                Token::Minus => (Binary::Sub, 9),
                Token::Star => (Binary::Mul, 10),
                Token::Slash => (Binary::Div, 10),
                Token::FloorDiv => (Binary::FloorDiv, 10),
                Token::Percent => (Binary::Rem, 10),
                Token::Modulo => (Binary::Modulo, 10),
                Token::Power => (Binary::Pow, 11),
                _ => break,
            };
            if prec < min {
                break;
            }
            self.next();
            let right = self.expr(if matches!(op, Binary::Pow) {
                prec
            } else {
                prec + 1
            })?;
            if is_chain_comparison(op) {
                let mut operands = vec![left, right];
                let mut operators = vec![op];
                while let Some(next) = comparison_at(self.peek()) {
                    if next.1 != prec {
                        break;
                    }
                    self.next();
                    operators.push(next.0);
                    operands.push(self.expr(prec + 1)?);
                }
                left = if operators.len() == 1 {
                    Expr::Binary(
                        Box::new(operands.remove(0)),
                        operators[0],
                        Box::new(operands.remove(0)),
                    )
                } else {
                    Expr::CompareChain(operands, operators)
                };
            } else {
                left = Expr::Binary(Box::new(left), op, Box::new(right));
            }
        }
        Ok(left)
    }
    fn question_starts_expression(&self, index: usize) -> bool {
        matches!(
            self.tokens.get(index),
            Some(
                Token::Number(_)
                    | Token::String(_, _)
                    | Token::Ident(_)
                    | Token::True
                    | Token::False
                    | Token::Nil
                    | Token::If
                    | Token::Unless
                    | Token::While
                    | Token::Until
                    | Token::Loop
                    | Token::For
                    | Token::Break
                    | Token::Continue
                    | Token::Return
                    | Token::Class
                    | Token::Switch
                    | Token::Try
                    | Token::Throw
                    | Token::Do
                    | Token::Not
                    | Token::PlusPlus
                    | Token::Minus
                    | Token::MinusMinus
                    | Token::Tilde
                    | Token::LParen
                    | Token::Arrow
                    | Token::FatArrow
                    | Token::LBracket
                    | Token::LBrace
            )
        )
    }
    fn prefix(&mut self) -> Result<Expr, Error> {
        match self.next() {
            Token::Number(n) => Ok(Expr::Number(n)),
            Token::String(s, interpolate) => {
                if interpolate && s.contains("#{") {
                    self.interpolated_string(s)
                } else {
                    Ok(Expr::String(s))
                }
            }
            Token::True => Ok(Expr::Bool(true)),
            Token::False => Ok(Expr::Bool(false)),
            Token::Nil => Ok(Expr::Nil),
            Token::Ident(n) => {
                if self.eat(&Token::Arrow) || self.eat(&Token::FatArrow) {
                    return Ok(Expr::Function(
                        vec![Param {
                            pattern: Pattern::Bind(n),
                            default: None,
                        }],
                        None,
                        Box::new(self.body_after_arrow()?),
                    ));
                }
                let after_name = self.at;
                let mut names = vec![n.clone()];
                while self.eat(&Token::Comma) {
                    match self.next() {
                        Token::Ident(name) => names.push(name),
                        _ => {
                            self.at = after_name;
                            break;
                        }
                    }
                }
                if self.eat(&Token::Arrow) || self.eat(&Token::FatArrow) {
                    return Ok(Expr::Function(
                        names
                            .into_iter()
                            .map(|name| Param {
                                pattern: Pattern::Bind(name),
                                default: None,
                            })
                            .collect(),
                        None,
                        Box::new(self.body_after_arrow()?),
                    ));
                }
                self.at = after_name;
                if self.eat(&Token::Assign) {
                    Ok(Expr::Assign(n, Box::new(self.expr(0)?)))
                } else if matches!(self.peek(), Token::Question)
                    && matches!(self.tokens.get(self.at + 1), Some(Token::Assign))
                {
                    self.at += 2;
                    Ok(Expr::AssignIfNil(n, Box::new(self.expr(0)?)))
                } else if let Some(op) = compound_operator(self.peek()) {
                    self.next();
                    Ok(Expr::Assign(
                        n.clone(),
                        Box::new(Expr::Binary(
                            Box::new(Expr::Name(n)),
                            op,
                            Box::new(self.expr(0)?),
                        )),
                    ))
                } else {
                    Ok(Expr::Name(n))
                }
            }
            Token::Minus => Ok(Expr::Unary(Unary::Neg, Box::new(self.expr(12)?))),
            Token::Not => Ok(Expr::Unary(Unary::Not, Box::new(self.expr(12)?))),
            Token::Tilde => Ok(Expr::Unary(Unary::BitNot, Box::new(self.expr(12)?))),
            Token::PlusPlus => self.prefix_update(Update::Increment),
            Token::MinusMinus => self.prefix_update(Update::Decrement),
            Token::If => {
                let c = self.expr(0)?;
                let yes = self.body()?;
                let no = self.else_body()?.unwrap_or(Expr::Nil);
                Ok(Expr::If(Box::new(c), Box::new(yes), Box::new(no)))
            }
            Token::Unless => {
                let condition = self.expr(0)?;
                let yes = self.body()?;
                let no = self.else_body()?.unwrap_or(Expr::Nil);
                Ok(Expr::If(
                    Box::new(Expr::Unary(Unary::Not, Box::new(condition))),
                    Box::new(yes),
                    Box::new(no),
                ))
            }
            Token::While => {
                let c = self.expr(0)?;
                let body = self.body()?;
                Ok(Expr::While(Box::new(c), Box::new(body)))
            }
            Token::Until => {
                let c = self.expr(0)?;
                let body = self.body()?;
                Ok(Expr::While(
                    Box::new(Expr::Unary(Unary::Not, Box::new(c))),
                    Box::new(body),
                ))
            }
            Token::Loop => Ok(Expr::While(
                Box::new(Expr::Bool(true)),
                Box::new(self.body()?),
            )),
            Token::For => self.for_prefix(),
            Token::Break => Ok(Expr::Break),
            Token::Continue => Ok(Expr::Continue),
            Token::Return => {
                let value = if matches!(
                    self.peek(),
                    Token::Semi
                        | Token::Dedent
                        | Token::Eof
                        | Token::Else
                        | Token::Catch
                        | Token::Finally
                ) {
                    None
                } else {
                    Some(Box::new(self.expr(0)?))
                };
                Ok(Expr::Return(value))
            }
            Token::Class => {
                let name = match self.next() {
                    Token::Ident(name) => name,
                    got => {
                        return Err(self.parse_error(format!("expected class name, found {got:?}")));
                    }
                };
                let params = if self.eat(&Token::LParen) {
                    self.parameters()?
                } else {
                    vec![]
                };
                self.expect(&Token::Arrow)?;
                Ok(Expr::Class(
                    name,
                    params,
                    Box::new(self.body_after_arrow()?),
                ))
            }
            Token::Switch => {
                let subject = self.expr(0)?;
                let (cases, fallback) = self.switch_cases()?;
                Ok(Expr::Switch(
                    Box::new(subject),
                    cases,
                    fallback.map(Box::new),
                ))
            }
            Token::Try => {
                let body = self.body()?;
                while self.eat(&Token::Semi) {}
                self.expect(&Token::Catch)?;
                let name = match self.next() {
                    Token::Ident(name) => name,
                    got => {
                        return Err(self.parse_error(format!("expected catch name, found {got:?}")));
                    }
                };
                let handler = self.body()?;
                let finalizer = self.finally_body()?.map(Box::new);
                Ok(Expr::Try(
                    Box::new(body),
                    name,
                    Box::new(handler),
                    finalizer,
                ))
            }
            Token::Throw => Ok(Expr::Throw(Box::new(self.expr(0)?))),
            Token::Do => Ok(Expr::Do(Box::new(self.expr(0)?))),
            Token::LParen => {
                if let Some(params) = self.lambda_params()? {
                    self.expect_arrow()?;
                    return Ok(Expr::Function(
                        params.0,
                        params.1,
                        Box::new(self.body_after_arrow()?),
                    ));
                }
                let x = self.expr(0)?;
                self.expect(&Token::RParen)?;
                Ok(x)
            }
            Token::Arrow => Ok(Expr::Function(
                vec![],
                None,
                Box::new(self.body_after_arrow()?),
            )),
            Token::FatArrow => Ok(Expr::Function(
                vec![],
                None,
                Box::new(self.body_after_arrow()?),
            )),
            Token::LBracket => {
                if self.eat(&Token::RBracket) {
                    return Ok(Expr::Array(vec![]));
                }
                let start = self.expr(0)?;
                let inclusive = if self.eat(&Token::RangeInclusive) {
                    Some(true)
                } else if matches!(self.peek(), Token::Ellipsis)
                    && !matches!(
                        self.tokens.get(self.at + 1),
                        Some(Token::Comma) | Some(Token::RBracket)
                    )
                {
                    self.at += 1;
                    Some(false)
                } else {
                    None
                };
                if let Some(inclusive) = inclusive {
                    let end = self.expr(0)?;
                    self.expect(&Token::RBracket)?;
                    Ok(Expr::Range(Box::new(start), Box::new(end), inclusive))
                } else if matches!(&start, Expr::For(..)) {
                    // CoffeeScript-style `[value for item in items]` is a
                    // comprehension wrapper, not an additional nested array.
                    self.expect(&Token::RBracket)?;
                    Ok(start)
                } else {
                    let mut items = vec![if self.eat(&Token::Ellipsis) {
                        Item::Splat(start)
                    } else {
                        Item::Expr(start)
                    }];
                    while self.eat(&Token::Comma) {
                        if self.eat(&Token::RBracket) {
                            return Ok(Expr::Array(items));
                        }
                        let item = self.expr(0)?;
                        items.push(if self.eat(&Token::Ellipsis) {
                            Item::Splat(item)
                        } else {
                            Item::Expr(item)
                        });
                    }
                    self.expect(&Token::RBracket)?;
                    Ok(Expr::Array(items))
                }
            }
            Token::LBrace => {
                let mut m = BTreeMap::new();
                if self.eat(&Token::RBrace) {
                    return Ok(Expr::Map(m));
                }
                loop {
                    let key = self.next();
                    let k = match &key {
                        Token::Ident(s) | Token::String(s, _) => s.clone(),
                        _ => return Err(self.parse_error("map key must be identifier or string")),
                    };
                    let value = if self.eat(&Token::Colon) {
                        self.expr(0)?
                    } else if matches!(key, Token::Ident(_)) {
                        Expr::Name(k.clone())
                    } else {
                        return Err(self.parse_error("string map keys require ':' and a value"));
                    };
                    m.insert(k, value);
                    if self.eat(&Token::RBrace) {
                        break;
                    }
                    self.expect(&Token::Comma)?;
                }
                Ok(Expr::Map(m))
            }
            x => Err(self.parse_error(format!("expected expression, found {x:?}"))),
        }
    }
    fn for_prefix(&mut self) -> Result<Expr, Error> {
        let (patterns, map, iterable, step, filter) = self.for_header()?;
        let body = self.body()?;
        Ok(Expr::For(
            patterns,
            map,
            Box::new(iterable),
            step,
            filter,
            Box::new(body),
        ))
    }
    fn prefix_update(&mut self, update: Update) -> Result<Expr, Error> {
        let name = match self.next() {
            Token::Ident(name) => name,
            got => {
                return Err(self.parse_error(format!(
                    "increment and decrement require a plain name, found {got:?}"
                )));
            }
        };
        Ok(Expr::Update(name, update, true))
    }
    fn for_tail(&mut self, body: Expr) -> Result<Expr, Error> {
        let (patterns, map, iterable, step, filter) = self.for_header()?;
        Ok(Expr::For(
            patterns,
            map,
            Box::new(iterable),
            step,
            filter,
            Box::new(body),
        ))
    }
    fn for_header(&mut self) -> Result<ForHeader, Error> {
        self.eat(&Token::Own);
        let mut patterns = vec![];
        loop {
            let pattern = self
                .pattern()
                .ok_or_else(|| self.parse_error("expected loop binding pattern"))?;
            patterns.push(pattern);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        let map = if self.eat(&Token::In) {
            false
        } else if self.eat(&Token::Of) {
            true
        } else {
            return Err(self.parse_error("expected in or of after loop binding"));
        };
        if (!map && !(1..=2).contains(&patterns.len())) || (map && patterns.len() != 2) {
            return Err(self.parse_error(if map {
                "map iteration requires key and value patterns"
            } else {
                "array iteration requires a value pattern and optional index pattern"
            }));
        }
        let iterable = self.expr(0)?;
        let step = if self.eat(&Token::By) {
            if map {
                return Err(self.parse_error("by is supported only for array iteration"));
            }
            Some(Box::new(self.expr(0)?))
        } else {
            None
        };
        let filter = if self.eat(&Token::When) {
            Some(Box::new(self.expr(0)?))
        } else {
            None
        };
        Ok((patterns, map, iterable, step, filter))
    }
    fn lambda_params(&mut self) -> Result<Option<LambdaParams>, Error> {
        let saved = self.at;
        let mut p = vec![];
        let mut saw_default = false;
        let mut misordered_default = false;
        if self.eat(&Token::RParen) {
            if matches!(self.peek(), Token::Arrow | Token::FatArrow) {
                return Ok(Some((p, None)));
            }
            self.at = saved;
            return Ok(None);
        }
        loop {
            let Some(pattern) = self.pattern() else {
                self.at = saved;
                return Ok(None);
            };
            if self.eat(&Token::Ellipsis) {
                let Pattern::Bind(name) = pattern else {
                    return Err(self.parse_error("rest parameter must be a name"));
                };
                if !self.eat(&Token::RParen) {
                    self.at = saved;
                    return Ok(None);
                }
                if matches!(self.peek(), Token::Arrow | Token::FatArrow) {
                    if misordered_default {
                        return Err(
                            self.parse_error("required parameters must precede default parameters")
                        );
                    }
                    return Ok(Some((p, Some(name))));
                }
                self.at = saved;
                return Ok(None);
            }
            let default = if self.eat(&Token::Assign) {
                if !matches!(pattern, Pattern::Bind(_)) {
                    return Err(self.parse_error("default parameter must be a name"));
                }
                saw_default = true;
                Some(self.expr(0)?)
            } else {
                if saw_default {
                    misordered_default = true;
                }
                None
            };
            p.push(Param { pattern, default });
            if self.eat(&Token::RParen) {
                break;
            }
            if !self.eat(&Token::Comma) {
                self.at = saved;
                return Ok(None);
            }
        }
        if matches!(self.peek(), Token::Arrow | Token::FatArrow) {
            if misordered_default {
                return Err(self.parse_error("required parameters must precede default parameters"));
            }
            Ok(Some((p, None)))
        } else {
            self.at = saved;
            Ok(None)
        }
    }
    fn expect_arrow(&mut self) -> Result<(), Error> {
        if matches!(self.peek(), Token::Arrow | Token::FatArrow) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.parse_error(format!("expected function arrow, found {:?}", self.peek())))
        }
    }
    fn parameters(&mut self) -> Result<Vec<Param>, Error> {
        let mut params = vec![];
        let mut saw_default = false;
        if self.eat(&Token::RParen) {
            return Ok(params);
        }
        loop {
            let pattern = self
                .pattern()
                .ok_or_else(|| self.parse_error("expected parameter pattern"))?;
            let default = if self.eat(&Token::Assign) {
                if !matches!(pattern, Pattern::Bind(_)) {
                    return Err(self.parse_error("default parameter must be a name"));
                }
                saw_default = true;
                Some(self.expr(0)?)
            } else {
                if saw_default {
                    return Err(
                        self.parse_error("required parameters must precede default parameters")
                    );
                }
                None
            };
            params.push(Param { pattern, default });
            if self.eat(&Token::RParen) {
                return Ok(params);
            }
            self.expect(&Token::Comma)?;
        }
    }
    fn interpolated_string(&self, source: String) -> Result<Expr, Error> {
        let mut pieces = vec![];
        let mut remainder = source.as_str();
        while let Some(start) = remainder.find("#{") {
            if start > 0 {
                pieces.push(Expr::String(remainder[..start].to_owned()));
            }
            let expression_start = start + 2;
            let mut depth = 1usize;
            let mut end = None;
            let mut quoted = None;
            let mut escaped = false;
            for (offset, ch) in remainder[expression_start..].char_indices() {
                if let Some(quote) = quoted {
                    if escaped {
                        escaped = false;
                    } else if ch == '\\' {
                        escaped = true;
                    } else if ch == quote {
                        quoted = None;
                    }
                    continue;
                }
                match ch {
                    '\'' | '"' => quoted = Some(ch),
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(expression_start + offset);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let end = end.ok_or_else(|| self.parse_error("unterminated string interpolation"))?;
            let (tokens, mut lines) = lex_spanned(remainder[expression_start..end].trim())?;
            let source_line = self
                .lines
                .get(self.at)
                .or_else(|| self.lines.last())
                .copied()
                .unwrap_or(1);
            lines.fill(source_line);
            let mut expression_parser = Parser {
                tokens,
                lines,
                at: 0,
            };
            let expression = expression_parser.expr(0)?;
            if !matches!(expression_parser.peek(), Token::Semi | Token::Eof) {
                return Err(self.parse_error("interpolation must contain one expression"));
            }
            pieces.push(expression);
            remainder = &remainder[end + 1..];
        }
        if !remainder.is_empty() {
            pieces.push(Expr::String(remainder.to_owned()));
        }
        Ok(Expr::Interpolate(pieces))
    }
    fn arguments(&mut self, close: Token) -> Result<Vec<Item>, Error> {
        let mut a = vec![];
        if self.eat(&close) {
            return Ok(a);
        }
        loop {
            let item = self.expr(0)?;
            a.push(if self.eat(&Token::Ellipsis) {
                Item::Splat(item)
            } else {
                Item::Expr(item)
            });
            if self.eat(&close) {
                break;
            }
            self.expect(&Token::Comma)?
        }
        Ok(a)
    }
}
fn comparison_at(token: &Token) -> Option<(Binary, u8)> {
    match token {
        Token::EqEq => Some((Binary::Eq, 6)),
        Token::NotEq => Some((Binary::Ne, 6)),
        Token::Lt => Some((Binary::Lt, 7)),
        Token::LtEq => Some((Binary::Le, 7)),
        Token::Gt => Some((Binary::Gt, 7)),
        Token::GtEq => Some((Binary::Ge, 7)),
        _ => None,
    }
}
fn is_chain_comparison(op: Binary) -> bool {
    matches!(
        op,
        Binary::Eq | Binary::Ne | Binary::Lt | Binary::Le | Binary::Gt | Binary::Ge
    )
}

fn compound_operator(token: &Token) -> Option<Binary> {
    match token {
        Token::PlusAssign => Some(Binary::Add),
        Token::MinusAssign => Some(Binary::Sub),
        Token::StarAssign => Some(Binary::Mul),
        Token::SlashAssign => Some(Binary::Div),
        Token::FloorDivAssign => Some(Binary::FloorDiv),
        Token::PercentAssign => Some(Binary::Rem),
        Token::ModuloAssign => Some(Binary::Modulo),
        Token::AmpAssign => Some(Binary::BitAnd),
        Token::PipeAssign => Some(Binary::BitOr),
        Token::CaretAssign => Some(Binary::BitXor),
        Token::ShiftLeftAssign => Some(Binary::ShiftLeft),
        Token::ShiftRightAssign => Some(Binary::ShiftRight),
        Token::ShiftRightUnsignedAssign => Some(Binary::ShiftRightUnsigned),
        Token::PowerAssign => Some(Binary::Pow),
        _ => None,
    }
}

fn postfix_update(token: &Token) -> Option<Update> {
    match token {
        Token::PlusPlus => Some(Update::Increment),
        Token::MinusMinus => Some(Update::Decrement),
        _ => None,
    }
}
