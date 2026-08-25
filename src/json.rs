use crate::vm::{
    decimal_source_failure_limit, decimal_text_resource_preflight,
    integer_digits_resource_preflight,
};
use crate::{Decimal, Integer, ResourceLimit, ResourceLimits, Value};
use num_bigint::BigInt;
use std::{collections::BTreeMap, fmt, rc::Rc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JsonFailure {
    message: String,
    resource_limit: Option<ResourceLimit>,
}

impl JsonFailure {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            resource_limit: None,
        }
    }

    fn resource(limit: ResourceLimit, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            resource_limit: Some(limit),
        }
    }

    pub(crate) fn resource_limit(&self) -> Option<ResourceLimit> {
        self.resource_limit
    }
}

impl fmt::Display for JsonFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

pub(crate) fn parse_json(source: &str, limits: ResourceLimits) -> Result<Value, JsonFailure> {
    if source.len() > limits.max_json_input_bytes() {
        return Err(JsonFailure::resource(
            ResourceLimit::JsonInputBytes,
            format!("JSON input exceeds {} bytes", limits.max_json_input_bytes()),
        ));
    }
    let mut parser = JsonParser {
        source,
        bytes: source.as_bytes(),
        cursor: 0,
        values: 0,
        limits,
    };
    parser.skip_whitespace();
    let value = parser.parse_value(0)?;
    parser.skip_whitespace();
    if parser.cursor != parser.bytes.len() {
        return Err(parser.syntax("unexpected trailing data"));
    }
    Ok(value)
}

pub(crate) fn encode_json(value: &Value, limits: ResourceLimits) -> Result<String, JsonFailure> {
    let mut encoder = JsonEncoder {
        output: String::new(),
        values: 0,
        limits,
    };
    encoder.encode_value(value, 0)?;
    Ok(encoder.output)
}

struct JsonParser<'a> {
    source: &'a str,
    bytes: &'a [u8],
    cursor: usize,
    values: usize,
    limits: ResourceLimits,
}

