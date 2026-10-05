//! Deterministic, inert Markdown source analysis.
#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt;

use rangoon_domain::{
    ANALYZER_VERSION, AnalysisReport, Authority, Diagnostic, DiagnosticCode, FragmentKind, Heading,
    ReviewState, SCHEMA_VERSION, SourceFormat, SourceFragment, SourceSnapshot, SourceSpan,
    byte_digest, fragment_id, source_id,
};

pub const MAX_SOURCE_BYTES: usize = 262_144;
pub const MAX_LINES: usize = 10_000;
pub const MAX_LINE_BYTES: usize = 16_384;
pub const MAX_FRAGMENTS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidName,
    UnsupportedFormat,
    InputTooLarge,
    InvalidUtf8,
    BinaryInput,
    TooManyLines,
    LineTooLong,
    TooManyFragments,
}

impl ErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidName => "invalid_name",
            Self::UnsupportedFormat => "unsupported_format",
            Self::InputTooLarge => "input_too_large",
            Self::InvalidUtf8 => "invalid_utf8",
            Self::BinaryInput => "binary_input",
            Self::TooManyLines => "too_many_lines",
            Self::LineTooLong => "line_too_long",
            Self::TooManyFragments => "too_many_fragments",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnalysisError {
    pub code: ErrorCode,
    pub message: &'static str,
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl Error for AnalysisError {}

const INVALID_NAME: AnalysisError = AnalysisError {
    code: ErrorCode::InvalidName,
    message: "display name must be a single non-control basename",
};
const UNSUPPORTED_FORMAT: AnalysisError = AnalysisError {
    code: ErrorCode::UnsupportedFormat,
    message: "only Markdown display names ending in .md are supported",
};
const INPUT_TOO_LARGE: AnalysisError = AnalysisError {
    code: ErrorCode::InputTooLarge,
    message: "source exceeds the maximum byte length",
};
const INVALID_UTF8: AnalysisError = AnalysisError {
    code: ErrorCode::InvalidUtf8,
    message: "source must be valid UTF-8",
};
const BINARY_INPUT: AnalysisError = AnalysisError {
    code: ErrorCode::BinaryInput,
    message: "source must not contain NUL bytes",
};
const TOO_MANY_LINES: AnalysisError = AnalysisError {
    code: ErrorCode::TooManyLines,
    message: "source exceeds the maximum line count",
};
const LINE_TOO_LONG: AnalysisError = AnalysisError {
    code: ErrorCode::LineTooLong,
    message: "source contains a line exceeding the maximum byte length",
};
const TOO_MANY_FRAGMENTS: AnalysisError = AnalysisError {
    code: ErrorCode::TooManyFragments,
    message: "source exceeds the maximum fragment count",
};

const MARKDOWN_SUBSET_MESSAGE: &str = "ATX Markdown sections are unreviewed proposals only; this analysis does not establish semantic portability.";

/// Validate a caller-supplied display label. This performs no path access.
pub fn validate_display_name(name: &str) -> Result<SourceFormat, AnalysisError> {
    if name.is_empty()
        || name.len() > 255
        || matches!(name, "." | "..")
        || name.chars().any(|character| {
            character == '/' || character == '\\' || character == ':' || character.is_control()
        })
    {
        return Err(INVALID_NAME);
    }

    let extension = name.get(name.len().saturating_sub(3)..);
    if !matches!(extension, Some(value) if value.eq_ignore_ascii_case(".md")) {
        return Err(UNSUPPORTED_FORMAT);
    }
    let stem = &name[..name.len() - 3];
    if stem.is_empty() {
        return Err(INVALID_NAME);
    }

    if stem.eq_ignore_ascii_case("agents") {
        Ok(SourceFormat::AgentsMarkdown)
    } else if stem.eq_ignore_ascii_case("claude") {
        Ok(SourceFormat::ClaudeMarkdown)
    } else if stem.eq_ignore_ascii_case("skill") {
        Ok(SourceFormat::SkillMarkdown)
    } else {
        Ok(SourceFormat::GenericMarkdown)
    }
}

/// Analyze caller-provided bytes as an inert, bounded Markdown subset.
pub fn analyze(display_name: &str, bytes: &[u8]) -> Result<AnalysisReport, AnalysisError> {
    let format = validate_display_name(display_name)?;
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err(INPUT_TOO_LARGE);
    }
    if bytes.contains(&0) {
        return Err(BINARY_INPUT);
    }
    let content = std::str::from_utf8(bytes).map_err(|_| INVALID_UTF8)?;
    let lines = split_inclusive_lines(bytes)?;

    let source_identity = source_id(display_name, bytes);
    let mut diagnostics = vec![Diagnostic {
        code: DiagnosticCode::MarkdownSubset,
        message: MARKDOWN_SUBSET_MESSAGE.to_owned(),
        line: None,
    }];
    let mut headings = Vec::new();
    let mut open_fence = None;

    for line in &lines {
        let source_line = &bytes[line.start..line.end];
        if let Some(fence) = open_fence {
            if is_closing_fence(source_line, fence) {
                open_fence = None;
            }
            continue;
        }
        if let Some(mut fence) = opening_fence(source_line, line.start == 0) {
            fence.line = line.number;
            open_fence = Some(fence);
            continue;
        }
        if let Some(heading) = parse_heading(source_line, line.start, line.start == 0) {
            headings.push(heading);
        }
    }

    if let Some(fence) = open_fence {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::UnclosedFence,
            message: "Markdown fence remains unclosed; later headings stay inert text".to_owned(),
            line: Some(fence.line),
        });
    }

    if bytes.is_empty() {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::EmptySource,
            message: "source is empty; no section proposals were created".to_owned(),
            line: None,
        });
    }

    let fragments = build_fragments(bytes, &lines, &headings, &source_identity)?;
    Ok(AnalysisReport {
        schema_version: SCHEMA_VERSION.to_owned(),
        analyzer_version: ANALYZER_VERSION.to_owned(),
        source: SourceSnapshot {
            id: source_identity,
            display_name: display_name.to_owned(),
            format,
            sha256: byte_digest(bytes),
            byte_length: bytes.len() as u64,
            line_count: lines.len() as u32,
            content: content.to_owned(),
        },
        fragments,
        diagnostics,
        authority: Authority::None,
    })
}

