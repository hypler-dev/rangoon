//! Allocation-bounded JSON preflight for untrusted cloud responses.

use crate::Diagnostic;
use std::collections::HashSet;

pub(crate) const RAW_LIMIT: usize = 1_048_576;
const MAX_DEPTH: usize = 16;
const MAX_KEYS: usize = 64;
const MAX_KEY_BYTES: usize = 128;
const MAX_ARRAY_ITEMS: usize = 512;
const MAX_VALUES: usize = 8_192;
const MAX_STRING_BYTES: usize = 262_144;
const MAX_NUMBER_BYTES: usize = 128;

/// Validate JSON syntax and resource bounds before typed deserialization.
///
/// Only decoded object keys are allocated while scanning. This makes duplicate
/// decoded keys observable before `serde_json` would otherwise overwrite one.
pub(crate) fn preflight(raw: &[u8]) -> Result<(), Diagnostic> {
    if raw.len() > RAW_LIMIT {
        return Err(Diagnostic::ResponseTooLarge);
    }
    std::str::from_utf8(raw).map_err(|_| Diagnostic::ResponseInvalid)?;
    let mut parser = Parser {
        raw,
        cursor: 0,
        values: 0,
    };
    parser.value(0)?;
    parser.space();
    if parser.cursor != raw.len() {
        return Err(Diagnostic::ResponseInvalid);
    }
    Ok(())
}

struct Parser<'a> {
    raw: &'a [u8],
    cursor: usize,
    values: usize,
}

