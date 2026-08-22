use crate::vm::Error;
use std::{iter::Peekable, str::Chars};
use unicode_ident::{is_xid_continue, is_xid_start};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Token {
    Number(f64),
    String(String, bool),
    Ident(String),
    True,
    False,
    Nil,
    If,
    Unless,
    Then,
    Else,
    While,
    Until,
    Loop,
    For,
    By,
    In,
    Break,
    Continue,
    Return,
    Class,
    Switch,
    When,
    Own,
    Of,
    Try,
    Catch,
    Finally,
    Throw,
    Do,
    And,
    Or,
    Not,
    Plus,
    PlusPlus,
    Minus,
    MinusMinus,
    Star,
    Slash,
    FloorDiv,
    Percent,
    Modulo,
    Amp,
    Pipe,
    Caret,
    Tilde,
    ShiftLeft,
    ShiftRight,
    ShiftRightUnsigned,
    Power,
    EqEq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    FloorDivAssign,
    PercentAssign,
    ModuloAssign,
    AmpAssign,
    PipeAssign,
    CaretAssign,
    ShiftLeftAssign,
    ShiftRightAssign,
    ShiftRightUnsignedAssign,
    PowerAssign,
    Arrow,
    FatArrow,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Dot,
    Question,
    RangeInclusive,
    Ellipsis,
    Semi,
    Indent,
    Dedent,
    Eof,
}

struct LexOutput {
    tokens: Vec<Token>,
    lines: Vec<usize>,
    current_line: usize,
}
impl LexOutput {
    fn new() -> Self {
        Self {
            tokens: vec![],
            lines: vec![],
            current_line: 1,
        }
    }
    fn set_line(&mut self, line: usize) {
        self.current_line = line;
    }
    fn push(&mut self, token: Token) {
        self.tokens.push(token);
        self.lines.push(self.current_line);
    }
    fn last(&self) -> Option<&Token> {
        self.tokens.last()
    }
    fn finish(self) -> (Vec<Token>, Vec<usize>) {
        (self.tokens, self.lines)
    }
}

#[cfg(test)]
pub(crate) fn lex(source: &str) -> Result<Vec<Token>, Error> {
    Ok(lex_spanned(source)?.0)
}

pub(crate) fn lex_spanned(source: &str) -> Result<(Vec<Token>, Vec<usize>), Error> {
    let normalized = normalize_indented_maps(&normalize_heredocs(source)?);
    let logical_lines = normalize_multiline_strings(&normalized);
    let mut out = LexOutput::new();
    let mut indents = vec![0usize];
    let mut groups = Vec::new();
    let mut continued = false;
    let mut block_comment_start = None;
    for (line_number, raw_line) in logical_lines {
        out.set_line(line_number);
        let line = raw_line.strip_suffix('\r').unwrap_or(&raw_line);
        let prefix = line.len() - line.trim_start_matches([' ', '\t']).len();
        let leading = &line[..prefix];
        if leading.contains('\t') {
            return Err(Error::parse("tabs are not permitted for indentation").at_line(line_number));
        }
        let content = &line[prefix..];
        if block_comment_start.is_some() {
            if content.contains("###") {
                block_comment_start = None;
            }
            continue;
        }
        if let Some(rest) = content.strip_prefix("###") {
            if !rest.contains("###") {
                block_comment_start = Some(line_number);
            }
            continue;
        }
        if content.is_empty() || content.starts_with('#') {
            continue;
        }
        if groups.is_empty() && !continued {
            let current = *indents.last().expect("indent stack");
            if prefix > current {
                indents.push(prefix);
                out.push(Token::Indent);
            } else if prefix < current {
                while prefix < *indents.last().expect("indent stack") {
                    indents.pop();
                    out.push(Token::Dedent);
                }
                if prefix != *indents.last().expect("indent stack") {
                    return Err(Error::parse("inconsistent indentation").at_line(line_number));
                }
                out.push(Token::Semi);
            }
        }
        lex_line(content, line_number, &mut groups, &mut out)?;
        continued = groups.is_empty() && line_continues(out.last());
        let needs_separator =
            (groups.is_empty() && !continued && !matches!(out.last(), Some(Token::Semi)))
                || (matches!(groups.last(), Some('[' | '{'))
                    && !continued
                    && collection_item_can_end(out.last()));
        if needs_separator {
            out.push(Token::Semi);
        }
    }
    if !groups.is_empty() {
        return Err(Error::parse("unterminated grouping delimiter")
            .at_line(normalized.lines().count().max(1)));
    }
    if let Some(line) = block_comment_start {
        return Err(Error::parse("unterminated block comment").at_line(line));
    }
    while indents.len() > 1 {
        indents.pop();
        out.push(Token::Dedent);
    }
    if !matches!(out.last(), Some(Token::Semi) | Some(Token::Dedent) | None) {
        out.push(Token::Semi);
    }
    out.push(Token::Eof);
    Ok(out.finish())
}

