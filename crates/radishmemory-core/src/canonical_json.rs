use std::collections::BTreeSet;

use crate::{CoreError, NonCanonicalJsonReason};

const MAX_NESTING_DEPTH: usize = 128;
const MAX_CANONICAL_NUMBER_BYTES: usize = 1024 * 1024;

/// Parsed `radishmemory-canonical-json-v1` content and its deterministic bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalJson {
    bytes: Vec<u8>,
}

impl CanonicalJson {
    /// Parses JSON, rejects duplicate keys and M0-forbidden nulls, then writes
    /// the frozen canonical representation.
    pub fn parse(input: &str) -> Result<Self, CoreError> {
        let value = Parser::new(input).parse()?;
        let mut output = String::new();
        write_value(&value, &mut output)?;
        Ok(Self {
            bytes: output.into_bytes(),
        })
    }

    /// Returns canonical UTF-8 bytes with no whitespace or trailing newline.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the value and returns the canonical bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Canonicalizes an M0 JSON input in one step.
pub fn canonicalize_json(input: &str) -> Result<Vec<u8>, CoreError> {
    CanonicalJson::parse(input).map(CanonicalJson::into_bytes)
}

#[derive(Clone, Debug)]
enum Value {
    Boolean(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

struct Parser<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<'a> Parser<'a> {
    const fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            cursor: 0,
        }
    }

    fn parse(mut self) -> Result<Value, CoreError> {
        self.skip_whitespace();
        let value = self.parse_value(0)?;
        self.skip_whitespace();
        if self.cursor != self.input.len() {
            return Err(json_error(NonCanonicalJsonReason::Syntax));
        }
        Ok(value)
    }

    fn parse_value(&mut self, depth: usize) -> Result<Value, CoreError> {
        if depth > MAX_NESTING_DEPTH {
            return Err(json_error(NonCanonicalJsonReason::NestingLimit));
        }
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => self.parse_string().map(Value::String),
            Some(b't') => self.parse_literal(b"true", Value::Boolean(true)),
            Some(b'f') => self.parse_literal(b"false", Value::Boolean(false)),
            Some(b'n') => self.parse_null(),
            Some(b'-' | b'0'..=b'9') => self.parse_number().map(Value::Number),
            _ => Err(json_error(NonCanonicalJsonReason::Syntax)),
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Value, CoreError> {
        self.cursor += 1;
        self.skip_whitespace();
        let mut members = Vec::new();
        let mut keys = BTreeSet::new();
        if self.consume(b'}') {
            return Ok(Value::Object(members));
        }

        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(json_error(NonCanonicalJsonReason::Syntax));
            }
            let key = self.parse_string()?;
            if !keys.insert(key.clone()) {
                return Err(json_error(NonCanonicalJsonReason::DuplicateKey));
            }
            self.skip_whitespace();
            if !self.consume(b':') {
                return Err(json_error(NonCanonicalJsonReason::Syntax));
            }
            let value = self.parse_value(depth + 1)?;
            members.push((key, value));
            self.skip_whitespace();
            if self.consume(b'}') {
                break;
            }
            if !self.consume(b',') {
                return Err(json_error(NonCanonicalJsonReason::Syntax));
            }
        }
        Ok(Value::Object(members))
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, CoreError> {
        self.cursor += 1;
        self.skip_whitespace();
        let mut items = Vec::new();
        if self.consume(b']') {
            return Ok(Value::Array(items));
        }

        loop {
            items.push(self.parse_value(depth + 1)?);
            self.skip_whitespace();
            if self.consume(b']') {
                break;
            }
            if !self.consume(b',') {
                return Err(json_error(NonCanonicalJsonReason::Syntax));
            }
        }
        Ok(Value::Array(items))
    }

    fn parse_string(&mut self) -> Result<String, CoreError> {
        let start = self.cursor;
        self.cursor += 1;
        while let Some(byte) = self.peek() {
            match byte {
                b'"' => {
                    self.cursor += 1;
                    return serde_json::from_slice(&self.input[start..self.cursor]).map_err(
                        |source| {
                            CoreError::non_canonical_json(
                                NonCanonicalJsonReason::Syntax,
                                Some(source),
                            )
                        },
                    );
                }
                b'\\' => {
                    self.cursor += 1;
                    if self.cursor >= self.input.len() {
                        return Err(json_error(NonCanonicalJsonReason::Syntax));
                    }
                    self.cursor += 1;
                }
                0x00..=0x1f => return Err(json_error(NonCanonicalJsonReason::Syntax)),
                _ => self.cursor += 1,
            }
        }
        Err(json_error(NonCanonicalJsonReason::Syntax))
    }

    fn parse_number(&mut self) -> Result<serde_json::Number, CoreError> {
        let start = self.cursor;
        while let Some(byte) = self.peek() {
            if matches!(byte, b' ' | b'\n' | b'\r' | b'\t' | b',' | b']' | b'}') {
                break;
            }
            self.cursor += 1;
        }
        serde_json::from_slice(&self.input[start..self.cursor]).map_err(|source| {
            CoreError::non_canonical_json(NonCanonicalJsonReason::Syntax, Some(source))
        })
    }

    fn parse_literal(&mut self, literal: &[u8], value: Value) -> Result<Value, CoreError> {
        let end = self.cursor + literal.len();
        if self.input.get(self.cursor..end) == Some(literal)
            && self
                .input
                .get(end)
                .is_none_or(|byte| is_value_boundary(*byte))
        {
            self.cursor += literal.len();
            Ok(value)
        } else {
            Err(json_error(NonCanonicalJsonReason::Syntax))
        }
    }

