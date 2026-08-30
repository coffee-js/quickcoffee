//! Resource-bounded, data-only CSON parsing.

use crate::vm::{decimal_text_resource_preflight, integer_digits_resource_preflight};
use crate::{Decimal, Integer, ResourceLimits, SourcePosition, SourceSpan, Value};
use num_bigint::BigInt;
use std::{collections::BTreeMap, error, fmt, ops::Range, rc::Rc};

const IMPLEMENTATION_MAX_RECURSION_DEPTH: usize = 128;

/// Stable category for a CSON parsing failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CsonErrorCode {
    /// The input is not valid CSON syntax.
    Syntax,
    /// Indentation is mixed, fractional, or structurally invalid.
    Indentation,
    /// A Map repeats a decoded key.
    DuplicateKey,
    /// A string contains CoffeeScript interpolation.
    Interpolation,
    /// The input contains an executable or computed expression.
    Expression,
    /// A bare identifier was used as a value.
    IdentifierValue,
    /// A numeric token is malformed or exceeds a numeric boundary.
    Number,
    /// The UTF-8 input byte boundary was exceeded.
    InputLimit,
    /// A decoded String byte boundary was exceeded.
    StringLimit,
    /// The total parsed Value boundary was exceeded.
    ValueLimit,
    /// A single Array or Map item boundary was exceeded.
    ContainerLimit,
    /// The configured or implementation-safe nesting boundary was exceeded.
    DepthLimit,
    /// The deterministic parser-work boundary was exceeded.
    WorkLimit,
    /// The diagnostic boundary does not permit another diagnostic.
    DiagnosticLimit,
}

impl CsonErrorCode {
    /// Returns the stable machine-readable error code.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syntax => "E_CSON_SYNTAX",
            Self::Indentation => "E_CSON_INDENTATION",
            Self::DuplicateKey => "E_CSON_DUPLICATE_KEY",
            Self::Interpolation => "E_CSON_INTERPOLATION",
            Self::Expression => "E_CSON_EXPRESSION",
            Self::IdentifierValue => "E_CSON_IDENTIFIER_VALUE",
            Self::Number => "E_CSON_NUMBER",
            Self::InputLimit => "E_CSON_INPUT_LIMIT",
            Self::StringLimit => "E_CSON_STRING_LIMIT",
            Self::ValueLimit => "E_CSON_VALUE_LIMIT",
            Self::ContainerLimit => "E_CSON_CONTAINER_LIMIT",
            Self::DepthLimit => "E_CSON_DEPTH_LIMIT",
            Self::WorkLimit => "E_CSON_WORK_LIMIT",
            Self::DiagnosticLimit => "E_CSON_DIAGNOSTIC_LIMIT",
        }
    }
}

impl fmt::Display for CsonErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A stable CSON error with an original-input byte range and physical source span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsonError {
    code: CsonErrorCode,
    message: String,
    byte_start: usize,
    byte_end: usize,
    span: SourceSpan,
}

impl CsonError {
    /// Returns the stable error category.
    pub const fn code(&self) -> CsonErrorCode {
        self.code
    }

    /// Returns the human-readable error detail without the stable code or location.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the half-open UTF-8 byte range in the original input.
    pub fn byte_range(&self) -> Range<usize> {
        self.byte_start..self.byte_end
    }

    /// Returns the one-based physical source span in the original input.
    pub fn span(&self) -> &SourceSpan {
        &self.span
    }

    fn original(
        source: &str,
        start: usize,
        end: usize,
        code: CsonErrorCode,
        message: impl Into<String>,
        max_diagnostics: usize,
    ) -> Self {
        let (code, message) = if max_diagnostics == 0 && code != CsonErrorCode::DiagnosticLimit {
            (
                CsonErrorCode::DiagnosticLimit,
                "CSON diagnostic count exceeds 0".to_owned(),
            )
        } else {
            (code, message.into())
        };
        let start = start.min(source.len());
        let end = end.max(start).min(source.len());
        Self {
            code,
            message,
            byte_start: start,
            byte_end: end,
            span: SourceSpan {
                source_name: None,
                start: source_position(source, start),
                end: Some(source_position(source, end)),
            },
        }
    }
}

impl fmt::Display for CsonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let position = self.span.start;
        match position.column {
            Some(column) => write!(
                formatter,
                "{} at {}:{}: {}",
                self.code, position.line, column, self.message
            ),
            None => write!(
                formatter,
                "{} at line {}: {}",
                self.code, position.line, self.message
            ),
        }
    }
}

impl error::Error for CsonError {}

/// Deterministic data, numeric, nesting, work, and diagnostic limits for CSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsonLimits {
    max_input_bytes: usize,
    max_output_bytes: usize,
    max_string_bytes: usize,
    max_values: usize,
    max_container_items: usize,
    max_nesting_depth: usize,
    max_integer_bits: u64,
    max_decimal_coefficient_bits: u64,
    max_decimal_scale: u32,
    max_work_units: usize,
    max_diagnostics: usize,
}

impl Default for CsonLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 1_000_000,
            max_output_bytes: 1_000_000,
            max_string_bytes: 1_000_000,
            max_values: 100_000,
            max_container_items: 100_000,
            max_nesting_depth: 128,
            max_integer_bits: 1_000_000,
            max_decimal_coefficient_bits: 1_000_000,
            max_decimal_scale: 100_000,
            max_work_units: 4_000_000,
            max_diagnostics: 32,
        }
    }
}

impl CsonLimits {
    /// Returns the maximum accepted UTF-8 input bytes.
    pub const fn max_input_bytes(&self) -> usize {
        self.max_input_bytes
    }

    /// Returns a policy with a replacement input-byte boundary.
    pub const fn with_max_input_bytes(mut self, limit: usize) -> Self {
        self.max_input_bytes = limit;
        self
    }

    /// Returns the reserved maximum canonical output bytes for the later serializer.
    pub const fn max_output_bytes(&self) -> usize {
        self.max_output_bytes
    }

