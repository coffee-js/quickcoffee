use crate::{Decimal, Integer, Value};
use num_bigint::BigInt;
use std::{collections::BTreeMap, fmt, rc::Rc};

pub(crate) const MAX_JSON_INPUT_BYTES: usize = 1_000_000;
pub(crate) const MAX_JSON_OUTPUT_BYTES: usize = 1_000_000;
pub(crate) const MAX_JSON_STRING_BYTES: usize = 1_000_000;
pub(crate) const MAX_JSON_CONTAINER_ITEMS: usize = 100_000;
pub(crate) const MAX_JSON_VALUES: usize = 100_000;
pub(crate) const MAX_JSON_DEPTH: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JsonFailure {
    message: String,
}

impl JsonFailure {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for JsonFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

pub(crate) fn parse_json(source: &str) -> Result<Value, JsonFailure> {
    if source.len() > MAX_JSON_INPUT_BYTES {
        return Err(JsonFailure::new(format!(
            "JSON input exceeds {MAX_JSON_INPUT_BYTES} bytes"
        )));
    }
    let mut parser = JsonParser {
        source,
        bytes: source.as_bytes(),
        cursor: 0,
        values: 0,
    };
    parser.skip_whitespace();
    let value = parser.parse_value(0)?;
    parser.skip_whitespace();
    if parser.cursor != parser.bytes.len() {
        return Err(parser.syntax("unexpected trailing data"));
    }
    Ok(value)
}

pub(crate) fn encode_json(value: &Value) -> Result<String, JsonFailure> {
    let mut encoder = JsonEncoder {
        output: String::new(),
        values: 0,
    };
    encoder.encode_value(value, 0)?;
    Ok(encoder.output)
}

struct JsonParser<'a> {
    source: &'a str,
    bytes: &'a [u8],
    cursor: usize,
    values: usize,
}