/// Joins ordinary quoted strings that span physical lines. Heredocs are already
/// lowered before this pass, so only an unmatched single/double quote is handled.
/// A normal newline contributes one space; a single unescaped trailing backslash
/// suppresses that separator. The returned line number is the opening line.
fn normalize_multiline_strings(source: &str) -> Vec<(usize, String)> {
    let physical: Vec<&str> = source.split('\n').collect();
    let mut logical = Vec::with_capacity(physical.len());
    let mut index = 0usize;
    let mut block_comment = false;
    while index < physical.len() {
        let line_number = index + 1;
        let mut line = physical[index].to_owned();
        let content = line.trim_start();
        if block_comment || content.starts_with('#') {
            if block_comment && content.contains("###") {
                block_comment = false;
            } else if !block_comment && content.starts_with("###") && !content[3..].contains("###")
            {
                block_comment = true;
            }
            logical.push((line_number, line));
            index += 1;
            continue;
        }
        while has_unclosed_string(&line) && index + 1 < physical.len() {
            let next = physical[index + 1];
            let continued_without_space = trim_single_trailing_backslash(&mut line);
            if !continued_without_space {
                line.push(' ');
            }
            line.push_str(next.trim());
            index += 1;
        }
        logical.push((line_number, line));
        index += 1;
    }
    logical
}

fn trim_single_trailing_backslash(line: &mut String) -> bool {
    let trimmed_len = line.trim_end().len();
    let bytes = line.as_bytes();
    if trimmed_len == 0 || bytes[trimmed_len - 1] != b'\\' {
        line.truncate(trimmed_len);
        return false;
    }
    let mut backslashes = 0usize;
    for byte in bytes[..trimmed_len].iter().rev() {
        if *byte != b'\\' {
            break;
        }
        backslashes += 1;
    }
    if backslashes % 2 == 0 {
        line.truncate(trimmed_len);
        false
    } else {
        line.truncate(trimmed_len - 1);
        true
    }
}

fn has_unclosed_string(line: &str) -> bool {
    let mut quote = None;
    let mut escaped = false;
    for ch in line.chars() {
        if let Some(current) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == current {
                quote = None;
            }
        } else {
            match ch {
                '\'' | '"' => quote = Some(ch),
                '#' => break,
                _ => {}
            }
        }
    }
    quote.is_some()
}

/// Lowers CoffeeScript-style triple-quoted strings to ordinary single-line
/// lexer input. Double triples retain interpolation; single triples do not.
fn normalize_heredocs(source: &str) -> Result<String, Error> {
    let mut chars = source.chars().peekable();
    let mut out = String::with_capacity(source.len());
    let mut line_comment = false;
    let mut block_comment = false;
    let mut string_quote = None;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if line_comment {
            out.push(ch);
            if ch == '\n' {
                line_comment = false;
            }
            continue;
        }
        if block_comment {
            out.push(ch);
            if ch == '#' && chars.peek() == Some(&'#') {
                chars.next();
                out.push('#');
                if chars.peek() == Some(&'#') {
                    chars.next();
                    out.push('#');
                    block_comment = false;
                }
            }
            continue;
        }
        if let Some(quote) = string_quote {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                string_quote = None;
            }
            continue;
        }
        if ch == '#' {
            out.push(ch);
            if chars.peek() == Some(&'#') {
                chars.next();
                out.push('#');
                if chars.peek() == Some(&'#') {
                    chars.next();
                    out.push('#');
                    block_comment = true;
                } else {
                    line_comment = true;
                }
            } else {
                line_comment = true;
            }
            continue;
        }
        if (ch == '\'' || ch == '"') && chars.peek() == Some(&ch) {
            chars.next();
            if chars.peek() == Some(&ch) {
                chars.next();
                let quote = ch;
                let mut value = String::new();
                let mut closed = false;
                while let Some(current) = chars.next() {
                    if current == quote && chars.peek() == Some(&quote) {
                        chars.next();
                        if chars.peek() == Some(&quote) {
                            chars.next();
                            closed = true;
                            break;
                        }
                        value.push(current);
                        value.push(quote);
                        continue;
                    }
                    value.push(current);
                }
                if !closed {
                    return Err(Error::parse("unterminated heredoc"));
                }
                out.push(quote);
                for current in value.chars() {
                    match current {
                        '\n' => out.push_str("\\n"),
                        '\r' => out.push_str("\\r"),
                        '\t' => out.push_str("\\t"),
                        '\\' => out.push_str("\\\\"),
                        x if x == quote => {
                            out.push('\\');
                            out.push(x);
                        }
                        x => out.push(x),
                    }
                }
                out.push(quote);
                continue;
            }
            out.push(ch);
            out.push(ch);
            continue;
        }
        out.push(ch);
        if ch == '\'' || ch == '"' {
            string_quote = Some(ch);
            escaped = false;
        }
    }
    Ok(out)
}

