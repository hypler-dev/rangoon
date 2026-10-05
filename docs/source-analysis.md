# Source analysis foundation (R1a)

Authority: [intent.md](intent.md). Implementation status and test evidence: [development.md](development.md). This implements a pure Rust analysis library, versioned data records, and a stdin-only CLI. It does not complete R1 desktop shell qualification, R2 persistence/import, or R3 semantic decomposition.

## Useful behavior

A developer can supply an actual instruction file and receive a deterministic, source-preserving inventory of its Markdown sections. The result makes the raw source, original-byte digest, section spans, review state and incomplete parser coverage explicit. No model, cloud account, engine connection, filesystem scanner or tool executor is needed.

```sh
cargo run --locked -p rangoon-cli -- analyze --name AGENTS.md < AGENTS.md
```

The shell opens the source file; Rangoon reads only stdin. The display name is a label, not a filesystem path. On Windows, use binary-preserving stdin redirection, for example from Command Prompt. Text-mode PowerShell pipelines can change source encoding/newlines before the CLI receives them; hashes describe the bytes actually supplied. The library is the intended future desktop-bridge boundary: the host passes approved bytes and a display label, never an unrestricted path command.

Output includes the original source text. It is a local analysis artifact, not redacted telemetry. The program performs no network access and does not persist or publish the result. Neither input instructions nor output fragments are executed.

## Input contract

- UTF-8 bytes, optional UTF-8 BOM retained, no NUL; maximum 262,144 bytes (256 KiB).
- A nonempty Markdown basename no longer than 255 UTF-8 bytes. No slash, backslash, colon or control characters. Case-insensitive `.md` suffix. `AGENTS.md`, `CLAUDE.md`, and `SKILL.md` receive format labels; all other valid Markdown names are generic.
- Maximum 10,000 logical LF-delimited lines, 16,384 content bytes per line excluding LF/CRLF terminators, and 256 fragments including the preamble. Excess input is rejected, never silently truncated.
- The CLI reads at most the byte maximum plus one to detect overflow. It does not promise a wall-clock deadline on a pipe whose producer has not closed it.
- No directory recursion, path confinement claim, symlink handling, archive extraction, package installation, hooks, filesystem writes, model/provider calls, scripts, or authority requests exist in this slice.

## Markdown subset

ATX headings with one to six hashes, at most three leading spaces, and whitespace/end after the hash run start sections. A preamble retains bytes before the first heading. Nonempty source without a heading has one preamble. Empty source has no fragments and an explicit diagnostic. Duplicate titles remain separate span-bound records.

Backtick and tilde fences with a run of at least three markers hide headings until a matching marker run of at least the opening length, with only trailing whitespace. A BOM is ignored for first-line syntax recognition but retained in content and spans. An unclosed fence keeps the remaining content inert and reports its opening line. This is a deliberately limited section scanner, not a CommonMark implementation, semantic skill extractor, instruction resolver, sanitizer, or harness compiler. Heading title text is trimmed but retains any closing hash decoration. Setext headings, frontmatter interpretation, nested containers and HTML-block rules are not implemented. The `markdown_subset` diagnostic accompanies every result.

## JSON contract

`schemaVersion` is `rangoon.source-analysis.v0`; `analyzerVersion` is `0.1.0`. Version zero is experimental. JSON member ordering is not a canonical signing contract.

| Record | Meaning |
| --- | --- |
| `source.id` | Versioned SHA-256 identity binding display name and exact original bytes |
| `source.displayName` / `format` | Caller label and detected filename class; neither establishes actual harness compatibility |
| `source.sha256` | SHA-256 of the exact supplied bytes, including BOM and CRLF |
| `source.byteLength` / `lineCount` / `content` | Original bytes represented as lossless UTF-8 text with explicit counts |
| `fragments[]` | Ordered preamble/section records; source text is covered exactly once with no gaps or overlaps |
| `fragment.span` | Zero-based byte offsets, end exclusive; one-based start/end lines, inclusive |
| `fragment.id` | Versioned source-ID and byte-range identity; duplicate headings remain distinct |
| `fragment.heading` | Optional heading level/title; no semantic capability claim |
| `fragment.reviewState` | Always `unreviewed`; analysis never self-approves |
| `diagnostics[]` | Explicit empty-source, limited-parser and unclosed-fence observations |
| `authority` | Always `none`; record types contain no executable grant |

A trailing LF ends the last line without adding a phantom line. Fragment boundaries always follow whole line boundaries and preserve valid UTF-8. Recombining fragment text in order reconstructs the original source exactly.

Hash framing is explicit: source ID hashes ASCII `rangoon.source.v0` plus NUL, unsigned 64-bit big-endian name byte length, name bytes, unsigned 64-bit big-endian source length, source bytes. Fragment ID hashes ASCII `rangoon.fragment.v0` plus NUL, unsigned 64-bit big-endian source-ID string length, source-ID UTF-8 bytes, start byte and end byte as unsigned 64-bit big-endian integers. IDs use `source:` / `fragment:` prefixes followed by lowercase hex. These identifiers establish content identity only; they are not signed provenance, authenticated origin, authorization or policy decisions. SHA-256 uses the pinned [RustCrypto implementation](https://docs.rs/sha2/0.11.0/sha2/).

Serde rejects unknown record fields and unknown enum variants. Rust struct construction and deserialization are not a validation boundary for arbitrary external reports: schema-version acceptance, digest revalidation and trust checks will be required before consuming stored or remote records. No report-ingestion API exists here.

## Errors and exits

Success writes one JSON report to stdout and exits 0. `--help` prints usage without requiring stdin. Invalid arguments/input exit 2, with no result on stdout and a `rangoon.cli-error.v0` JSON error on stderr. I/O/serialization failures exit 3. Error messages are fixed text and never echo rejected source content. A broken stdout can still leave a partial stream at the receiver; callers must require both valid complete JSON and success exit.

Input error codes: `invalid_name`, `unsupported_format`, `input_too_large`, `invalid_utf8`, `binary_input`, `too_many_lines`, `line_too_long`, `too_many_fragments`. CLI-only errors: `usage`, `input_io`, `serialization`, `output_io`.

## Qualification and next slice

Run `cargo test --workspace --locked`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, and the existing Node preview checks. CI runs the Rust source suite on Linux, Windows and macOS with the minimum declared Rust version, plus the preview checks on Linux. CI runner success is source portability evidence, not a Windows 11 UI test, installer signature, native enforcement guarantee or supported desktop release.

Next: qualify the shared desktop shell/native bridge, then a user-selected-file import flow with explicit payload permission, durable snapshots, reviewable edits and recovery. Keep directory scanning, archive support, provider-assisted analysis and execution behind their own bounded contracts and tests.