impl JsonParser<'_> {
    fn parse_value(&mut self, depth: usize) -> Result<Value, JsonFailure> {
        self.values += 1;
        if self.values > MAX_JSON_VALUES {
            return Err(JsonFailure::new(format!(
                "JSON value count exceeds {MAX_JSON_VALUES}"
            )));
        }
        match self.peek() {
            Some(b'n') => {
                self.literal(b"null")?;
                Ok(Value::Nil)
            }
            Some(b't') => {
                self.literal(b"true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.literal(b"false")?;
                Ok(Value::Bool(false))
            }
            Some(b'"') => self.parse_string().map(Value::from),
            Some(b'[') => self.parse_array(depth),
            Some(b'{') => self.parse_object(depth),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(_) => Err(self.syntax("expected a JSON value")),
            None => Err(self.syntax("expected a JSON value")),
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, JsonFailure> {
        self.check_depth(depth)?;
        self.cursor += 1;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.consume(b']') {
            return Ok(Value::Array(Rc::new(values)));
        }
        loop {
            if values.len() >= MAX_JSON_CONTAINER_ITEMS {
                return Err(JsonFailure::new(format!(
                    "JSON array exceeds {MAX_JSON_CONTAINER_ITEMS} items"
                )));
            }
            values.push(self.parse_value(depth + 1)?);
            self.skip_whitespace();
            if self.consume(b']') {
                break;
            }
            self.expect(b',', "expected ',' or ']' after array item")?;
            self.skip_whitespace();
        }
        Ok(Value::Array(Rc::new(values)))
    }

    fn parse_object(&mut self, depth: usize) -> Result<Value, JsonFailure> {
        self.check_depth(depth)?;
        self.cursor += 1;
        self.skip_whitespace();
        let mut values = BTreeMap::new();
        if self.consume(b'}') {
            return Ok(Value::Map(Rc::new(values)));
        }
        loop {
            if values.len() >= MAX_JSON_CONTAINER_ITEMS {
                return Err(JsonFailure::new(format!(
                    "JSON object exceeds {MAX_JSON_CONTAINER_ITEMS} members"
                )));
            }
            if self.peek() != Some(b'"') {
                return Err(self.syntax("JSON object key must be a string"));
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.expect(b':', "expected ':' after JSON object key")?;
            self.skip_whitespace();
            let value = self.parse_value(depth + 1)?;
            if values.insert(key.clone(), value).is_some() {
                return Err(self.syntax(format!("duplicate JSON object key {key:?}")));
            }
            self.skip_whitespace();
            if self.consume(b'}') {
                break;
            }
            self.expect(b',', "expected ',' or '}' after object member")?;
            self.skip_whitespace();
        }
        Ok(Value::Map(Rc::new(values)))
    }

    fn parse_string(&mut self) -> Result<String, JsonFailure> {
        self.cursor += 1;
        let mut output = String::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err(self.syntax("unterminated JSON string"));
            };
            match byte {
                b'"' => {
                    self.cursor += 1;
                    return Ok(output);
                }
                b'\\' => {
                    self.cursor += 1;
                    self.parse_escape(&mut output)?;
                }
                0x00..=0x1f => {
                    return Err(self.syntax("unescaped control character in JSON string"));
                }
                0x20..=0x7f => {
                    output.push(char::from(byte));
                    self.cursor += 1;
                }
                _ => {
                    let character = self.source[self.cursor..]
                        .chars()
                        .next()
                        .expect("cursor is on a UTF-8 boundary");
                    output.push(character);
                    self.cursor += character.len_utf8();
                }
            }
            if output.len() > MAX_JSON_STRING_BYTES {
                return Err(JsonFailure::new(format!(
                    "JSON string exceeds {MAX_JSON_STRING_BYTES} bytes"
                )));
            }
        }
    }

    fn parse_escape(&mut self, output: &mut String) -> Result<(), JsonFailure> {
        let Some(escape) = self.peek() else {
            return Err(self.syntax("unterminated JSON escape"));
        };
        self.cursor += 1;
        match escape {
            b'"' => output.push('"'),
            b'\\' => output.push('\\'),
            b'/' => output.push('/'),
            b'b' => output.push('\u{0008}'),
            b'f' => output.push('\u{000c}'),
            b'n' => output.push('\n'),
            b'r' => output.push('\r'),
            b't' => output.push('\t'),
            b'u' => {
                let high = self.parse_hex_quad()?;
                let scalar = if (0xd800..=0xdbff).contains(&high) {
                    if !self.consume(b'\\') || !self.consume(b'u') {
                        return Err(self
                            .syntax("high surrogate must be followed by a low surrogate escape"));
                    }
                    let low = self.parse_hex_quad()?;
                    if !(0xdc00..=0xdfff).contains(&low) {
                        return Err(self.syntax("invalid low surrogate in JSON escape"));
                    }
                    0x10000 + ((u32::from(high) - 0xd800) << 10) + (u32::from(low) - 0xdc00)
                } else if (0xdc00..=0xdfff).contains(&high) {
                    return Err(self.syntax("unexpected low surrogate in JSON escape"));
                } else {
                    u32::from(high)
                };
                output.push(
                    char::from_u32(scalar)
                        .ok_or_else(|| self.syntax("invalid Unicode scalar in JSON escape"))?,
                );
            }
            _ => return Err(self.syntax("invalid JSON escape")),
        }
        Ok(())
    }

    fn parse_hex_quad(&mut self) -> Result<u16, JsonFailure> {
        if self.cursor + 4 > self.bytes.len() {
            return Err(self.syntax("incomplete Unicode escape"));
        }
        let mut value = 0_u16;
        for _ in 0..4 {
            let digit = match self.bytes[self.cursor] {
                byte @ b'0'..=b'9' => u16::from(byte - b'0'),
                byte @ b'a'..=b'f' => u16::from(byte - b'a' + 10),
                byte @ b'A'..=b'F' => u16::from(byte - b'A' + 10),
                _ => return Err(self.syntax("invalid hexadecimal digit in Unicode escape")),
            };
            value = value * 16 + digit;
            self.cursor += 1;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Value, JsonFailure> {
        let start = self.cursor;
        self.consume(b'-');
        match self.peek() {
            Some(b'0') => {
                self.cursor += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.syntax("JSON number cannot contain a leading zero"));
                }
            }
            Some(b'1'..=b'9') => {
                self.cursor += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.cursor += 1;
                }
            }
            _ => return Err(self.syntax("invalid JSON number")),
        }
        let mut decimal = false;
        if self.consume(b'.') {
            decimal = true;
            self.require_digits("JSON fraction requires at least one digit")?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            decimal = true;
            self.cursor += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.cursor += 1;
            }
            self.require_digits("JSON exponent requires at least one digit")?;
        }
        let source = &self.source[start..self.cursor];
        if decimal {
            let value = Decimal::parse(source)
                .ok_or_else(|| self.syntax("JSON decimal exceeds the exact numeric limits"))?;
            Ok(Value::Decimal(Rc::new(value)))
        } else {
            let (negative, digits) = source
                .strip_prefix('-')
                .map_or((false, source), |digits| (true, digits));
            let mut value = BigInt::parse_bytes(digits.as_bytes(), 10)
                .ok_or_else(|| self.syntax("invalid JSON integer"))?;
            if negative {
                value = -value;
            }
            Ok(Value::Integer(Rc::new(
                Integer::from_bigint(value)
                    .map_err(|_| self.syntax("JSON integer exceeds the exact numeric limits"))?,
            )))
        }
    }

    fn require_digits(&mut self, message: &str) -> Result<(), JsonFailure> {
        let start = self.cursor;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.cursor += 1;
        }
        if self.cursor == start {
            Err(self.syntax(message))
        } else {
            Ok(())
        }
    }

    fn literal(&mut self, literal: &[u8]) -> Result<(), JsonFailure> {
        if self.bytes[self.cursor..].starts_with(literal) {
            self.cursor += literal.len();
            Ok(())
        } else {
            Err(self.syntax("invalid JSON literal"))
        }
    }

    fn check_depth(&self, depth: usize) -> Result<(), JsonFailure> {
        if depth >= MAX_JSON_DEPTH {
            Err(JsonFailure::new(format!(
                "JSON nesting exceeds {MAX_JSON_DEPTH}"
            )))
        } else {
            Ok(())
        }
    }

    fn expect(&mut self, byte: u8, message: &str) -> Result<(), JsonFailure> {
        if self.consume(byte) {
            Ok(())
        } else {
            Err(self.syntax(message))
        }
    }

    fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.cursor).copied()
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.cursor += 1;
        }
    }

    fn syntax(&self, detail: impl fmt::Display) -> JsonFailure {
        JsonFailure::new(format!("invalid JSON at byte {}: {detail}", self.cursor))
    }
}