/// Lowers the unbraced object form used by CoffeeScript into explicit map
/// delimiters without adding physical lines. A block is recognized only when
/// an indented child begins with an identifier/string key and follows a parent
/// assignment (`record =`) or map entry (`nested:`).
fn normalize_indented_maps(source: &str) -> String {
    let mut output: Vec<String> = Vec::new();
    let mut active: Vec<usize> = Vec::new();
    for raw in source.split('\n') {
        let indent = raw.len() - raw.trim_start_matches(' ').len();
        let content = raw[indent..].trim_end();
        if !content.is_empty() && !content.starts_with('#') {
            while active.last().is_some_and(|child| indent < *child) {
                append_map_closing(&mut output);
                active.pop();
            }
            if let Some(previous) = previous_significant(&mut output) {
                let previous_indent = previous.len() - previous.trim_start_matches(' ').len();
                let previous_content = previous[previous_indent..].trim_end();
                if indent > previous_indent
                    && map_parent_line(previous_content)
                    && map_entry_line(content)
                {
                    append_before_comment(previous, " {");
                    active.push(indent);
                }
            }
        }
        output.push(raw.to_owned());
    }
    while !active.is_empty() {
        append_map_closing(&mut output);
        active.pop();
    }
    output.join("\n")
}

fn previous_significant(lines: &mut [String]) -> Option<&mut String> {
    lines
        .iter_mut()
        .rev()
        .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
}

fn append_before_comment(line: &mut String, text: &str) {
    if let Some(position) = line.find('#') {
        line.insert_str(position, text);
    } else {
        line.push_str(text);
    }
}

fn append_map_closing(lines: &mut [String]) {
    if let Some(line) = previous_significant(lines) {
        append_before_comment(line, " }");
    }
}

fn map_parent_line(content: &str) -> bool {
    let content = content.split('#').next().unwrap_or(content).trim_end();
    if content.ends_with(':') {
        return true;
    }
    if !content.ends_with('=') {
        return false;
    }
    let before = content[..content.len() - 1].trim_end().chars().last();
    !matches!(
        before,
        Some('=' | '>' | '+' | '-' | '*' | '/' | '%' | '&' | '|' | '^')
    )
}

fn map_entry_line(content: &str) -> bool {
    let Some((key, _)) = content.split_once(':') else {
        return false;
    };
    let key = key.trim();
    if key.is_empty() {
        return false;
    }
    if (key.starts_with('\'') && key.ends_with('\''))
        || (key.starts_with('"') && key.ends_with('"'))
    {
        return key.len() >= 2;
    }
    let mut chars = key.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (is_xid_start(first) || first == '_')
        && chars.all(|character| is_xid_continue(character) || character == '_')
}

