//! Allocation-bounded JSON preflight before closed typed deserialization.
use crate::Diagnostic;
use std::collections::HashSet;

#[derive(Clone, Copy)]
pub(crate) enum WireKind {
    Request,
    Response,
}

impl WireKind {
    fn invalid(self) -> Diagnostic {
        match self {
            Self::Request => Diagnostic::InputInvalid,
            Self::Response => Diagnostic::ResponseInvalid,
        }
    }

    fn limit(self) -> Diagnostic {
        match self {
            Self::Request => Diagnostic::InputLimit,
            Self::Response => Diagnostic::ResponseLimit,
        }
    }
}

#[derive(Debug)]
enum Segment {
    Key(String),
    Index,
}

fn at(path: &[Segment], pattern: &[&str]) -> bool {
    path.len() == pattern.len()
        && path
            .iter()
            .zip(pattern)
            .all(|(segment, expected)| match segment {
                Segment::Key(key) => key == expected,
                Segment::Index => *expected == "*",
            })
}

pub(crate) fn preflight(raw: &[u8], kind: WireKind) -> Result<(), Diagnostic> {
    let cap = match kind {
        WireKind::Request => 8 * 1024 * 1024,
        WireKind::Response => 128 * 1024,
    };
    if raw.len() > cap {
        return Err(kind.limit());
    }
    std::str::from_utf8(raw).map_err(|_| kind.invalid())?;
    let mut parser = Parser {
        raw,
        cursor: 0,
        kind,
        values: 0,
        content_bytes: 0,
        authored_bytes: 0,
        selections: 0,
        required_ranges: 0,
        citations: 0,
    };
    parser.value(0, &mut Vec::new())?;
    parser.space();
    if parser.cursor != raw.len() {
        return Err(kind.invalid());
    }
    Ok(())
}

struct Parser<'a> {
    raw: &'a [u8],
    cursor: usize,
    kind: WireKind,
    values: usize,
    content_bytes: usize,
    authored_bytes: usize,
    selections: usize,
    required_ranges: usize,
    citations: usize,
}