struct JsonEncoder {
    output: String,
    values: usize,
}

impl JsonEncoder {
    fn encode_value(&mut self, value: &Value, depth: usize) -> Result<(), JsonFailure> {
        self.values += 1;
        if self.values > MAX_JSON_VALUES {
            return Err(JsonFailure::new(format!(
                "JSON value count exceeds {MAX_JSON_VALUES}"
            )));
        }
        match value {
            Value::Nil => self.push("null"),
            Value::Bool(value) => self.push(if *value { "true" } else { "false" }),
            Value::Number(value) if value.is_finite() => self.push(&value.to_string()),
            Value::Number(_) => Err(JsonFailure::new(
                "encode_json rejects non-finite Number values",
            )),
            Value::Integer(value) => self.push(&value.to_decimal_string()),
            Value::Decimal(value) => {
                let plain = value.to_plain_string();
                if value.scale() == 0 {
                    self.push(&plain)?;
                    self.push(".0")
                } else {
                    self.push(&plain)
                }
            }
            Value::String(value) => self.encode_string(value),
            Value::Array(values) => {
                self.check_depth(depth)?;
                if values.len() > MAX_JSON_CONTAINER_ITEMS {
                    return Err(JsonFailure::new(format!(
                        "JSON array exceeds {MAX_JSON_CONTAINER_ITEMS} items"
                    )));
                }
                self.push("[")?;
                for (index, value) in values.iter().enumerate() {
                    if index > 0 {
                        self.push(",")?;
                    }
                    self.encode_value(value, depth + 1)?;
                }
                self.push("]")
            }
            Value::Map(values) => {
                self.check_depth(depth)?;
                if values.len() > MAX_JSON_CONTAINER_ITEMS {
                    return Err(JsonFailure::new(format!(
                        "JSON object exceeds {MAX_JSON_CONTAINER_ITEMS} members"
                    )));
                }
                self.push("{")?;
                for (index, (key, value)) in values.iter().enumerate() {
                    if index > 0 {
                        self.push(",")?;
                    }
                    self.encode_string(key)?;
                    self.push(":")?;
                    self.encode_value(value, depth + 1)?;
                }
                self.push("}")
            }
            Value::Error(_) => Err(JsonFailure::new(
                "encode_json does not support Error values",
            )),
            Value::Function(_) => Err(JsonFailure::new(
                "encode_json does not support Function values",
            )),
        }
    }