fn lex_line(
    line: &str,
    line_number: usize,
    groups: &mut Vec<char>,
    out: &mut LexOutput,
) -> Result<(), Error> {
    out.set_line(line_number);
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\t' => {}
            '#' => break,
            ';' => {
                if !matches!(out.last(), Some(Token::Semi)) {
                    out.push(Token::Semi)
                }
            }
            '0'..='9' => {
                if ch == '0' {
                    let radix = match chars.peek().copied() {
                        Some('x' | 'X') => Some(16),
                        Some('b' | 'B') => Some(2),
                        Some('o' | 'O') => Some(8),
                        _ => None,
                    };
                    if let Some(radix) = radix {
                        chars.next();
                        let mut digits = String::new();
                        while chars.peek().is_some_and(|digit| digit.is_ascii_hexdigit()) {
                            digits.push(chars.next().expect("peeked"));
                        }
                        if digits.is_empty()
                            || digits.chars().any(|digit| digit.to_digit(radix).is_none())
                        {
                            return Err(Error::parse(format!("invalid base-{radix} number"))
                                .at_line(line_number));
                        }
                        let value = u64::from_str_radix(&digits, radix).map_err(|_| {
                            Error::parse(format!("invalid base-{radix} number"))
                                .at_line(line_number)
                        })?;
                        out.push(Token::Number(value as f64));
                        continue;
                    }
                }
                let mut source = ch.to_string();
                let mut saw_dot = false;
                while let Some(next) = chars.peek() {
                    if next.is_ascii_digit() {
                        source.push(chars.next().expect("peeked"));
                    } else if *next == '.' && !saw_dot && chars.clone().nth(1) != Some('.') {
                        saw_dot = true;
                        source.push(chars.next().expect("peeked"));
                    } else {
                        break;
                    }
                }
                if matches!(chars.peek(), Some('e' | 'E')) {
                    source.push(chars.next().expect("peeked"));
                    if matches!(chars.peek(), Some('+' | '-')) {
                        source.push(chars.next().expect("peeked"));
                    }
                    let exponent_start = source.len();
                    while chars.peek().is_some_and(|digit| digit.is_ascii_digit()) {
                        source.push(chars.next().expect("peeked"));
                    }
                    if source.len() == exponent_start {
                        return Err(Error::parse("invalid exponent").at_line(line_number));
                    }
                }
                out.push(Token::Number(source.parse().map_err(|_| {
                    Error::parse("invalid number").at_line(line_number)
                })?));
            }
            '\'' | '"' => {
                let quote = ch;
                let mut value = String::new();
                let mut closed = false;
                while let Some(current) = chars.next() {
                    if current == quote {
                        closed = true;
                        break;
                    }
                    if current == '\\' {
                        let escaped = chars.next().ok_or_else(|| {
                            Error::parse("unterminated string").at_line(line_number)
                        })?;
                        value.push(decode_string_escape(&mut chars, escaped, line_number)?);
                    } else {
                        value.push(current);
                    }
                }
                if !closed {
                    return Err(Error::parse("unterminated string").at_line(line_number));
                }
                out.push(Token::String(value, quote == '"'));
            }
            c if is_xid_start(c) || c == '_' => {
                let mut name = c.to_string();
                while matches!(chars.peek(), Some(next) if is_xid_continue(*next) || *next == '_') {
                    name.push(chars.next().expect("peeked"));
                }
                out.push(match name.as_str() {
                    "true" | "yes" | "on" => Token::True,
                    "false" | "no" | "off" => Token::False,
                    "nil" => Token::Nil,
                    "if" => Token::If,
                    "unless" => Token::Unless,
                    "then" => Token::Then,
                    "else" => Token::Else,
                    "while" => Token::While,
                    "until" => Token::Until,
                    "loop" => Token::Loop,
                    "for" => Token::For,
                    "by" => Token::By,
                    "in" => Token::In,
                    "break" => Token::Break,
                    "continue" => Token::Continue,
                    "return" => Token::Return,
                    "class" => Token::Class,
                    "switch" => Token::Switch,
                    "when" => Token::When,
                    "own" => Token::Own,
                    "of" => Token::Of,
                    "try" => Token::Try,
                    "catch" => Token::Catch,
                    "finally" => Token::Finally,
                    "throw" => Token::Throw,
                    "do" => Token::Do,
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    "is" => Token::EqEq,
                    "isnt" => Token::NotEq,
                    _ => Token::Ident(name),
                });
            }
            '+' => {
                if chars.peek() == Some(&'+') {
                    chars.next();
                    out.push(Token::PlusPlus)
                } else if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::PlusAssign)
                } else {
                    out.push(Token::Plus)
                }
            }
            '-' => {
                if chars.peek() == Some(&'>') {
                    chars.next();
                    out.push(Token::Arrow)
                } else if chars.peek() == Some(&'-') {
                    chars.next();
                    out.push(Token::MinusMinus)
                } else if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::MinusAssign)
                } else {
                    out.push(Token::Minus)
                }
            }
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        out.push(Token::PowerAssign)
                    } else {
                        out.push(Token::Power)
                    }
                } else if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::StarAssign)
                } else {
                    out.push(Token::Star)
                }
            }
            '/' => {
                if chars.peek() == Some(&'/') {
                    chars.next();
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        out.push(Token::FloorDivAssign)
                    } else {
                        out.push(Token::FloorDiv)
                    }
                } else if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::SlashAssign)
                } else {
                    out.push(Token::Slash)
                }
            }
            '%' => {
                if chars.peek() == Some(&'%') {
                    chars.next();
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        out.push(Token::ModuloAssign)
                    } else {
                        out.push(Token::Modulo)
                    }
                } else if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::PercentAssign)
                } else {
                    out.push(Token::Percent)
                }
            }
            '&' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::AmpAssign)
                } else {
                    out.push(Token::Amp)
                }
            }
            '|' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::PipeAssign)
                } else {
                    out.push(Token::Pipe)
                }
            }
            '^' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::CaretAssign)
                } else {
                    out.push(Token::Caret)
                }
            }
            '~' => out.push(Token::Tilde),
            '(' => {
                groups.push('(');
                out.push(Token::LParen)
            }
            ')' => {
                close_group(groups, ')', line_number)?;
                out.push(Token::RParen)
            }
            '[' => {
                groups.push('[');
                out.push(Token::LBracket)
            }
            ']' => {
                close_group(groups, ']', line_number)?;
                out.push(Token::RBracket)
            }
            '{' => {
                groups.push('{');
                out.push(Token::LBrace)
            }
            '}' => {
                close_group(groups, '}', line_number)?;
                out.push(Token::RBrace)
            }
            ',' => out.push(Token::Comma),
            ':' => out.push(Token::Colon),
            '?' => out.push(Token::Question),
            '.' => {
                if chars.peek() == Some(&'.') {
                    chars.next();
                    if chars.peek() == Some(&'.') {
                        chars.next();
                        out.push(Token::Ellipsis);
                    } else {
                        out.push(Token::RangeInclusive);
                    }
                } else {
                    out.push(Token::Dot)
                }
            }
            '=' => {
                if chars.peek() == Some(&'>') {
                    chars.next();
                    out.push(Token::FatArrow)
                } else if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::EqEq)
                } else {
                    out.push(Token::Assign)
                }
            }
            '!' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::NotEq)
                } else {
                    out.push(Token::Not)
                }
            }
            '<' => {
                if chars.peek() == Some(&'<') {
                    chars.next();
                    if chars.peek() == Some(&'=') {
                        chars.next();
                        out.push(Token::ShiftLeftAssign)
                    } else {
                        out.push(Token::ShiftLeft)
                    }
                } else if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::LtEq)
                } else {
                    out.push(Token::Lt)
                }
            }
            '>' => {
                if chars.peek() == Some(&'>') {
                    chars.next();
                    if chars.peek() == Some(&'>') {
                        chars.next();
                        if chars.peek() == Some(&'=') {
                            chars.next();
                            out.push(Token::ShiftRightUnsignedAssign)
                        } else {
                            out.push(Token::ShiftRightUnsigned)
                        }
                    } else if chars.peek() == Some(&'=') {
                        chars.next();
                        out.push(Token::ShiftRightAssign)
                    } else {
                        out.push(Token::ShiftRight)
                    }
                } else if chars.peek() == Some(&'=') {
                    chars.next();
                    out.push(Token::GtEq)
                } else {
                    out.push(Token::Gt)
                }
            }
            _ => {
                return Err(
                    Error::parse(format!("unexpected character '{ch}'")).at_line(line_number)
                );
            }
        }
    }
    Ok(())
}