    /// Returns a policy with a replacement canonical-output boundary.
    pub const fn with_max_output_bytes(mut self, limit: usize) -> Self {
        self.max_output_bytes = limit;
        self
    }

    /// Returns the maximum UTF-8 bytes in one decoded String or Map key.
    pub const fn max_string_bytes(&self) -> usize {
        self.max_string_bytes
    }

    /// Returns a policy with a replacement decoded-String boundary.
    pub const fn with_max_string_bytes(mut self, limit: usize) -> Self {
        self.max_string_bytes = limit;
        self
    }

    /// Returns the maximum total Values created by one parse.
    pub const fn max_values(&self) -> usize {
        self.max_values
    }

    /// Returns a policy with a replacement total-Value boundary.
    pub const fn with_max_values(mut self, limit: usize) -> Self {
        self.max_values = limit;
        self
    }

    /// Returns the maximum items in one Array or Map.
    pub const fn max_container_items(&self) -> usize {
        self.max_container_items
    }

    /// Returns a policy with a replacement per-container item boundary.
    pub const fn with_max_container_items(mut self, limit: usize) -> Self {
        self.max_container_items = limit;
        self
    }

    /// Returns the configured collection nesting boundary.
    pub const fn max_nesting_depth(&self) -> usize {
        self.max_nesting_depth
    }

    /// Returns a policy with a replacement collection nesting boundary.
    pub const fn with_max_nesting_depth(mut self, limit: usize) -> Self {
        self.max_nesting_depth = limit;
        self
    }

    /// Returns the maximum Integer magnitude bits.
    pub const fn max_integer_bits(&self) -> u64 {
        self.max_integer_bits
    }

    /// Returns a policy with a replacement Integer magnitude boundary.
    pub const fn with_max_integer_bits(mut self, limit: u64) -> Self {
        self.max_integer_bits = limit;
        self
    }

    /// Returns the maximum Decimal coefficient magnitude bits.
    pub const fn max_decimal_coefficient_bits(&self) -> u64 {
        self.max_decimal_coefficient_bits
    }

    /// Returns a policy with a replacement Decimal coefficient boundary.
    pub const fn with_max_decimal_coefficient_bits(mut self, limit: u64) -> Self {
        self.max_decimal_coefficient_bits = limit;
        self
    }

    /// Returns the maximum Decimal scale.
    pub const fn max_decimal_scale(&self) -> u32 {
        self.max_decimal_scale
    }

    /// Returns a policy with a replacement Decimal scale boundary.
    pub const fn with_max_decimal_scale(mut self, limit: u32) -> Self {
        self.max_decimal_scale = limit;
        self
    }

    /// Returns the maximum deterministic lexer and parser work units.
    pub const fn max_work_units(&self) -> usize {
        self.max_work_units
    }

    /// Returns a policy with a replacement deterministic work boundary.
    pub const fn with_max_work_units(mut self, limit: usize) -> Self {
        self.max_work_units = limit;
        self
    }

    /// Returns the maximum diagnostics available to a parse operation.
    pub const fn max_diagnostics(&self) -> usize {
        self.max_diagnostics
    }

    /// Returns a policy with a replacement diagnostic boundary.
    pub const fn with_max_diagnostics(mut self, limit: usize) -> Self {
        self.max_diagnostics = limit;
        self
    }

    fn numeric_resource_limits(self) -> ResourceLimits {
        ResourceLimits::default()
            .with_max_integer_bits(self.max_integer_bits)
            .with_max_decimal_coefficient_bits(self.max_decimal_coefficient_bits)
            .with_max_decimal_scale(self.max_decimal_scale)
    }
}

/// Parses data-only CSON with [`CsonLimits::default`].
pub fn parse_cson(source: &str) -> Result<Value, CsonError> {
    parse_cson_with_limits(source, CsonLimits::default())
}

/// Parses data-only CSON under explicit deterministic limits.
pub fn parse_cson_with_limits(source: &str, limits: CsonLimits) -> Result<Value, CsonError> {
    let normalized = NormalizedSource::new(source, limits)?;
    let (tokens, work) = Lexer::new(&normalized, limits).lex()?;
    Parser::new(&normalized, &tokens, limits, work).parse_document()
}

fn source_position(source: &str, offset: usize) -> SourcePosition {
    let bytes = source.as_bytes();
    let mut cursor = 0;
    let mut line = 1;
    let mut column = 1;
    while cursor < offset.min(bytes.len()) {
        match bytes[cursor] {
            b'\r' if bytes.get(cursor + 1) == Some(&b'\n') => {
                cursor += 2;
                line += 1;
                column = 1;
            }
            b'\n' => {
                cursor += 1;
                line += 1;
                column = 1;
            }
            _ => {
                let character = source[cursor..]
                    .chars()
                    .next()
                    .expect("cursor remains on a UTF-8 boundary");
                cursor += character.len_utf8();
                column += 1;
            }
        }
    }
    SourcePosition {
        line,
        column: Some(column),
    }
}

struct NormalizedSource<'a> {
    original: &'a str,
    text: String,
    original_offsets: Vec<usize>,
    limits: CsonLimits,
}