    fn parse_null(&mut self) -> Result<Value, CoreError> {
        let literal = b"null";
        let end = self.cursor + literal.len();
        if self.input.get(self.cursor..end) == Some(literal)
            && self
                .input
                .get(end)
                .is_none_or(|byte| is_value_boundary(*byte))
        {
            self.cursor += literal.len();
            Err(json_error(NonCanonicalJsonReason::NullForbidden))
        } else {
            Err(json_error(NonCanonicalJsonReason::Syntax))
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.cursor += 1;
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.cursor).copied()
    }
}

fn write_value(value: &Value, output: &mut String) -> Result<(), CoreError> {
    match value {
        Value::Boolean(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&normalize_number(value)?),
        Value::String(value) => write_string(value, output),
        Value::Array(items) => {
            output.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_value(item, output)?;
            }
            output.push(']');
        }
        Value::Object(members) => {
            let mut sorted = members.iter().collect::<Vec<_>>();
            sorted.sort_by(|left, right| left.0.chars().cmp(right.0.chars()));
            output.push('{');
            for (index, (key, item)) in sorted.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_string(key, output);
                output.push(':');
                write_value(item, output)?;
            }
            output.push('}');
        }
    }
    Ok(())
}

fn write_string(value: &str, output: &mut String) {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{0008}' => output.push_str("\\b"),
            '\u{0009}' => output.push_str("\\t"),
            '\u{000a}' => output.push_str("\\n"),
            '\u{000c}' => output.push_str("\\f"),
            '\u{000d}' => output.push_str("\\r"),
            '\u{0000}'..='\u{001f}' => {
                let code = character as usize;
                output.push_str("\\u00");
                output.push(HEX[code >> 4] as char);
                output.push(HEX[code & 0x0f] as char);
            }
            _ => output.push(character),
        }
    }
    output.push('"');
}

fn normalize_number(number: &serde_json::Number) -> Result<String, CoreError> {
    let raw = number.as_str();
    let (negative, unsigned) = raw
        .strip_prefix('-')
        .map_or((false, raw), |unsigned| (true, unsigned));
    let exponent_index = unsigned.find(['e', 'E']);
    let (mantissa, exponent) = match exponent_index {
        Some(index) => (&unsigned[..index], parse_exponent(&unsigned[index + 1..])?),
        None => (unsigned, 0_i64),
    };

    let (integer, fraction) = mantissa
        .split_once('.')
        .map_or((mantissa, ""), |parts| parts);
    let mut digits = String::with_capacity(integer.len() + fraction.len());
    digits.push_str(integer);
    digits.push_str(fraction);

    let Some(first_non_zero) = digits.bytes().position(|byte| byte != b'0') else {
        return Ok("0".to_owned());
    };
    let significant = digits[first_non_zero..].trim_end_matches('0');
    let integer_length = i64::try_from(integer.len())
        .map_err(|_| json_error(NonCanonicalJsonReason::NumberExpansionLimit))?;
    let leading_zeroes = i64::try_from(first_non_zero)
        .map_err(|_| json_error(NonCanonicalJsonReason::NumberExpansionLimit))?;
    let decimal_position = integer_length
        .checked_add(exponent)
        .and_then(|position| position.checked_sub(leading_zeroes))
        .ok_or_else(|| json_error(NonCanonicalJsonReason::NumberExpansionLimit))?;

    let mut output = String::new();
    if negative {
        output.push('-');
    }
    if decimal_position <= 0 {
        let zeroes = usize::try_from(-decimal_position)
            .map_err(|_| json_error(NonCanonicalJsonReason::NumberExpansionLimit))?;
        ensure_number_length(output.len() + 2 + zeroes + significant.len())?;
        output.push_str("0.");
        output.extend(std::iter::repeat_n('0', zeroes));
        output.push_str(significant);
    } else {
        let decimal_position = usize::try_from(decimal_position)
            .map_err(|_| json_error(NonCanonicalJsonReason::NumberExpansionLimit))?;
        if decimal_position >= significant.len() {
            let zeroes = decimal_position - significant.len();
            ensure_number_length(output.len() + significant.len() + zeroes)?;
            output.push_str(significant);
            output.extend(std::iter::repeat_n('0', zeroes));
        } else {
            ensure_number_length(output.len() + significant.len() + 1)?;
            output.push_str(&significant[..decimal_position]);
            output.push('.');
            output.push_str(&significant[decimal_position..]);
        }
    }
    Ok(output)
}

fn parse_exponent(value: &str) -> Result<i64, CoreError> {
    let (negative, digits) = value.strip_prefix('-').map_or_else(
        || (false, value.strip_prefix('+').unwrap_or(value)),
        |digits| (true, digits),
    );
    let mut exponent = 0_i64;
    for byte in digits.bytes() {
        exponent = exponent
            .checked_mul(10)
            .and_then(|current| current.checked_add(i64::from(byte - b'0')))
            .ok_or_else(|| json_error(NonCanonicalJsonReason::NumberExpansionLimit))?;
    }
    if negative {
        exponent
            .checked_neg()
            .ok_or_else(|| json_error(NonCanonicalJsonReason::NumberExpansionLimit))
    } else {
        Ok(exponent)
    }
}

fn ensure_number_length(length: usize) -> Result<(), CoreError> {
    if length > MAX_CANONICAL_NUMBER_BYTES {
        Err(json_error(NonCanonicalJsonReason::NumberExpansionLimit))
    } else {
        Ok(())
    }
}

fn json_error(reason: NonCanonicalJsonReason) -> CoreError {
    CoreError::non_canonical_json(reason, None)
}

const fn is_value_boundary(byte: u8) -> bool {
    matches!(byte, b' ' | b'\n' | b'\r' | b'\t' | b',' | b']' | b'}')
}