fn decode_string_escape(
    chars: &mut Peekable<Chars<'_>>,
    escaped: char,
    line_number: usize,
) -> Result<char, Error> {
    let simple = match escaped {
        '0' => Some('\0'),
        'b' => Some('\u{0008}'),
        'f' => Some('\u{000c}'),
        'n' => Some('\n'),
        'r' => Some('\r'),
        't' => Some('\t'),
        'v' => Some('\u{000b}'),
        '\\' => Some('\\'),
        '\'' => Some('\''),
        '"' => Some('"'),
        _ => None,
    };
    if let Some(character) = simple {
        return Ok(character);
    }
    let (digits, radix, kind) = match escaped {
        'x' => (2, 16, "hexadecimal"),
        'u' => {
            if chars.peek() == Some(&'{') {
                chars.next();
                let mut digits = String::new();
                let mut closed = false;
                while let Some(&next) = chars.peek() {
                    if next == '}' {
                        chars.next();
                        closed = true;
                        break;
                    }
                    if !next.is_ascii_hexdigit() || digits.len() >= 6 {
                        return Err(Error::parse("invalid Unicode escape").at_line(line_number));
                    }
                    digits.push(next);
                    chars.next();
                }
                if digits.is_empty() || !closed {
                    return Err(Error::parse("invalid Unicode escape").at_line(line_number));
                }
                let value = u32::from_str_radix(&digits, 16)
                    .map_err(|_| Error::parse("invalid Unicode escape").at_line(line_number))?;
                return char::from_u32(value).ok_or_else(|| {
                    Error::parse("Unicode escape is not a valid scalar value").at_line(line_number)
                });
            }
            (4, 16, "Unicode")
        }
        _ => {
            return Err(Error::parse("unknown string escape").at_line(line_number));
        }
    };
    let mut value = String::new();
    for _ in 0..digits {
        let digit = chars.next().ok_or_else(|| {
            Error::parse(format!("incomplete {kind} escape")).at_line(line_number)
        })?;
        if !digit.is_ascii_hexdigit() {
            return Err(Error::parse(format!("invalid {kind} escape")).at_line(line_number));
        }
        value.push(digit);
    }
    let scalar = u32::from_str_radix(&value, radix)
        .map_err(|_| Error::parse(format!("invalid {kind} escape")).at_line(line_number))?;
    if escaped == 'u' {
        char::from_u32(scalar).ok_or_else(|| {
            Error::parse("Unicode escape is not a valid scalar value").at_line(line_number)
        })
    } else {
        Ok(char::from_u32(scalar).expect("two hexadecimal digits form a scalar"))
    }
}