impl Parser<'_> {
    fn space(&mut self) {
        while self
            .raw
            .get(self.cursor)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.cursor += 1;
        }
    }

    fn take(&mut self, byte: u8) -> Result<(), Diagnostic> {
        self.space();
        if self.raw.get(self.cursor) != Some(&byte) {
            return Err(self.kind.invalid());
        }
        self.cursor += 1;
        Ok(())
    }

    fn value(&mut self, depth: usize, path: &mut Vec<Segment>) -> Result<(), Diagnostic> {
        self.space();
        self.values += 1;
        if self.values > 8192 {
            return Err(self.kind.limit());
        }
        match self.raw.get(self.cursor) {
            Some(b'{') => {
                if depth >= 16 {
                    return Err(self.kind.limit());
                }
                self.object(depth + 1, path)
            }
            Some(b'[') => {
                if depth >= 16 {
                    return Err(self.kind.limit());
                }
                self.array(depth + 1, path)
            }
            Some(b'"') => {
                let length = self.string(self.string_cap(path))?;
                if at(path, &["inputs", "*", "content"]) {
                    self.content_bytes += length;
                    if self.content_bytes > 4 * 1024 * 1024 {
                        return Err(self.kind.limit());
                    }
                }
                if at(path, &["proposals", "*", "authoredText"]) {
                    self.authored_bytes += length;
                    if self.authored_bytes > 64 * 1024 {
                        return Err(self.kind.limit());
                    }
                }
                Ok(())
            }
            Some(b'0'..=b'9') => self.number(),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            _ => Err(self.kind.invalid()),
        }
    }

    fn object(&mut self, depth: usize, path: &mut Vec<Segment>) -> Result<(), Diagnostic> {
        self.take(b'{')?;
        self.space();
        if self.raw.get(self.cursor) == Some(&b'}') {
            self.cursor += 1;
            return Ok(());
        }
        let mut keys = HashSet::new();
        loop {
            if keys.len() >= 32 {
                return Err(self.kind.limit());
            }
            self.space();
            let start = self.cursor;
            self.string(128)?;
            // Only bounded keys are allocated; value strings are measured in place.
            let key: String = serde_json::from_slice(&self.raw[start..self.cursor])
                .map_err(|_| self.kind.invalid())?;
            if !keys.insert(key.clone()) {
                return Err(self.kind.invalid());
            }
            self.take(b':')?;
            path.push(Segment::Key(key));
            self.value(depth, path)?;
            path.pop();
            self.space();
            match self.raw.get(self.cursor) {
                Some(b'}') => {
                    self.cursor += 1;
                    return Ok(());
                }
                Some(b',') => {
                    self.cursor += 1;
                }
                _ => return Err(self.kind.invalid()),
            }
        }
    }

    fn array(&mut self, depth: usize, path: &mut Vec<Segment>) -> Result<(), Diagnostic> {
        self.take(b'[')?;
        self.space();
        if self.raw.get(self.cursor) == Some(&b']') {
            self.cursor += 1;
            return Ok(());
        }
        let cap = if at(path, &["inputs"]) || at(path, &["proposals"]) {
            16
        } else if at(path, &["inputs", "*", "selections"])
            || at(path, &["inputs", "*", "requiredProtectedRanges"])
        {
            256
        } else if at(path, &["proposals", "*", "citations"]) {
            64
        } else if at(path, &["uncertainties"]) {
            32
        } else {
            512
        };
        let mut count = 0;
        loop {
            count += 1;
            if count > cap {
                return Err(self.kind.limit());
            }
            if at(path, &["inputs", "*", "selections"]) {
                self.selections += 1;
                if self.selections > 256 {
                    return Err(self.kind.limit());
                }
            }
            if at(path, &["inputs", "*", "requiredProtectedRanges"]) {
                self.required_ranges += 1;
                if self.required_ranges > 256 {
                    return Err(self.kind.limit());
                }
            }
            if at(path, &["proposals", "*", "citations"]) {
                self.citations += 1;
                if self.citations > 256 {
                    return Err(self.kind.limit());
                }
            }
            path.push(Segment::Index);
            self.value(depth, path)?;
            path.pop();
            self.space();
            match self.raw.get(self.cursor) {
                Some(b']') => {
                    self.cursor += 1;
                    return Ok(());
                }
                Some(b',') => {
                    self.cursor += 1;
                }
                _ => return Err(self.kind.invalid()),
            }
        }
    }

    fn string_cap(&self, path: &[Segment]) -> usize {
        if matches!(self.kind, WireKind::Request) && at(path, &["inputs", "*", "content"]) {
            return 256 * 1024;
        }
        if at(path, &["inputs", "*", "scope"]) {
            return 512;
        }
        if at(path, &["proposals", "*", "authoredText"]) {
            return 16 * 1024;
        }
        if at(path, &["proposals", "*", "title"]) {
            return 120;
        }
        if at(path, &["proposals", "*", "explanation"]) || at(path, &["uncertainties", "*"]) {
            return 1024;
        }
        if at(path, &["target", "profileId"]) || at(path, &["target", "profileSha256"]) {
            return 64;
        }
        if at(path, &["target", "model"]) {
            return 128;
        }
        // Reference paths vary by input/response nesting, but their field widths do not.
        match path.last() {
            Some(Segment::Key(key))
                if key == "sourceId" || key == "capabilityId" || key == "revisionId" =>
            {
                80
            }
            Some(Segment::Key(key)) if key == "sha256" || key == "schemaVersion" => 64,
            Some(Segment::Key(key)) if key == "kind" || key == "task" => 32,
            _ => 64 * 1024,
        }
    }

    /// Scan without constructing the decoded string, including escaped Unicode.
    fn string(&mut self, cap: usize) -> Result<usize, Diagnostic> {
        if self.raw.get(self.cursor) != Some(&b'"') {
            return Err(self.kind.invalid());
        }
        self.cursor += 1;
        let mut length = 0;
        loop {
            match self.raw.get(self.cursor).copied() {
                Some(b'"') => {
                    self.cursor += 1;
                    return Ok(length);
                }
                Some(b'\\') => {
                    self.cursor += 1;
                    match self.raw.get(self.cursor).copied() {
                        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                            self.cursor += 1;
                            length += 1;
                        }
                        Some(b'u') => {
                            self.cursor += 1;
                            let first = self.hex4()?;
                            let scalar = if (0xd800..=0xdbff).contains(&first) {
                                if self.raw.get(self.cursor..self.cursor + 2) != Some(b"\\u") {
                                    return Err(self.kind.invalid());
                                }
                                self.cursor += 2;
                                let second = self.hex4()?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return Err(self.kind.invalid());
                                }
                                0x10000 + ((u32::from(first) - 0xd800) << 10) + u32::from(second)
                                    - 0xdc00
                            } else if (0xdc00..=0xdfff).contains(&first) {
                                return Err(self.kind.invalid());
                            } else {
                                u32::from(first)
                            };
                            length += char::from_u32(scalar)
                                .ok_or_else(|| self.kind.invalid())?
                                .len_utf8();
                        }
                        _ => return Err(self.kind.invalid()),
                    }
                }
                Some(0..=31) | None => return Err(self.kind.invalid()),
                Some(_) => {
                    self.cursor += 1;
                    length += 1;
                }
            }
            if length > cap {
                return Err(self.kind.limit());
            }
        }
    }

    fn hex4(&mut self) -> Result<u16, Diagnostic> {
        let mut result = 0u16;
        for _ in 0..4 {
            let value = match self.raw.get(self.cursor).copied() {
                Some(c @ b'0'..=b'9') => c - b'0',
                Some(c @ b'a'..=b'f') => c - b'a' + 10,
                Some(c @ b'A'..=b'F') => c - b'A' + 10,
                _ => return Err(self.kind.invalid()),
            };
            result = (result << 4) | u16::from(value);
            self.cursor += 1;
        }
        Ok(result)
    }

    fn number(&mut self) -> Result<(), Diagnostic> {
        let start = self.cursor;
        while self.raw.get(self.cursor).is_some_and(u8::is_ascii_digit) {
            self.cursor += 1;
            if self.cursor - start > 20 {
                return Err(self.kind.limit());
            }
        }
        let bytes = &self.raw[start..self.cursor];
        if bytes.len() > 1 && bytes[0] == b'0' {
            return Err(self.kind.invalid());
        }
        let value = std::str::from_utf8(bytes).map_err(|_| self.kind.invalid())?;
        value.parse::<u64>().map_err(|_| self.kind.invalid())?;
        Ok(())
    }

    fn literal(&mut self, token: &[u8]) -> Result<(), Diagnostic> {
        if self.raw.get(self.cursor..self.cursor + token.len()) != Some(token) {
            return Err(self.kind.invalid());
        }
        self.cursor += token.len();
        Ok(())
    }
}

#[cfg(test)]
#[path = "json_tests.rs"]
mod tests;