    fn encode_string(&mut self, value: &str) -> Result<(), JsonFailure> {
        if value.len() > MAX_JSON_STRING_BYTES {
            return Err(JsonFailure::new(format!(
                "JSON string exceeds {MAX_JSON_STRING_BYTES} bytes"
            )));
        }
        self.push("\"")?;
        for character in value.chars() {
            match character {
                '"' => self.push("\\\"")?,
                '\\' => self.push("\\\\")?,
                '\u{0008}' => self.push("\\b")?,
                '\u{000c}' => self.push("\\f")?,
                '\n' => self.push("\\n")?,
                '\r' => self.push("\\r")?,
                '\t' => self.push("\\t")?,
                '\u{0000}'..='\u{001f}' => {
                    self.push(&format!("\\u{:04x}", u32::from(character)))?;
                }
                _ => {
                    let mut buffer = [0_u8; 4];
                    self.push(character.encode_utf8(&mut buffer))?;
                }
            }
        }
        self.push("\"")
    }

    fn push(&mut self, value: &str) -> Result<(), JsonFailure> {
        if self.output.len().saturating_add(value.len()) > MAX_JSON_OUTPUT_BYTES {
            return Err(JsonFailure::new(format!(
                "JSON output exceeds {MAX_JSON_OUTPUT_BYTES} bytes"
            )));
        }
        self.output.push_str(value);
        Ok(())
    }

    fn check_depth(&self, depth: usize) -> Result<(), JsonFailure> {
        if depth >= MAX_JSON_DEPTH {
            Err(JsonFailure::new(format!(
                "JSON nesting exceeds {MAX_JSON_DEPTH}"
            )))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_JSON_DEPTH, MAX_JSON_INPUT_BYTES, encode_json, parse_json};
    use crate::Value;

    #[test]
    fn exact_numbers_and_canonical_objects_round_trip() {
        let value = parse_json(r#"{"small":1,"large":9007199254740993,"money":12.30,"whole":1e2}"#)
            .unwrap();
        assert_eq!(
            encode_json(&value).unwrap(),
            r#"{"large":9007199254740993,"money":12.3,"small":1,"whole":100.0}"#
        );
    }

    #[test]
    fn strings_decode_surrogates_and_reject_duplicates() {
        let value = parse_json(r#"{"emoji":"\ud83d\ude00","text":"a\n中"}"#).unwrap();
        assert_eq!(
            encode_json(&value).unwrap(),
            "{\"emoji\":\"😀\",\"text\":\"a\\n中\"}"
        );
        assert!(
            parse_json(r#"{"same":1,"same":2}"#)
                .unwrap_err()
                .to_string()
                .contains("duplicate JSON object key")
        );
    }

    #[test]
    fn canonical_round_trip_corpus_is_idempotent() {
        for source in [
            "null",
            "true",
            "-0",
            "9007199254740993",
            "1e2",
            "-12.3400",
            r#""quote\" slash\\ line\n scalar中 surrogate\ud83d\ude00""#,
            r#"[null,false,0,0.01,{"z":2,"a":1}]"#,
            r#"{"empty":{},"items":[],"nested":{"valid":true}}"#,
        ] {
            let encoded = encode_json(&parse_json(source).unwrap()).unwrap();
            let reencoded = encode_json(&parse_json(&encoded).unwrap()).unwrap();
            assert_eq!(reencoded, encoded, "{source}");
        }
    }

    #[test]
    fn malformed_numbers_nesting_and_size_limits_fail_atomically() {
        for source in [
            "",
            "-",
            "+1",
            ".1",
            "01",
            "1.",
            "1e",
            "1e+",
            "[1,]",
            "{\"key\":}",
            "true false",
            r#""\x00""#,
            r#""\ud800""#,
            r#""\udc00""#,
            "\"raw\nnewline\"",
        ] {
            assert!(parse_json(source).is_err(), "{source}");
        }
        let nested = format!(
            "{}null{}",
            "[".repeat(MAX_JSON_DEPTH + 1),
            "]".repeat(MAX_JSON_DEPTH + 1)
        );
        assert!(
            parse_json(&nested)
                .unwrap_err()
                .to_string()
                .contains("nesting")
        );
        let oversized = " ".repeat(MAX_JSON_INPUT_BYTES + 1);
        assert!(
            parse_json(&oversized)
                .unwrap_err()
                .to_string()
                .contains("input exceeds")
        );
        assert!(
            encode_json(&Value::from("x".repeat(MAX_JSON_INPUT_BYTES)))
                .unwrap_err()
                .to_string()
                .contains("output exceeds")
        );
        assert!(encode_json(&Value::Number(f64::NAN)).is_err());

        let mut nested_value = Value::Nil;
        for _ in 0..=MAX_JSON_DEPTH {
            nested_value = Value::Array(std::rc::Rc::new(vec![nested_value]));
        }
        assert!(
            encode_json(&nested_value)
                .unwrap_err()
                .to_string()
                .contains("nesting")
        );
    }
}
