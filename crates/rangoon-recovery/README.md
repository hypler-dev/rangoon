# Historical recovery-envelope experiment

This standalone crate preserves the E1d-1 offline age experiment and its test
fixtures. It is excluded from Rangoon's active workspace and is not used by the
desktop application, native recovery commands or portable CLI. It does not define
an active backup format, migration path, compatibility reader or production
security claim. No published Rangoon backup format depended on this experiment.

The candidate was not promoted: the selected age crate remains beta/testing-only,
and its graph includes unmaintained compile-time dependencies. The newer candidate
also introduced a dependency advisory requiring separate assessment. This is not a
claim that the experiment demonstrated an X25519 exploit.

Native encrypted recovery instead reuses Rangoon's existing pinned SQLCipher
backend and an independently generated recovery key. See
[the canonical storage specification](../../docs/encrypted-workspace.md).

Historical checks remain runnable explicitly with the pinned Rust toolchain:

```sh
cargo test --manifest-path crates/rangoon-recovery/Cargo.toml --locked --offline
```

Do not add a dependency on this crate or maintain dual recovery profiles without a
new evidence-backed and reviewed product requirement.