/// A trailing operator keeps the following physical line in the same expression.
/// This deliberately covers only explicit operators; ordinary newlines still end
/// statements, and an arrow still opens its normal indentation block.
fn line_continues(token: Option<&Token>) -> bool {
    matches!(
        token,
        Some(
            Token::Or
                | Token::And
                | Token::Not
                | Token::In
                | Token::Of
                | Token::Plus
                | Token::Minus
                | Token::Star
                | Token::Slash
                | Token::FloorDiv
                | Token::Percent
                | Token::Modulo
                | Token::Amp
                | Token::Pipe
                | Token::Caret
                | Token::ShiftLeft
                | Token::ShiftRight
                | Token::ShiftRightUnsigned
                | Token::Power
                | Token::EqEq
                | Token::NotEq
                | Token::Lt
                | Token::LtEq
                | Token::Gt
                | Token::GtEq
                | Token::Assign
                | Token::PlusAssign
                | Token::MinusAssign
                | Token::StarAssign
                | Token::SlashAssign
                | Token::FloorDivAssign
                | Token::PercentAssign
                | Token::ModuloAssign
                | Token::AmpAssign
                | Token::PipeAssign
                | Token::CaretAssign
                | Token::ShiftLeftAssign
                | Token::ShiftRightAssign
                | Token::ShiftRightUnsignedAssign
                | Token::PowerAssign
        )
    )
}
fn close_group(groups: &mut Vec<char>, closing: char, line: usize) -> Result<(), Error> {
    let Some(opening) = groups.pop() else {
        return Err(Error::parse("unmatched grouping delimiter").at_line(line));
    };
    let expected = match opening {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        _ => unreachable!("group stack only contains opening delimiters"),
    };
    if expected == closing {
        Ok(())
    } else {
        Err(Error::parse(format!("expected '{expected}', found '{closing}'")).at_line(line))
    }
}