impl Parser<'_> {
    fn invalid<T>(&self) -> Result<T, Diagnostic> {
        Err(Diagnostic::ResponseInvalid)
    }

    fn too_large<T>(&self) -> Result<T, Diagnostic> {
        Err(Diagnostic::ResponseTooLarge)
    }

    fn space(&mut self) {
        while self
            .raw
            .get(self.cursor)
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.cursor += 1;
        }
    }

    fn take(&mut self, expected: u8) -> Result<(), Diagnostic> {
        self.space();
        if self.raw.get(self.cursor) != Some(&expected) {
            return self.invalid();
        }
        self.cursor += 1;
        Ok(())
    }

    fn value(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.space();
        self.values += 1;
        if self.values > MAX_VALUES {
            return self.too_large();
        }
        match self.raw.get(self.cursor) {
            Some(b'{') => {
                if depth >= MAX_DEPTH {
                    return self.too_large();
                }
                self.object(depth + 1)
            }
            Some(b'[') => {
                if depth >= MAX_DEPTH {
                    return self.too_large();
                }
                self.array(depth + 1)
            }
            Some(b'"') => self.string(MAX_STRING_BYTES).map(|_| ()),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            _ => self.invalid(),
        }
    }

    fn object(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.take(b'{')?;
        self.space();
        if self.raw.get(self.cursor) == Some(&b'}') {
            self.cursor += 1;
            return Ok(());
        }
        let mut keys = HashSet::new();
        loop {
            if keys.len() >= MAX_KEYS {
                return self.too_large();
            }
            self.space();
            let start = self.cursor;
            self.string(MAX_KEY_BYTES)?;
            let key: String = serde_json::from_slice(&self.raw[start..self.cursor])
                .map_err(|_| Diagnostic::ResponseInvalid)?;
            if !keys.insert(key) {
                return self.invalid();
            }
            self.take(b':')?;
            self.value(depth)?;
            self.space();
            match self.raw.get(self.cursor) {
                Some(b'}') => {
                    self.cursor += 1;
                    return Ok(());
                }
                Some(b',') => self.cursor += 1,
                _ => return self.invalid(),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<(), Diagnostic> {
        self.take(b'[')?;
        self.space();
        if self.raw.get(self.cursor) == Some(&b']') {
            self.cursor += 1;
            return Ok(());
        }
        let mut count = 0;
        loop {
            count += 1;
            if count > MAX_ARRAY_ITEMS {
                return self.too_large();
            }
            self.value(depth)?;
            self.space();
            match self.raw.get(self.cursor) {
                Some(b']') => {
                    self.cursor += 1;
                    return Ok(());
                }
                Some(b',') => self.cursor += 1,
                _ => return self.invalid(),
            }
        }
    }

    /// Scan a string and count decoded UTF-8 bytes without allocating it.
    fn string(&mut self, limit: usize) -> Result<usize, Diagnostic> {
        if self.raw.get(self.cursor) != Some(&b'"') {
            return self.invalid();
        }
        self.cursor += 1;
        let mut decoded = 0usize;
        loop {
            match self.raw.get(self.cursor).copied() {
                Some(b'"') => {
                    self.cursor += 1;
                    return Ok(decoded);
                }
                Some(b'\\') => {
                    self.cursor += 1;
                    let width = match self.raw.get(self.cursor).copied() {
                        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                            self.cursor += 1;
                            1
                        }
                        Some(b'u') => {
                            self.cursor += 1;
                            let first = self.hex4()?;
                            let scalar = if (0xd800..=0xdbff).contains(&first) {
                                if self.raw.get(self.cursor..self.cursor + 2) != Some(b"\\u") {
                                    return self.invalid();
                                }
                                self.cursor += 2;
                                let second = self.hex4()?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return self.invalid();
                                }
                                0x1_0000 + ((u32::from(first) - 0xd800) << 10) + u32::from(second)
                                    - 0xdc00
                            } else if (0xdc00..=0xdfff).contains(&first) {
                                return self.invalid();
                            } else {
                                u32::from(first)
                            };
                            char::from_u32(scalar)
                                .ok_or(Diagnostic::ResponseInvalid)?
                                .len_utf8()
                        }
                        _ => return self.invalid(),
                    };
                    decoded = decoded
                        .checked_add(width)
                        .ok_or(Diagnostic::ResponseTooLarge)?;
                }
                Some(0..=31) | None => return self.invalid(),
                Some(byte) => {
                    let width = match byte {
                        0..=0x7f => 1,
                        0xc2..=0xdf => 2,
                        0xe0..=0xef => 3,
                        0xf0..=0xf4 => 4,
                        _ => return self.invalid(),
                    };
                    self.cursor += width;
                    decoded = decoded
                        .checked_add(width)
                        .ok_or(Diagnostic::ResponseTooLarge)?;
                }
            }
            if decoded > limit {
                return self.too_large();
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, Diagnostic> {
        let mut value = 0u16;
        for _ in 0..4 {
            let digit = match self.raw.get(self.cursor).copied() {
                Some(byte @ b'0'..=b'9') => byte - b'0',
                Some(byte @ b'a'..=b'f') => byte - b'a' + 10,
                Some(byte @ b'A'..=b'F') => byte - b'A' + 10,
                _ => return self.invalid(),
            };
            value = (value << 4) | u16::from(digit);
            self.cursor += 1;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<(), Diagnostic> {
        let start = self.cursor;
        if self.raw.get(self.cursor) == Some(&b'-') {
            self.cursor += 1;
        }
        match self.raw.get(self.cursor) {
            Some(b'0') => self.cursor += 1,
            Some(b'1'..=b'9') => {
                self.cursor += 1;
                while self.raw.get(self.cursor).is_some_and(u8::is_ascii_digit) {
                    self.cursor += 1;
                }
            }
            _ => return self.invalid(),
        }
        if self.raw.get(self.cursor) == Some(&b'.') {
            self.cursor += 1;
            let start = self.cursor;
            while self.raw.get(self.cursor).is_some_and(u8::is_ascii_digit) {
                self.cursor += 1;
            }
            if self.cursor == start {
                return self.invalid();
            }
        }
        if matches!(self.raw.get(self.cursor), Some(b'e' | b'E')) {
            self.cursor += 1;
            if matches!(self.raw.get(self.cursor), Some(b'+' | b'-')) {
                self.cursor += 1;
            }
            let start = self.cursor;
            while self.raw.get(self.cursor).is_some_and(u8::is_ascii_digit) {
                self.cursor += 1;
            }
            if self.cursor == start {
                return self.invalid();
            }
        }
        if self.cursor - start > MAX_NUMBER_BYTES {
            return self.too_large();
        }
        let number = std::str::from_utf8(&self.raw[start..self.cursor])
            .map_err(|_| Diagnostic::ResponseInvalid)?;
        if !number.parse::<f64>().is_ok_and(|number| number.is_finite()) {
            return self.invalid();
        }
        Ok(())
    }

    fn literal(&mut self, token: &[u8]) -> Result<(), Diagnostic> {
        if self.raw.get(self.cursor..self.cursor + token.len()) != Some(token) {
            return self.invalid();
        }
        self.cursor += token.len();
        Ok(())
    }
}
