use std::{fmt::Write, fs, path::PathBuf};

use sha2::{Digest, Sha256};

const PINNED_SOURCES: [(&str, &str); 4] = [
    (
        "proto/openshell.proto",
        "eca4c0201ae623590c9deb6b0027a86b5193eb727d0e79c6852b40704d1a4d7d",
    ),
    (
        "proto/datamodel.proto",
        "e50bd875a760f54b80269c65ee47984cae424fb623b7f59551ba62550c23e70b",
    ),
    (
        "proto/sandbox.proto",
        "d06089ccd15b57519adb3236e7a4c16661c7c52c9f184f2f826246c62e090839",
    ),
    (
        "proto/options.proto",
        "b0e03562c92777978f6f3e0f46260cdb82f1d88d1c1837eab0e53cdd0f195eb7",
    ),
];

fn main() {
    println!("cargo:rerun-if-changed=proto/SOURCES.toml");
    let receipt = fs::read_to_string("proto/SOURCES.toml")
        .expect("pinned OpenShell source receipt must be readable");
    for (source, expected_hash) in PINNED_SOURCES {
        println!("cargo:rerun-if-changed={source}");
        verify_receipt_entry(&receipt, source, expected_hash);
        verify_pinned_source(source, expected_hash);
    }

    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("Rangoon OpenShell client requires its vendored protobuf compiler");
    // tonic-build 0.13 does not expose its prost configuration for ordinary
    // source compilation. This scoped build-script setting selects only the
    // vendored compiler for the invocation below.
    unsafe { std::env::set_var("PROTOC", protoc) };

    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .compile_protos(
            &[PathBuf::from("proto/openshell.proto")],
            &[PathBuf::from("proto")],
        )
        .expect("pinned OpenShell v0.1.4 protobuf generation must succeed");
}

fn verify_pinned_source(path: &str, expected_hash: &str) {
    let source = fs::read(path).expect("pinned OpenShell protobuf source must be readable");
    let mut actual_hash = String::with_capacity(64);
    for byte in Sha256::digest(source) {
        write!(&mut actual_hash, "{byte:02x}").expect("writing to String cannot fail");
    }
    assert_eq!(
        actual_hash, expected_hash,
        "pinned OpenShell protobuf source hash mismatch: {path}"
    );
}

fn verify_receipt_entry(receipt: &str, path: &str, expected_hash: &str) {
    let filename = path
        .strip_prefix("proto/")
        .expect("pinned source must be under proto");
    let expected_line = format!("\"{filename}\" = \"{expected_hash}\"");
    assert!(
        receipt.lines().any(|line| line.trim() == expected_line),
        "pinned OpenShell source receipt mismatch: {filename}"
    );
}