impl<'a> NormalizedSource<'a> {
    fn new(source: &'a str, limits: CsonLimits) -> Result<Self, CsonError> {
        if source.len() > limits.max_input_bytes {
            return Err(CsonError::original(
                source,
                0,
                source.len().min(1),
                CsonErrorCode::InputLimit,
                format!("CSON input exceeds {} bytes", limits.max_input_bytes),
                limits.max_diagnostics,
            ));
        }
        if source.starts_with('\u{feff}') {
            return Err(CsonError::original(
                source,
                0,
                '\u{feff}'.len_utf8(),
                CsonErrorCode::Syntax,
                "CSON v1 rejects a UTF-8 BOM",
                limits.max_diagnostics,
            ));
        }

        let bytes = source.as_bytes();
        let mut normalized = Vec::with_capacity(bytes.len());
        let mut original_offsets = Vec::with_capacity(bytes.len() + 1);
        let mut cursor = 0;
        while cursor < bytes.len() {
            if bytes[cursor] == b'\r' {
                if bytes.get(cursor + 1) != Some(&b'\n') {
                    return Err(CsonError::original(
                        source,
                        cursor,
                        cursor + 1,
                        CsonErrorCode::Syntax,
                        "standalone carriage return is not valid CSON v1",
                        limits.max_diagnostics,
                    ));
                }
                original_offsets.push(cursor);
                normalized.push(b'\n');
                cursor += 2;
            } else {
                original_offsets.push(cursor);
                normalized.push(bytes[cursor]);
                cursor += 1;
            }
        }
        original_offsets.push(source.len());
        Ok(Self {
            original: source,
            text: String::from_utf8(normalized).expect("normalizing UTF-8 preserves UTF-8"),
            original_offsets,
            limits,
        })
    }