fn collection_item_can_end(token: Option<&Token>) -> bool {
    !matches!(
        token,
        None | Some(
            Token::Comma
                | Token::Semi
                | Token::LBracket
                | Token::LBrace
                | Token::Colon
                | Token::Assign
                | Token::Plus
                | Token::Minus
                | Token::Star
                | Token::Slash
                | Token::And
                | Token::Or
                | Token::Pipe
                | Token::Caret
                | Token::Amp
                | Token::Arrow
                | Token::FatArrow
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_tokens_are_emitted_at_line_boundaries() {
        let tokens = lex("double = (x) ->\n  next = x + 1\n  next * 2\ndouble(20)").unwrap();
        assert!(tokens.contains(&Token::Indent));
        assert!(tokens.contains(&Token::Dedent));
    }

    #[test]
    fn block_comments_are_ignored_without_affecting_layout() {
        let tokens = lex("if true\n  ### explanation\n  ignored ? text\n  ###\n  42").unwrap();
        assert!(tokens.contains(&Token::Indent));
        assert!(tokens.contains(&Token::Number(42.)));
        assert!(lex("### one line ###\n42").is_ok());
        assert!(lex("### starts\n42").is_err());
    }

    #[test]
    fn unicode_xid_identifiers_accept_combining_marks_after_a_start_character() {
        assert!(lex("स्थित = 42\nस्थित").is_ok());
        assert!(lex("\u{94b}value = 1").is_err());
    }

    #[test]
    fn triple_quoted_heredocs_preserve_newlines_and_interpolation_mode() {
        let tokens = lex("message = \"\"\"first\nsecond #{1 + 1}\"\"\"").unwrap();
        assert!(
            matches!(tokens[2], Token::String(ref value, true) if value == "first\nsecond #{1 + 1}")
        );
        let tokens = lex("message = '''first\nsecond'''").unwrap();
        assert!(matches!(tokens[2], Token::String(ref value, false) if value == "first\nsecond"));
        assert!(lex("message = \"\"\"unfinished").is_err());
    }

    #[test]
    fn heredoc_markers_inside_comments_and_strings_are_not_reinterpreted() {
        let tokens = lex("###\n\"\"\"\n###\n42").unwrap();
        assert!(tokens.contains(&Token::Number(42.)));
        assert!(lex("# \"\"\"\n42").is_ok());
        assert!(lex("value = '\"\"\"'\nvalue").is_ok());
    }

    #[test]
    fn update_operators_are_lexed_before_single_arithmetic_tokens() {
        let tokens = lex("value++\n--value\nvalue += 1\nvalue -= 1").unwrap();
        assert!(tokens.contains(&Token::PlusPlus));
        assert!(tokens.contains(&Token::MinusMinus));
        assert!(tokens.contains(&Token::PlusAssign));
        assert!(tokens.contains(&Token::MinusAssign));
        let tokens = lex("a // b\na %% b\na //= b\na %%= b").unwrap();
        assert!(tokens.contains(&Token::FloorDiv));
        assert!(tokens.contains(&Token::Modulo));
        assert!(tokens.contains(&Token::FloorDivAssign));
        assert!(tokens.contains(&Token::ModuloAssign));
        let tokens = lex(
            "a & b | c ^ d ~e a << b a >> b a >>> b a &= b a |= b a ^= b a <<= b a >>= b a >>>= b",
        )
        .unwrap();
        assert!(tokens.contains(&Token::Amp));
        assert!(tokens.contains(&Token::Pipe));
        assert!(tokens.contains(&Token::Caret));
        assert!(tokens.contains(&Token::Tilde));
        assert!(tokens.contains(&Token::ShiftLeft));
        assert!(tokens.contains(&Token::ShiftRight));
        assert!(tokens.contains(&Token::ShiftRightUnsigned));
        assert!(tokens.contains(&Token::AmpAssign));
        assert!(tokens.contains(&Token::PipeAssign));
        assert!(tokens.contains(&Token::CaretAssign));
        assert!(tokens.contains(&Token::ShiftLeftAssign));
        assert!(tokens.contains(&Token::ShiftRightAssign));
        assert!(tokens.contains(&Token::ShiftRightUnsignedAssign));
    }

    #[test]
    fn trailing_operators_continue_without_layout_tokens() {
        let tokens = lex("value = 1 +\n  2 * 3\nnext = value").unwrap();
        assert!(tokens.contains(&Token::Plus));
        assert!(
            !tokens
                .windows(2)
                .any(|pair| { matches!(pair, [Token::Plus, Token::Semi]) })
        );
        assert!(!tokens.contains(&Token::Indent));
        assert!(!tokens.contains(&Token::Dedent));
    }
}