#[derive(Clone, Copy)]
struct Line {
    start: usize,
    end: usize,
    number: u32,
}

fn split_inclusive_lines(bytes: &[u8]) -> Result<Vec<Line>, AnalysisError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'\n' {
            lines.push(Line {
                start,
                end: index + 1,
                number: (lines.len() + 1) as u32,
            });
            start = index + 1;
        }
    }
    if start < bytes.len() {
        lines.push(Line {
            start,
            end: bytes.len(),
            number: (lines.len() + 1) as u32,
        });
    }
    if lines.len() > MAX_LINES {
        return Err(TOO_MANY_LINES);
    }
    for line in &lines {
        if line_content(&bytes[line.start..line.end]).len() > MAX_LINE_BYTES {
            return Err(LINE_TOO_LONG);
        }
    }
    Ok(lines)
}

#[derive(Clone, Copy)]
struct Fence {
    marker: u8,
    length: usize,
    line: u32,
}

fn opening_fence(line: &[u8], first_line: bool) -> Option<Fence> {
    let content = line_content(line);
    let content = if first_line {
        content.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(content)
    } else {
        content
    };
    let indent = content.iter().take_while(|byte| **byte == b' ').count();
    if indent > 3 {
        return None;
    }
    let rest = &content[indent..];
    let marker = *rest.first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let length = rest.iter().take_while(|byte| **byte == marker).count();
    (length >= 3).then_some(Fence {
        marker,
        length,
        line: 0,
    })
}

fn is_closing_fence(line: &[u8], fence: Fence) -> bool {
    let (indent, rest) = indentation_and_rest(line);
    if indent > 3 || rest.first().copied() != Some(fence.marker) {
        return false;
    }
    let length = rest
        .iter()
        .take_while(|byte| **byte == fence.marker)
        .count();
    length >= fence.length && rest[length..].iter().all(|byte| byte.is_ascii_whitespace())
}