    fn error(
        &self,
        start: usize,
        end: usize,
        code: CsonErrorCode,
        message: impl Into<String>,
    ) -> CsonError {
        let start = start.min(self.text.len());
        let end = end.max(start).min(self.text.len());
        CsonError::original(
            self.original,
            self.original_offsets[start],
            self.original_offsets[end],
            code,
            message,
            self.limits.max_diagnostics,
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    LineStart(usize),
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Colon,
    Comma,
    String(String),
    Number,
    Identifier,
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: TokenKind,
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndentUnit {
    Spaces(usize),
    Tabs(usize),
}

struct Lexer<'a> {
    source: &'a NormalizedSource<'a>,
    limits: CsonLimits,
    cursor: usize,
    at_line_start: bool,
    current_line_level: usize,
    indent_unit: Option<IndentUnit>,
    work: usize,
    tokens: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a NormalizedSource<'a>, limits: CsonLimits) -> Self {
        Self {
            source,
            limits,
            cursor: 0,
            at_line_start: true,
            current_line_level: 0,
            indent_unit: None,
            work: 0,
            tokens: Vec::new(),
        }
    }

    fn lex(mut self) -> Result<(Vec<Token>, usize), CsonError> {
        while self.cursor < self.bytes().len() {
            if self.at_line_start {
                let prefix_start = self.cursor;
                while matches!(self.peek(), Some(b' ' | b'\t')) {
                    self.bump()?;
                }
                if self.peek() == Some(b'\n') {
                    self.consume_newline()?;
                    continue;
                }
                if self.peek() == Some(b'#') {
                    self.skip_comment()?;
                    continue;
                }
                if self.peek().is_none() {
                    break;
                }
                let level = self.indent_level(prefix_start, self.cursor)?;
                self.current_line_level = level;
                self.tokens.push(Token {
                    kind: TokenKind::LineStart(level),
                    start: prefix_start,
                    end: self.cursor,
                });
                self.at_line_start = false;
            }

            match self.peek() {
                Some(b' ' | b'\t') => {
                    self.bump()?;
                }
                Some(b'\n') => self.consume_newline()?,
                Some(b'#') => self.skip_comment()?,
                Some(b'{') => self.punctuation(TokenKind::LeftBrace)?,
                Some(b'}') => self.punctuation(TokenKind::RightBrace)?,
                Some(b'[') => self.punctuation(TokenKind::LeftBracket)?,
                Some(b']') => self.punctuation(TokenKind::RightBracket)?,
                Some(b':') => self.punctuation(TokenKind::Colon)?,
                Some(b',') => self.punctuation(TokenKind::Comma)?,
                Some(b'\'') if self.remaining().starts_with("'''") => {
                    self.lex_multiline_string()?;
                }
                Some(quote @ (b'\'' | b'"')) => self.lex_string(quote)?,
                Some(b'-') if matches!(self.peek_at(1), Some(b'0'..=b'9')) => {
                    self.lex_number()?;
                }
                Some(b'0'..=b'9') => self.lex_number()?,
                Some(byte) if is_identifier_start(byte) => self.lex_identifier()?,
                Some(byte) if is_expression_byte(byte) => {
                    let start = self.cursor;
                    self.bump()?;
                    return Err(self.source.error(
                        start,
                        self.cursor,
                        CsonErrorCode::Expression,
                        "CSON v1 rejects executable and computed expressions",
                    ));
                }
                Some(_) => {
                    let start = self.cursor;
                    let character = self.remaining().chars().next().unwrap();
                    self.charge(character.len_utf8())?;
                    self.cursor += character.len_utf8();
                    return Err(self.source.error(
                        start,
                        self.cursor,
                        CsonErrorCode::Syntax,
                        "unexpected character in CSON input",
                    ));
                }
                None => break,
            }
        }
        Ok((self.tokens, self.work))
    }

    fn bytes(&self) -> &[u8] {
        self.source.text.as_bytes()
    }

    fn remaining(&self) -> &str {
        &self.source.text[self.cursor..]
    }

    fn peek(&self) -> Option<u8> {
        self.bytes().get(self.cursor).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes().get(self.cursor + offset).copied()
    }

    fn charge(&mut self, amount: usize) -> Result<(), CsonError> {
        let next = self.work.saturating_add(amount);
        if next > self.limits.max_work_units {
            return Err(self.source.error(
                self.cursor,
                self.cursor,
                CsonErrorCode::WorkLimit,
                format!(
                    "CSON parser work exceeds {} units",
                    self.limits.max_work_units
                ),
            ));
        }
        self.work = next;
        Ok(())
    }

    fn bump(&mut self) -> Result<u8, CsonError> {
        self.charge(1)?;
        let byte = self.bytes()[self.cursor];
        self.cursor += 1;
        Ok(byte)
    }

    fn consume_newline(&mut self) -> Result<(), CsonError> {
        self.bump()?;
        self.at_line_start = true;
        Ok(())
    }

    fn skip_comment(&mut self) -> Result<(), CsonError> {
        while !matches!(self.peek(), None | Some(b'\n')) {
            self.bump()?;
        }
        if self.peek() == Some(b'\n') {
            self.consume_newline()?;
        }
        Ok(())
    }

    fn indent_level(&mut self, start: usize, end: usize) -> Result<usize, CsonError> {
        let prefix = &self.source.text[start..end];
        if prefix.is_empty() {
            return Ok(0);
        }
        let spaces = prefix.bytes().all(|byte| byte == b' ');
        let tabs = prefix.bytes().all(|byte| byte == b'\t');
        if !spaces && !tabs {
            return Err(self.source.error(
                start,
                end,
                CsonErrorCode::Indentation,
                "CSON indentation cannot mix spaces and tabs",
            ));
        }
        let width = prefix.len();
        let unit = self.indent_unit.get_or_insert(if spaces {
            IndentUnit::Spaces(width)
        } else {
            IndentUnit::Tabs(width)
        });
        let unit_width = match (*unit, spaces) {
            (IndentUnit::Spaces(width), true) | (IndentUnit::Tabs(width), false) => width,
            _ => {
                return Err(self.source.error(
                    start,
                    end,
                    CsonErrorCode::Indentation,
                    "CSON indentation style changes within the document",
                ));
            }
        };
        if width % unit_width != 0 {
            return Err(self.source.error(
                start,
                end,
                CsonErrorCode::Indentation,
                "CSON indentation is not an integer number of levels",
            ));
        }
        Ok(width / unit_width)
    }

    fn punctuation(&mut self, kind: TokenKind) -> Result<(), CsonError> {
        let start = self.cursor;
        self.bump()?;
        self.tokens.push(Token {
            kind,
            start,
            end: self.cursor,
        });
        Ok(())
    }

    fn lex_identifier(&mut self) -> Result<(), CsonError> {
        let start = self.cursor;
        self.bump()?;
        while matches!(self.peek(), Some(byte) if is_identifier_continue(byte)) {
            self.bump()?;
        }
        self.tokens.push(Token {
            kind: TokenKind::Identifier,
            start,
            end: self.cursor,
        });
        Ok(())
    }

    fn lex_number(&mut self) -> Result<(), CsonError> {
        let start = self.cursor;
        if self.peek() == Some(b'-') {
            self.bump()?;
        }
        match self.peek() {
            Some(b'0') => {
                self.bump()?;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.source.error(
                        start,
                        self.cursor + 1,
                        CsonErrorCode::Number,
                        "CSON number cannot contain a leading zero",
                    ));
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.bump()?;
                }
            }
            _ => {
                return Err(self.source.error(
                    start,
                    self.cursor,
                    CsonErrorCode::Number,
                    "invalid CSON number",
                ));
            }
        }
        if self.peek() == Some(b'.') {
            self.bump()?;
            self.require_number_digits(start, "CSON fraction requires at least one digit")?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.bump()?;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.bump()?;
            }
            self.require_number_digits(start, "CSON exponent requires at least one digit")?;
        }
        self.tokens.push(Token {
            kind: TokenKind::Number,
            start,
            end: self.cursor,
        });
        Ok(())
    }

    fn require_number_digits(&mut self, start: usize, message: &str) -> Result<(), CsonError> {
        let digits = self.cursor;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.bump()?;
        }
        if self.cursor == digits {
            Err(self
                .source
                .error(start, self.cursor, CsonErrorCode::Number, message))
        } else {
            Ok(())
        }
    }

    fn lex_string(&mut self, quote: u8) -> Result<(), CsonError> {
        let start = self.cursor;
        self.bump()?;
        let mut output = String::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err(self.source.error(
                    start,
                    self.cursor,
                    CsonErrorCode::Syntax,
                    "unterminated CSON string",
                ));
            };
            if byte == quote {
                self.bump()?;
                self.tokens.push(Token {
                    kind: TokenKind::String(output),
                    start,
                    end: self.cursor,
                });
                return Ok(());
            }
            if byte == b'\n' {
                return Err(self.source.error(
                    start,
                    self.cursor,
                    CsonErrorCode::Syntax,
                    "physical newline in quoted CSON string",
                ));
            }
            if quote == b'"' && byte == b'#' && self.peek_at(1) == Some(b'{') {
                return Err(self.source.error(
                    self.cursor,
                    self.cursor + 2,
                    CsonErrorCode::Interpolation,
                    "CSON v1 rejects CoffeeScript interpolation",
                ));
            }
            let character = if byte == b'\\' {
                self.bump()?;
                self.lex_escape()?
            } else if byte.is_ascii() {
                self.bump()?;
                char::from(byte)
            } else {
                let character = self.remaining().chars().next().unwrap();
                self.charge(character.len_utf8())?;
                self.cursor += character.len_utf8();
                character
            };
            self.check_string_limit(start, self.cursor, output.len() + character.len_utf8())?;
            output.push(character);
        }
    }

    fn lex_escape(&mut self) -> Result<char, CsonError> {
        let escape_start = self.cursor.saturating_sub(1);
        let Some(escape) = self.peek() else {
            return Err(self.source.error(
                escape_start,
                self.cursor,
                CsonErrorCode::Syntax,
                "unterminated CSON escape",
            ));
        };
        self.bump()?;
        Ok(match escape {
            b'\'' => '\'',
            b'"' => '"',
            b'\\' => '\\',
            b'b' => '\u{0008}',
            b'f' => '\u{000c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => {
                let high = self.lex_hex_quad(escape_start)?;
                let scalar = if (0xd800..=0xdbff).contains(&high) {
                    if self.peek() != Some(b'\\') || self.peek_at(1) != Some(b'u') {
                        return Err(self.source.error(
                            escape_start,
                            self.cursor,
                            CsonErrorCode::Syntax,
                            "high surrogate must be followed by a low surrogate escape",
                        ));
                    }
                    self.bump()?;
                    self.bump()?;
                    let low = self.lex_hex_quad(escape_start)?;
                    if !(0xdc00..=0xdfff).contains(&low) {
                        return Err(self.source.error(
                            escape_start,
                            self.cursor,
                            CsonErrorCode::Syntax,
                            "invalid low surrogate in CSON escape",
                        ));
                    }
                    0x10000 + ((u32::from(high) - 0xd800) << 10) + (u32::from(low) - 0xdc00)
                } else if (0xdc00..=0xdfff).contains(&high) {
                    return Err(self.source.error(
                        escape_start,
                        self.cursor,
                        CsonErrorCode::Syntax,
                        "unexpected low surrogate in CSON escape",
                    ));
                } else {
                    u32::from(high)
                };
                char::from_u32(scalar).ok_or_else(|| {
                    self.source.error(
                        escape_start,
                        self.cursor,
                        CsonErrorCode::Syntax,
                        "invalid Unicode scalar in CSON escape",
                    )
                })?
            }
            _ => {
                return Err(self.source.error(
                    escape_start,
                    self.cursor,
                    CsonErrorCode::Syntax,
                    "invalid CSON escape",
                ));
            }
        })
    }

    fn lex_hex_quad(&mut self, start: usize) -> Result<u16, CsonError> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let Some(byte) = self.peek() else {
                return Err(self.source.error(
                    start,
                    self.cursor,
                    CsonErrorCode::Syntax,
                    "incomplete Unicode escape",
                ));
            };
            let digit = match byte {
                byte @ b'0'..=b'9' => u16::from(byte - b'0'),
                byte @ b'a'..=b'f' => u16::from(byte - b'a' + 10),
                byte @ b'A'..=b'F' => u16::from(byte - b'A' + 10),
                _ => {
                    return Err(self.source.error(
                        start,
                        self.cursor + 1,
                        CsonErrorCode::Syntax,
                        "invalid hexadecimal digit in Unicode escape",
                    ));
                }
            };
            self.bump()?;
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn lex_multiline_string(&mut self) -> Result<(), CsonError> {
        let start = self.cursor;
        let opening_level = self.current_line_level;
        let is_inline_value = !matches!(
            self.tokens.last().map(|token| &token.kind),
            Some(TokenKind::LineStart(_)) | None
        );
        let expected_closing_level = opening_level.saturating_add(usize::from(is_inline_value));
        self.bump()?;
        self.bump()?;
        self.bump()?;
        if self.peek() != Some(b'\n') {
            return Err(self.source.error(
                start,
                self.cursor,
                CsonErrorCode::Syntax,
                "triple-single CSON string must open before a newline",
            ));
        }
        self.consume_newline()?;
        let mut lines = Vec::<(usize, usize)>::new();
        loop {
            if self.cursor >= self.bytes().len() {
                return Err(self.source.error(
                    start,
                    self.cursor,
                    CsonErrorCode::Syntax,
                    "unterminated triple-single CSON string",
                ));
            }
            let line_start = self.cursor;
            let line_end = self.source.text[line_start..]
                .find('\n')
                .map_or(self.bytes().len(), |offset| line_start + offset);
            let line = &self.source.text[line_start..line_end];
            let indent_end = line
                .bytes()
                .take_while(|byte| matches!(byte, b' ' | b'\t'))
                .count();
            let after_indent = &line[indent_end..];
            let closing = after_indent.strip_prefix("'''").is_some_and(|suffix| {
                let suffix = suffix.trim_start_matches([' ', '\t']);
                suffix.is_empty() || suffix.starts_with('#')
            });
            if closing {
                let prefix = &line[..indent_end];
                let closing_level = self.indent_level(line_start, line_start + indent_end)?;
                if closing_level != expected_closing_level {
                    return Err(self.source.error(
                        line_start,
                        line_start + indent_end,
                        CsonErrorCode::Indentation,
                        "triple-single CSON closing delimiter is misindented",
                    ));
                }
                let mut output = String::new();
                for (index, (content_start, content_end)) in lines.iter().enumerate() {
                    let content = &self.source.text[*content_start..*content_end];
                    let stripped = if content.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
                        ""
                    } else {
                        content.strip_prefix(prefix).ok_or_else(|| {
                            self.source.error(
                                *content_start,
                                *content_start + content.len(),
                                CsonErrorCode::Indentation,
                                "multiline CSON content is less indented than its delimiter",
                            )
                        })?
                    };
                    let additional = stripped.len() + usize::from(index > 0);
                    self.check_string_limit(start, line_end, output.len() + additional)?;
                    if index > 0 {
                        output.push('\n');
                    }
                    output.push_str(stripped);
                }
                let consumed = indent_end + 3;
                self.charge(consumed)?;
                self.cursor += consumed;
                self.at_line_start = false;
                self.tokens.push(Token {
                    kind: TokenKind::String(output),
                    start,
                    end: self.cursor,
                });
                return Ok(());
            }
            self.charge(line.len())?;
            self.cursor = line_end;
            lines.push((line_start, line_end));
            if self.peek() == Some(b'\n') {
                self.consume_newline()?;
            }
        }
    }

    fn check_string_limit(&self, start: usize, end: usize, length: usize) -> Result<(), CsonError> {
        if length > self.limits.max_string_bytes {
            Err(self.source.error(
                start,
                end,
                CsonErrorCode::StringLimit,
                format!("CSON string exceeds {} bytes", self.limits.max_string_bytes),
            ))
        } else {
            Ok(())
        }
    }
}

fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$')
}

fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

fn is_expression_byte(byte: u8) -> bool {
    matches!(
        byte,
        b'(' | b')'
            | b'.'
            | b'/'
            | b'+'
            | b'-'
            | b'*'
            | b'%'
            | b'&'
            | b'|'
            | b'^'
            | b'~'
            | b'<'
            | b'>'
            | b'='
            | b'?'
            | b'@'
    )
}

struct Parser<'a> {
    source: &'a NormalizedSource<'a>,
    tokens: &'a [Token],
    limits: CsonLimits,
    index: usize,
    values: usize,
    work: usize,
}

impl<'a> Parser<'a> {
    fn new(
        source: &'a NormalizedSource<'a>,
        tokens: &'a [Token],
        limits: CsonLimits,
        work: usize,
    ) -> Self {
        Self {
            source,
            tokens,
            limits,
            index: 0,
            values: 0,
            work,
        }
    }

    fn parse_document(mut self) -> Result<Value, CsonError> {
        if self.tokens.is_empty() {
            return Err(self.error_at(
                self.source.text.len(),
                self.source.text.len(),
                CsonErrorCode::Syntax,
                "expected one CSON root value",
            ));
        }
        let (level, line) = self.take_line_start()?;
        if level != 0 {
            return Err(self.error_token(
                &line,
                CsonErrorCode::Indentation,
                "CSON root value must not be indented",
            ));
        }
        let value = self.parse_value_or_map(0, 0)?;
        if let Some(token) = self.current() {
            return Err(self.error_token(
                token,
                CsonErrorCode::Syntax,
                "unexpected data after the CSON root value",
            ));
        }
        Ok(value)
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn take(&mut self) -> Option<Token> {
        let token = self.current()?.clone();
        self.index += 1;
        Some(token)
    }

    fn charge(&mut self, amount: usize, start: usize) -> Result<(), CsonError> {
        let next = self.work.saturating_add(amount);
        if next > self.limits.max_work_units {
            return Err(self.error_at(
                start,
                start,
                CsonErrorCode::WorkLimit,
                format!(
                    "CSON parser work exceeds {} units",
                    self.limits.max_work_units
                ),
            ));
        }
        self.work = next;
        Ok(())
    }

    fn count_value(&mut self, token: &Token) -> Result<(), CsonError> {
        self.charge(1, token.start)?;
        if self.values >= self.limits.max_values {
            return Err(self.error_token(
                token,
                CsonErrorCode::ValueLimit,
                format!("CSON value count exceeds {}", self.limits.max_values),
            ));
        }
        self.values += 1;
        Ok(())
    }

    fn check_depth(&self, token: &Token, depth: usize) -> Result<(), CsonError> {
        let effective = self
            .limits
            .max_nesting_depth
            .min(IMPLEMENTATION_MAX_RECURSION_DEPTH);
        if depth >= effective {
            Err(self.error_token(
                token,
                CsonErrorCode::DepthLimit,
                format!("CSON nesting exceeds {effective}"),
            ))
        } else {
            Ok(())
        }
    }

    fn take_line_start(&mut self) -> Result<(usize, Token), CsonError> {
        let Some(token) = self.take() else {
            return Err(self.error_here(CsonErrorCode::Syntax, "expected a CSON line"));
        };
        let TokenKind::LineStart(level) = token.kind else {
            return Err(self.error_token(
                &token,
                CsonErrorCode::Syntax,
                "expected a CSON line boundary",
            ));
        };
        Ok((level, token))
    }

    fn peek_line_start(&self) -> Option<usize> {
        match &self.current()?.kind {
            TokenKind::LineStart(level) => Some(*level),
            _ => None,
        }
    }

    fn consume_line_start(&mut self) -> Option<usize> {
        let level = self.peek_line_start()?;
        self.index += 1;
        Some(level)
    }

    fn looks_like_map_entry(&self, index: usize) -> bool {
        matches!(
            self.tokens.get(index).map(|token| &token.kind),
            Some(TokenKind::Identifier | TokenKind::String(_))
        ) && matches!(
            self.tokens.get(index + 1).map(|token| &token.kind),
            Some(TokenKind::Colon)
        )
    }

    fn parse_value_or_map(&mut self, level: usize, depth: usize) -> Result<Value, CsonError> {
        if self.looks_like_map_entry(self.index) {
            self.parse_indented_map(level, depth)
        } else {
            self.parse_value(level, depth)
        }
    }

    fn parse_value(&mut self, level: usize, depth: usize) -> Result<Value, CsonError> {
        let Some(token) = self.current().cloned() else {
            return Err(self.error_here(CsonErrorCode::Syntax, "expected a CSON value"));
        };
        match &token.kind {
            TokenKind::LeftBracket => self.parse_array(level, depth),
            TokenKind::LeftBrace => self.parse_braced_map(level, depth),
            TokenKind::String(value) => {
                self.count_value(&token)?;
                self.index += 1;
                Ok(Value::from(value.clone()))
            }
            TokenKind::Number => {
                self.count_value(&token)?;
                self.index += 1;
                self.parse_number(&token)
            }
            TokenKind::Identifier => {
                self.count_value(&token)?;
                self.index += 1;
                let identifier = &self.source.text[token.start..token.end];
                match identifier {
                    "null" => Ok(Value::Nil),
                    "true" => Ok(Value::Bool(true)),
                    "false" => Ok(Value::Bool(false)),
                    _ => Err(self.error_token(
                        &token,
                        CsonErrorCode::IdentifierValue,
                        format!("CSON v1 rejects bare identifier value {identifier:?}"),
                    )),
                }
            }
            _ => Err(self.error_token(&token, CsonErrorCode::Syntax, "expected a CSON value")),
        }
    }

    fn parse_number(&self, token: &Token) -> Result<Value, CsonError> {
        let source = &self.source.text[token.start..token.end];
        let limits = self.limits.numeric_resource_limits();
        if source.contains(['.', 'e', 'E']) {
            decimal_text_resource_preflight(source, limits).map_err(|_| {
                self.error_token(
                    token,
                    CsonErrorCode::Number,
                    "CSON Decimal exceeds its configured numeric boundary",
                )
            })?;
            let value = Decimal::parse(source).ok_or_else(|| {
                self.error_token(
                    token,
                    CsonErrorCode::Number,
                    "CSON Decimal exceeds the implementation numeric boundary",
                )
            })?;
            if value.scale() > self.limits.max_decimal_scale
                || value.inner().bits() > self.limits.max_decimal_coefficient_bits
            {
                return Err(self.error_token(
                    token,
                    CsonErrorCode::Number,
                    "CSON Decimal exceeds its configured numeric boundary",
                ));
            }
            Ok(Value::Decimal(Rc::new(value)))
        } else {
            let (negative, digits) = source
                .strip_prefix('-')
                .map_or((false, source), |digits| (true, digits));
            integer_digits_resource_preflight(digits, limits).map_err(|_| {
                self.error_token(
                    token,
                    CsonErrorCode::Number,
                    "CSON Integer exceeds its configured numeric boundary",
                )
            })?;
            let mut value = BigInt::parse_bytes(digits.as_bytes(), 10).ok_or_else(|| {
                self.error_token(token, CsonErrorCode::Number, "invalid CSON Integer")
            })?;
            if negative {
                value = -value;
            }
            if value.bits() > self.limits.max_integer_bits {
                return Err(self.error_token(
                    token,
                    CsonErrorCode::Number,
                    "CSON Integer exceeds its configured numeric boundary",
                ));
            }
            Ok(Value::Integer(Rc::new(
                Integer::from_bigint(value).map_err(|_| {
                    self.error_token(
                        token,
                        CsonErrorCode::Number,
                        "CSON Integer exceeds the implementation numeric boundary",
                    )
                })?,
            )))
        }
    }

    fn parse_indented_map(&mut self, level: usize, depth: usize) -> Result<Value, CsonError> {
        let first = self
            .current()
            .cloned()
            .ok_or_else(|| self.error_here(CsonErrorCode::Syntax, "expected a CSON Map entry"))?;
        self.check_depth(&first, depth)?;
        self.count_value(&first)?;
        let mut values = BTreeMap::new();
        loop {
            let (key, key_token) = self.parse_key()?;
            self.expect_colon()?;
            let value = self.parse_after_colon(level, depth + 1)?;
            self.insert_map_value(&mut values, key, value, &key_token)?;

            let Some(next_level) = self.peek_line_start() else {
                break;
            };
            if next_level < level {
                break;
            }
            if next_level > level {
                let token = self.current().unwrap();
                return Err(self.error_token(
                    token,
                    CsonErrorCode::Indentation,
                    "unexpected indentation after a complete CSON value",
                ));
            }
            if !self.looks_like_map_entry(self.index + 1) {
                break;
            }
            self.index += 1;
        }
        Ok(Value::Map(Rc::new(values)))
    }

    fn parse_braced_map(&mut self, level: usize, depth: usize) -> Result<Value, CsonError> {
        let open = self.take().unwrap();
        self.check_depth(&open, depth)?;
        self.count_value(&open)?;
        let mut entry_level = level;
        if let Some(next_level) = self.consume_line_start() {
            if matches!(
                self.current().map(|token| &token.kind),
                Some(TokenKind::RightBrace)
            ) {
                self.check_closing_level(next_level, level, "Map")?;
            } else {
                self.check_item_level(next_level, level, "Map")?;
                entry_level = next_level;
            }
        }
        let mut values = BTreeMap::new();
        if self.consume_kind(|kind| matches!(kind, TokenKind::RightBrace)) {
            return Ok(Value::Map(Rc::new(values)));
        }
        loop {
            let (key, key_token) = self.parse_key()?;
            self.expect_colon()?;
            let value = self.parse_after_colon(entry_level, depth + 1)?;
            self.insert_map_value(&mut values, key, value, &key_token)?;

            if self.consume_kind(|kind| matches!(kind, TokenKind::Comma)) {
                if let Some(next_level) = self.consume_line_start() {
                    if matches!(
                        self.current().map(|token| &token.kind),
                        Some(TokenKind::RightBrace)
                    ) {
                        self.check_closing_level(next_level, level, "Map")?;
                    } else {
                        self.check_item_level(next_level, level, "Map")?;
                        entry_level = next_level;
                    }
                }
                if self.consume_kind(|kind| matches!(kind, TokenKind::RightBrace)) {
                    break;
                }
            } else if let Some(next_level) = self.consume_line_start() {
                if matches!(
                    self.current().map(|token| &token.kind),
                    Some(TokenKind::RightBrace)
                ) {
                    self.check_closing_level(next_level, level, "Map")?;
                } else {
                    self.check_item_level(next_level, level, "Map")?;
                    entry_level = next_level;
                }
                if self.consume_kind(|kind| matches!(kind, TokenKind::RightBrace)) {
                    break;
                }
            } else if self.consume_kind(|kind| matches!(kind, TokenKind::RightBrace)) {
                break;
            } else {
                return Err(self.error_here(
                    CsonErrorCode::Syntax,
                    "expected comma, newline, or '}' after CSON Map entry",
                ));
            }
            if !self.looks_like_map_entry(self.index) {
                return Err(self.error_here(CsonErrorCode::Syntax, "expected a CSON Map entry"));
            }
        }
        Ok(Value::Map(Rc::new(values)))
    }

    fn parse_array(&mut self, level: usize, depth: usize) -> Result<Value, CsonError> {
        let open = self.take().unwrap();
        self.check_depth(&open, depth)?;
        self.count_value(&open)?;
        let mut item_level = level;
        if let Some(next_level) = self.consume_line_start() {
            if matches!(
                self.current().map(|token| &token.kind),
                Some(TokenKind::RightBracket)
            ) {
                self.check_closing_level(next_level, level, "Array")?;
            } else {
                self.check_item_level(next_level, level, "Array")?;
                item_level = next_level;
            }
        }
        let mut values = Vec::new();
        if self.consume_kind(|kind| matches!(kind, TokenKind::RightBracket)) {
            return Ok(Value::Array(Rc::new(values)));
        }
        loop {
            if values.len() >= self.limits.max_container_items {
                return Err(self.error_here(
                    CsonErrorCode::ContainerLimit,
                    format!(
                        "CSON Array exceeds {} items",
                        self.limits.max_container_items
                    ),
                ));
            }
            let item_start = self.current().map_or(open.start, |token| token.start);
            let value = self.parse_value_or_map(item_level, depth + 1)?;
            self.charge(1, item_start)?;
            values.push(value);
            if self.consume_kind(|kind| matches!(kind, TokenKind::Comma)) {
                if let Some(next_level) = self.consume_line_start() {
                    if matches!(
                        self.current().map(|token| &token.kind),
                        Some(TokenKind::RightBracket)
                    ) {
                        self.check_closing_level(next_level, level, "Array")?;
                    } else {
                        self.check_item_level(next_level, level, "Array")?;
                        item_level = next_level;
                    }
                }
                if self.consume_kind(|kind| matches!(kind, TokenKind::RightBracket)) {
                    break;
                }
            } else if let Some(next_level) = self.consume_line_start() {
                if matches!(
                    self.current().map(|token| &token.kind),
                    Some(TokenKind::RightBracket)
                ) {
                    self.check_closing_level(next_level, level, "Array")?;
                } else {
                    self.check_item_level(next_level, level, "Array")?;
                    item_level = next_level;
                }
                if self.consume_kind(|kind| matches!(kind, TokenKind::RightBracket)) {
                    break;
                }
            } else if self.consume_kind(|kind| matches!(kind, TokenKind::RightBracket)) {
                break;
            } else {
                return Err(self.error_here(
                    CsonErrorCode::Syntax,
                    "expected comma, newline, or ']' after CSON Array item",
                ));
            }
        }
        Ok(Value::Array(Rc::new(values)))
    }

    fn check_item_level(
        &self,
        actual: usize,
        parent: usize,
        container: &str,
    ) -> Result<(), CsonError> {
        let expected = parent.saturating_add(1);
        if actual == expected {
            Ok(())
        } else {
            Err(self.error_here(
                CsonErrorCode::Indentation,
                format!("multiline CSON {container} item must be indented one level"),
            ))
        }
    }

    fn check_closing_level(
        &self,
        actual: usize,
        parent: usize,
        container: &str,
    ) -> Result<(), CsonError> {
        if actual == parent {
            Ok(())
        } else {
            Err(self.error_here(
                CsonErrorCode::Indentation,
                format!("multiline CSON {container} closing delimiter is misindented"),
            ))
        }
    }

    fn parse_key(&mut self) -> Result<(String, Token), CsonError> {
        let Some(token) = self.take() else {
            return Err(self.error_here(CsonErrorCode::Syntax, "expected a CSON Map key"));
        };
        let key = match &token.kind {
            TokenKind::Identifier => {
                let key = &self.source.text[token.start..token.end];
                if key.len() > self.limits.max_string_bytes {
                    return Err(self.error_token(
                        &token,
                        CsonErrorCode::StringLimit,
                        format!(
                            "CSON Map key exceeds {} bytes",
                            self.limits.max_string_bytes
                        ),
                    ));
                }
                key.to_owned()
            }
            TokenKind::String(key) => key.clone(),
            _ => {
                return Err(self.error_token(
                    &token,
                    CsonErrorCode::Syntax,
                    "CSON Map key must be an ASCII identifier or quoted String",
                ));
            }
        };
        Ok((key, token))
    }

    fn expect_colon(&mut self) -> Result<(), CsonError> {
        if self.consume_kind(|kind| matches!(kind, TokenKind::Colon)) {
            Ok(())
        } else {
            Err(self.error_here(CsonErrorCode::Syntax, "expected ':' after CSON Map key"))
        }
    }

    fn parse_after_colon(&mut self, level: usize, depth: usize) -> Result<Value, CsonError> {
        if let Some(child_level) = self.consume_line_start() {
            if child_level != level + 1 {
                return Err(self.error_here(
                    CsonErrorCode::Indentation,
                    "nested CSON value must add exactly one indentation level",
                ));
            }
            self.parse_value_or_map(child_level, depth)
        } else {
            self.parse_value(level, depth)
        }
    }

    fn insert_map_value(
        &mut self,
        values: &mut BTreeMap<String, Value>,
        key: String,
        value: Value,
        key_token: &Token,
    ) -> Result<(), CsonError> {
        if values.contains_key(&key) {
            return Err(self.error_token(
                key_token,
                CsonErrorCode::DuplicateKey,
                format!("duplicate CSON Map key {key:?}"),
            ));
        }
        if values.len() >= self.limits.max_container_items {
            return Err(self.error_token(
                key_token,
                CsonErrorCode::ContainerLimit,
                format!(
                    "CSON Map exceeds {} entries",
                    self.limits.max_container_items
                ),
            ));
        }
        self.charge(1, key_token.start)?;
        values.insert(key, value);
        Ok(())
    }

    fn consume_kind(&mut self, predicate: impl FnOnce(&TokenKind) -> bool) -> bool {
        if self.current().is_some_and(|token| predicate(&token.kind)) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn error_here(&self, code: CsonErrorCode, message: impl Into<String>) -> CsonError {
        let message = message.into();
        if let Some(token) = self.current() {
            self.error_token(token, code, message)
        } else {
            self.error_at(
                self.source.text.len(),
                self.source.text.len(),
                code,
                message,
            )
        }
    }

    fn error_token(
        &self,
        token: &Token,
        code: CsonErrorCode,
        message: impl Into<String>,
    ) -> CsonError {
        self.error_at(token.start, token.end, code, message)
    }

    fn error_at(
        &self,
        start: usize,
        end: usize,
        code: CsonErrorCode,
        message: impl Into<String>,
    ) -> CsonError {
        self.source.error(start, end, code, message)
    }
}