impl JsonParser<'_> {
    fn parse_value(&mut self, depth: usize) -> Result<Value, JsonFailure> {
        self.values += 1;
        if self.values > self.limits.max_json_values() {
            return Err(JsonFailure::resource(
                ResourceLimit::JsonValueCount,
                format!("JSON value count exceeds {}", self.limits.max_json_values()),
            ));
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
            if values.len() >= self.limits.max_json_container_items() {
                return Err(JsonFailure::resource(
                    ResourceLimit::JsonContainerItems,
                    format!(
                        "JSON array exceeds {} items",
                        self.limits.max_json_container_items()
                    ),
                ));
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
            if values.len() >= self.limits.max_json_container_items() {
                return Err(JsonFailure::resource(
                    ResourceLimit::JsonContainerItems,
                    format!(
                        "JSON object exceeds {} members",
                        self.limits.max_json_container_items()
                    ),
                ));
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
            if output.len() > self.limits.max_json_string_bytes() {
                return Err(JsonFailure::resource(
                    ResourceLimit::JsonStringBytes,
                    format!(
                        "JSON string exceeds {} bytes",
                        self.limits.max_json_string_bytes()
                    ),
                ));
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
            decimal_text_resource_preflight(source, self.limits).map_err(|error| {
                JsonFailure::resource(
                    error
                        .resource_limit()
                        .expect("decimal preflight only returns resource errors"),
                    error.message(),
                )
            })?;
            let value = Decimal::parse(source).ok_or_else(|| {
                JsonFailure::resource(
                    decimal_source_failure_limit(source),
                    "JSON decimal exceeds the implementation numeric limit",
                )
            })?;
            if value.scale() > self.limits.max_decimal_scale() {
                return Err(JsonFailure::resource(
                    ResourceLimit::DecimalScale,
                    format!("decimal scale exceeds {}", self.limits.max_decimal_scale()),
                ));
            }
            if value.inner().bits() > self.limits.max_decimal_coefficient_bits() {
                return Err(JsonFailure::resource(
                    ResourceLimit::DecimalCoefficientBits,
                    format!(
                        "decimal coefficient magnitude exceeds {} bits",
                        self.limits.max_decimal_coefficient_bits()
                    ),
                ));
            }
            Ok(Value::Decimal(Rc::new(value)))
        } else {
            let (negative, digits) = source
                .strip_prefix('-')
                .map_or((false, source), |digits| (true, digits));
            integer_digits_resource_preflight(digits, self.limits).map_err(|error| {
                JsonFailure::resource(
                    error
                        .resource_limit()
                        .expect("integer preflight only returns resource errors"),
                    error.message(),
                )
            })?;
            let mut value = BigInt::parse_bytes(digits.as_bytes(), 10)
                .ok_or_else(|| self.syntax("invalid JSON integer"))?;
            if negative {
                value = -value;
            }
            if value.bits() > self.limits.max_integer_bits() {
                return Err(JsonFailure::resource(
                    ResourceLimit::IntegerBits,
                    format!(
                        "integer magnitude exceeds {} bits",
                        self.limits.max_integer_bits()
                    ),
                ));
            }
            Ok(Value::Integer(Rc::new(
                Integer::from_bigint(value).map_err(|_| {
                    JsonFailure::resource(
                        ResourceLimit::IntegerBits,
                        "JSON integer exceeds the implementation numeric limit",
                    )
                })?,
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
        if depth >= self.limits.max_json_nesting_depth() {
            Err(JsonFailure::resource(
                ResourceLimit::JsonNestingDepth,
                format!(
                    "JSON nesting exceeds {}",
                    self.limits.max_json_nesting_depth()
                ),
            ))
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
    limits: ResourceLimits,
}

impl JsonEncoder {
    fn encode_value(&mut self, value: &Value, depth: usize) -> Result<(), JsonFailure> {
        self.values += 1;
        if self.values > self.limits.max_json_values() {
            return Err(JsonFailure::resource(
                ResourceLimit::JsonValueCount,
                format!("JSON value count exceeds {}", self.limits.max_json_values()),
            ));
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
                if values.len() > self.limits.max_json_container_items() {
                    return Err(JsonFailure::resource(
                        ResourceLimit::JsonContainerItems,
                        format!(
                            "JSON array exceeds {} items",
                            self.limits.max_json_container_items()
                        ),
                    ));
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
                if values.len() > self.limits.max_json_container_items() {
                    return Err(JsonFailure::resource(
                        ResourceLimit::JsonContainerItems,
                        format!(
                            "JSON object exceeds {} members",
                            self.limits.max_json_container_items()
                        ),
                    ));
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
            Value::Class(_) => Err(JsonFailure::new(
                "encode_json does not support Class values",
            )),
            Value::Instance(_) => Err(JsonFailure::new(
                "encode_json does not support Instance values",
            )),
        }
    }

    fn encode_string(&mut self, value: &str) -> Result<(), JsonFailure> {
        if value.len() > self.limits.max_json_string_bytes() {
            return Err(JsonFailure::resource(
                ResourceLimit::JsonStringBytes,
                format!(
                    "JSON string exceeds {} bytes",
                    self.limits.max_json_string_bytes()
                ),
            ));
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
        if self.output.len().saturating_add(value.len()) > self.limits.max_json_output_bytes() {
            return Err(JsonFailure::resource(
                ResourceLimit::JsonOutputBytes,
                format!(
                    "JSON output exceeds {} bytes",
                    self.limits.max_json_output_bytes()
                ),
            ));
        }
        self.output.push_str(value);
        Ok(())
    }

    fn check_depth(&self, depth: usize) -> Result<(), JsonFailure> {
        if depth >= self.limits.max_json_nesting_depth() {
            Err(JsonFailure::resource(
                ResourceLimit::JsonNestingDepth,
                format!(
                    "JSON nesting exceeds {}",
                    self.limits.max_json_nesting_depth()
                ),
            ))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{encode_json as encode_with_limits, parse_json as parse_with_limits};
    use crate::{ResourceLimit, ResourceLimits, Value};

    fn parse_json(source: &str) -> Result<Value, super::JsonFailure> {
        parse_with_limits(source, ResourceLimits::default())
    }

    fn encode_json(value: &Value) -> Result<String, super::JsonFailure> {
        encode_with_limits(value, ResourceLimits::default())
    }

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
        let defaults = ResourceLimits::default();
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
            "[".repeat(defaults.max_json_nesting_depth() + 1),
            "]".repeat(defaults.max_json_nesting_depth() + 1)
        );
        assert!(
            parse_json(&nested)
                .unwrap_err()
                .to_string()
                .contains("nesting")
        );
        let oversized = " ".repeat(defaults.max_json_input_bytes() + 1);
        assert!(
            parse_json(&oversized)
                .unwrap_err()
                .to_string()
                .contains("input exceeds")
        );
        assert!(
            encode_json(&Value::from("x".repeat(defaults.max_json_output_bytes())))
                .unwrap_err()
                .to_string()
                .contains("output exceeds")
        );
        assert!(encode_json(&Value::Number(f64::NAN)).is_err());

        let mut nested_value = Value::Nil;
        for _ in 0..=defaults.max_json_nesting_depth() {
            nested_value = Value::Array(std::rc::Rc::new(vec![nested_value]));
        }
        assert!(
            encode_json(&nested_value)
                .unwrap_err()
                .to_string()
                .contains("nesting")
        );
    }

    #[test]
    fn configurable_limits_accept_boundaries_and_report_stable_categories() {
        let defaults = ResourceLimits::default();

        let input_boundary = defaults.with_max_json_input_bytes(4);
        assert!(parse_with_limits("null", input_boundary).is_ok());
        assert_eq!(
            parse_with_limits("null", input_boundary.with_max_json_input_bytes(3))
                .unwrap_err()
                .resource_limit(),
            Some(ResourceLimit::JsonInputBytes)
        );

        let output_boundary = defaults.with_max_json_output_bytes(4);
        assert_eq!(
            encode_with_limits(&Value::Bool(true), output_boundary).unwrap(),
            "true"
        );
        assert_eq!(
            encode_with_limits(
                &Value::Bool(true),
                output_boundary.with_max_json_output_bytes(3)
            )
            .unwrap_err()
            .resource_limit(),
            Some(ResourceLimit::JsonOutputBytes)
        );

        let string_boundary = defaults.with_max_json_string_bytes(3);
        assert_eq!(
            parse_with_limits(r#""中""#, string_boundary)
                .unwrap()
                .as_str(),
            Some("中")
        );
        assert_eq!(
            parse_with_limits(r#""中""#, string_boundary.with_max_json_string_bytes(2))
                .unwrap_err()
                .resource_limit(),
            Some(ResourceLimit::JsonStringBytes)
        );

        let container_boundary = defaults.with_max_json_container_items(1);
        assert!(parse_with_limits("[0]", container_boundary).is_ok());
        assert_eq!(
            parse_with_limits("[0]", container_boundary.with_max_json_container_items(0))
                .unwrap_err()
                .resource_limit(),
            Some(ResourceLimit::JsonContainerItems)
        );

        let value_boundary = defaults.with_max_json_values(2);
        let one_item = Value::array([Value::from(0_i64)]);
        assert!(encode_with_limits(&one_item, value_boundary).is_ok());
        assert_eq!(
            encode_with_limits(&one_item, value_boundary.with_max_json_values(1))
                .unwrap_err()
                .resource_limit(),
            Some(ResourceLimit::JsonValueCount)
        );

        let depth_boundary = defaults.with_max_json_nesting_depth(1);
        assert!(parse_with_limits("[0]", depth_boundary).is_ok());
        assert_eq!(
            parse_with_limits("[[0]]", depth_boundary)
                .unwrap_err()
                .resource_limit(),
            Some(ResourceLimit::JsonNestingDepth)
        );

        assert_eq!(
            parse_json("1e100001").unwrap_err().resource_limit(),
            Some(ResourceLimit::DecimalScale)
        );
    }
}