fn indentation_and_rest(line: &[u8]) -> (usize, &[u8]) {
    let content = line_content(line);
    let indent = content.iter().take_while(|byte| **byte == b' ').count();
    (indent, &content[indent..])
}

/// Exclude only LF or CRLF terminators. A bare trailing CR remains source content.
fn line_content(line: &[u8]) -> &[u8] {
    match line.strip_suffix(b"\n") {
        Some(without_lf) => without_lf.strip_suffix(b"\r").unwrap_or(without_lf),
        None => line,
    }
}

struct ParsedHeading {
    start: usize,
    heading: Heading,
}

fn parse_heading(line: &[u8], line_start: usize, first_line: bool) -> Option<ParsedHeading> {
    let source_line = if first_line {
        match line_content(line).strip_prefix(b"\xEF\xBB\xBF") {
            Some(after_bom) => after_bom,
            None => line_content(line),
        }
    } else {
        line_content(line)
    };
    let indent = source_line.iter().take_while(|byte| **byte == b' ').count();
    if indent > 3 {
        return None;
    }
    let rest = &source_line[indent..];
    let level = rest.iter().take_while(|byte| **byte == b'#').count();
    if !(1..=6).contains(&level) || !matches!(rest.get(level), None | Some(b' ' | b'\t')) {
        return None;
    }
    let title_bytes = rest[level..].trim_ascii();
    let title = std::str::from_utf8(title_bytes).ok()?.to_owned();
    Some(ParsedHeading {
        start: line_start,
        heading: Heading {
            level: level as u8,
            title,
        },
    })
}

fn build_fragments(
    bytes: &[u8],
    lines: &[Line],
    headings: &[ParsedHeading],
    source_identity: &str,
) -> Result<Vec<SourceFragment>, AnalysisError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    let needs_preamble = match headings.first() {
        Some(heading) => heading.start > 0,
        None => true,
    };
    let fragment_count = headings.len() + usize::from(needs_preamble);
    if fragment_count > MAX_FRAGMENTS {
        return Err(TOO_MANY_FRAGMENTS);
    }

    let mut fragments = Vec::with_capacity(fragment_count);
    if let Some(first_heading) = headings.first() {
        if first_heading.start > 0 {
            fragments.push(make_fragment(
                bytes,
                lines,
                source_identity,
                FragmentKind::Preamble,
                None,
                0,
                first_heading.start,
            ));
        }
        for (index, heading) in headings.iter().enumerate() {
            let end = headings
                .get(index + 1)
                .map_or(bytes.len(), |next| next.start);
            fragments.push(make_fragment(
                bytes,
                lines,
                source_identity,
                FragmentKind::Section,
                Some(heading.heading.clone()),
                heading.start,
                end,
            ));
        }
    } else {
        fragments.push(make_fragment(
            bytes,
            lines,
            source_identity,
            FragmentKind::Preamble,
            None,
            0,
            bytes.len(),
        ));
    }
    Ok(fragments)
}

fn make_fragment(
    bytes: &[u8],
    lines: &[Line],
    source_identity: &str,
    kind: FragmentKind,
    heading: Option<Heading>,
    start: usize,
    end: usize,
) -> SourceFragment {
    let span = SourceSpan {
        start_byte: start as u64,
        end_byte: end as u64,
        start_line: line_for_offset(lines, start),
        end_line: line_for_offset(lines, end - 1),
    };
    SourceFragment {
        id: fragment_id(source_identity, span),
        kind,
        heading,
        span,
        text: std::str::from_utf8(&bytes[start..end])
            .expect("source was validated as UTF-8")
            .to_owned(),
        review_state: ReviewState::Unreviewed,
    }
}

fn line_for_offset(lines: &[Line], offset: usize) -> u32 {
    lines
        .iter()
        .find(|line| line.start <= offset && offset < line.end)
        .expect("nonempty fragment offsets are covered by a source line")
        .number
}
