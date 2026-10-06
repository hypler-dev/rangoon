use crate::{
    Artifact, BundleError, CandidateManifest, CompilationReport, Diagnostic, DiagnosticCode,
    Profile, Readiness, Severity,
    bundle::{HEADER_BYTES, MAGIC, MAX_BUNDLE_BYTES},
    candidate_id, canonical, compile, encode_bundle, inspect_bundle,
};
use rangoon_domain::{
    byte_digest,
    capability::{self, ContentReview, Reviewer},
    capability_v1::{Revision, RevisionProvenance},
};

fn id(prefix: &str, byte: char) -> String {
    format!("{prefix}:{}", byte.to_string().repeat(64))
}

fn reviewed_report(profile: Profile, content: &str) -> crate::CompilationReport {
    reviewed_report_with_title(profile, "Exact rules", content)
}

fn reviewed_report_with_title(
    profile: Profile,
    title: &str,
    content: &str,
) -> crate::CompilationReport {
    let capability_id = id("capability", 'a');
    let revision = Revision {
        id: capability::revision_id(&capability_id, None, title, content),
        parent_revision_id: None,
        title: title.into(),
        content: content.into(),
        sha256: byte_digest(content.as_bytes()),
        created_at_ms: 1,
        review: Some(ContentReview {
            reviewer: Reviewer::LocalOperator,
            reviewed_at_ms: 2,
        }),
        provenance: RevisionProvenance::Ordinary {},
    };
    compile(&capability_id, &revision, profile, &[]).unwrap()
}

#[test]
fn round_trip_preserves_exact_artifact_and_both_profiles() {
    for profile in [Profile::AgentsMdV1, Profile::ClaudeMdV1] {
        let report = reviewed_report(profile, "\u{feff}# Rules\r\nKeep café 🦀\r\n");
        let encoded = encode_bundle(&report).unwrap();
        assert_eq!(encoded.inspection.compilation, report);
        assert_eq!(inspect_bundle(&encoded.bytes).unwrap(), encoded.inspection);
        assert_eq!(
            encoded.inspection.compilation.artifact.content,
            "\u{feff}# Rules\r\nKeep café 🦀\r\n"
        );
    }
}

#[test]
fn encoder_rejects_unreviewed_blocked_and_mutated_reports() {
    let unreviewed = {
        let report = reviewed_report(Profile::AgentsMdV1, "rules");
        let mut value = report.clone();
        value.readiness = Readiness::ReviewRequired;
        value.candidate = None;
        value
    };
    assert_eq!(encode_bundle(&unreviewed), Err(BundleError::NotCandidate));

    let blocked = reviewed_report(Profile::ClaudeMdV1, "mail@example.test");
    assert_eq!(blocked.readiness, Readiness::Blocked);
    assert_eq!(encode_bundle(&blocked), Err(BundleError::NotCandidate));

    for mutate in [
        |report: &mut CompilationReport| report.schema_version = "fabricated".into(),
        |report: &mut CompilationReport| report.compilation_id = id("compilation", 'b'),
        |report: &mut CompilationReport| report.review_observation = None,
        |report: &mut CompilationReport| report.artifact.byte_length = 0,
        |report: &mut CompilationReport| {
            report.candidate.as_mut().unwrap().candidate_id = id("candidate", 'b');
        },
        |report: &mut CompilationReport| {
            report.candidate.as_mut().unwrap().manifest_sha256 = "b".repeat(64);
        },
    ] {
        let mut mutated = reviewed_report(Profile::AgentsMdV1, "rules");
        mutate(&mut mutated);
        assert_eq!(encode_bundle(&mutated), Err(BundleError::Invalid));
    }
}

#[test]
fn verifier_rejects_tampering_even_when_checksum_is_rehashed() {
    let encoded = encode_bundle(&reviewed_report(Profile::AgentsMdV1, "rules")).unwrap();
    let mut wrong_artifact = encoded.bytes.clone();
    let artifact_offset = HEADER_BYTES
        + usize::try_from(u64::from_be_bytes(
            wrong_artifact[MAGIC.len()..MAGIC.len() + 8]
                .try_into()
                .unwrap(),
        ))
        .unwrap();
    wrong_artifact[artifact_offset] = b'x';
    let trailer_start = wrong_artifact.len() - 64;
    let checksum = byte_digest(&wrong_artifact[..trailer_start]);
    wrong_artifact[trailer_start..].copy_from_slice(checksum.as_bytes());
    assert_eq!(inspect_bundle(&wrong_artifact), Err(BundleError::Invalid));

    let mut wrong_manifest = encoded.bytes.clone();
    let manifest_end = HEADER_BYTES
        + usize::try_from(u64::from_be_bytes(
            wrong_manifest[MAGIC.len()..MAGIC.len() + 8]
                .try_into()
                .unwrap(),
        ))
        .unwrap();
    let position = wrong_manifest[HEADER_BYTES..manifest_end]
        .windows(b"AGENTS.md".len())
        .position(|value| value == b"AGENTS.md")
        .unwrap()
        + HEADER_BYTES;
    wrong_manifest[position..position + b"AGENTS.md".len()].copy_from_slice(b"CLAUDE.md");
    let trailer_start = wrong_manifest.len() - 64;
    let checksum = byte_digest(&wrong_manifest[..trailer_start]);
    wrong_manifest[trailer_start..].copy_from_slice(checksum.as_bytes());
    assert_eq!(inspect_bundle(&wrong_manifest), Err(BundleError::Invalid));
}

#[test]
fn verifier_rejects_noncanonical_lengths_truncation_and_trailing_data() {
    let encoded = encode_bundle(&reviewed_report(Profile::AgentsMdV1, "rules")).unwrap();
    for end in 0..encoded.bytes.len() {
        assert_eq!(
            inspect_bundle(&encoded.bytes[..end]),
            Err(BundleError::Invalid)
        );
    }
    let mut trailing = encoded.bytes.clone();
    trailing.push(b'x');
    assert_eq!(inspect_bundle(&trailing), Err(BundleError::Invalid));
    let mut oversized = encoded.bytes.clone();
    oversized[MAGIC.len()..MAGIC.len() + 8].copy_from_slice(&((32_769_u64).to_be_bytes()));
    assert_eq!(inspect_bundle(&oversized), Err(BundleError::TooLarge));
}

#[test]
fn verifier_requires_canonical_manifest_and_closed_types() {
    let encoded = encode_bundle(&reviewed_report(Profile::AgentsMdV1, "rules")).unwrap();
    let manifest_end = HEADER_BYTES
        + usize::try_from(u64::from_be_bytes(
            encoded.bytes[MAGIC.len()..MAGIC.len() + 8]
                .try_into()
                .unwrap(),
        ))
        .unwrap();
    let mut wrong_type = encoded.bytes.clone();
    let target = b"instruction_semantics_unverified";
    let position = wrong_type[HEADER_BYTES..manifest_end]
        .windows(target.len())
        .position(|value| value == target)
        .unwrap()
        + HEADER_BYTES;
    wrong_type[position..position + target.len()]
        .copy_from_slice(b"instruction_semantics_UNVERIFIED");
    let trailer_start = wrong_type.len() - 64;
    let checksum = byte_digest(&wrong_type[..trailer_start]);
    wrong_type[trailer_start..].copy_from_slice(checksum.as_bytes());
    assert_eq!(inspect_bundle(&wrong_type), Err(BundleError::Invalid));
}

#[test]
fn verifier_rejects_coherently_rehashed_blocking_claude_manifest() {
    let mut report = reviewed_report(Profile::AgentsMdV1, "mail@example.test <!-- note -->");
    let profile = Profile::ClaudeMdV1;
    let profile_digest = byte_digest(&profile.canonical_descriptor_bytes().unwrap());
    let diagnostics = vec![
        Diagnostic {
            code: DiagnosticCode::InstructionSemanticsUnverified,
            severity: Severity::Warning,
            detail: None,
        },
        Diagnostic {
            code: DiagnosticCode::TargetEnvironmentUnqualified,
            severity: Severity::Warning,
            detail: None,
        },
        Diagnostic {
            code: DiagnosticCode::PossibleIncludeSyntax,
            severity: Severity::Error,
            detail: None,
        },
        Diagnostic {
            code: DiagnosticCode::PossibleCommentElision,
            severity: Severity::Error,
            detail: None,
        },
    ];
    let artifact = Artifact {
        path: profile.descriptor().artifact_path,
        content: report.artifact.content.clone(),
        sha256: report.artifact.sha256.clone(),
        byte_length: report.artifact.byte_length,
    };
    let compilation_id = crate::bundle::compilation_id(
        &profile_digest,
        &report.selected_revision,
        &artifact,
        &diagnostics,
    )
    .unwrap();
    let candidate = report.candidate.as_mut().unwrap();
    candidate.manifest.profile_digest = profile_digest.clone();
    candidate.manifest.artifact.path = artifact.path.clone();
    candidate.manifest.diagnostics = diagnostics.clone();
    candidate.manifest.compilation_id = compilation_id;
    candidate.manifest_sha256 = byte_digest(&canonical(&candidate.manifest).unwrap());
    candidate.candidate_id = candidate_id(
        &candidate.manifest.compilation_id,
        &candidate.manifest_sha256,
    );
    let raw = pack_bundle(
        &canonical(&candidate.manifest).unwrap(),
        report.artifact.content.as_bytes(),
    );
    assert_eq!(inspect_bundle(&raw), Err(BundleError::Invalid));
}

#[test]
fn verifier_rejects_closed_schema_missing_or_noncanonical_manifest_fields() {
    let encoded = encode_bundle(&reviewed_report(Profile::AgentsMdV1, "rules")).unwrap();
    let (manifest, artifact) = bundle_parts(&encoded.bytes);
    let review_start = manifest.find("\"reviewObservation\":").unwrap();
    let review_value_start = review_start + "\"reviewObservation\":".len();
    let review_end = review_value_start
        + manifest[review_value_start..]
            .find(",\"diagnostics\"")
            .unwrap();
    let null_review = format!(
        "{}\"reviewObservation\":null{}",
        &manifest[..review_start],
        &manifest[review_end..]
    );
    let reordered =
        serde_json::to_string(&serde_json::from_str::<serde_json::Value>(&manifest).unwrap())
            .unwrap();
    assert_ne!(reordered, manifest);
    for mutated in [
        format!(" {manifest}"),
        reordered,
        manifest.replacen(
            "\"schemaVersion\":\"rangoon.instruction-candidate.v1\"",
            "\"schemaVersion\":\"rangoon.instruction-candidate.v0\"",
            1,
        ),
        manifest.replacen(
            "\"reviewer\":\"local_operator\"",
            "\"reviewer\":\"wrong_operator\"",
            1,
        ),
        manifest.replacen("\"reviewedAtMs\":2", "\"reviewedAtMs\":-1", 1),
        manifest.replacen("\"reviewedAtMs\":2", "\"reviewedAtMs\":8640000000000001", 1),
        manifest.replacen("\"byteLength\":5", "\"byteLength\":\"5\"", 1),
        manifest.replacen("\"byteLength\":5", "\"byteLength\":5.0", 1),
        manifest.replacen("\"byteLength\":5", "\"byteLength\":-1", 1),
        null_review,
        manifest.replacen(",\"authority\":\"none\"", "", 1),
        manifest.replacen("\"parentRevisionId\":null,", "", 1),
        manifest.replacen(",\"detail\":null", "", 1),
        manifest.replacen(
            "{\"kind\":\"ordinary\"}",
            "{\"kind\":\"ordinary\",\"unknown\":true}",
            1,
        ),
        manifest.replacen(
            "{\"kind\":\"ordinary\"}",
            "{\"kind\":\"ordinary\",\"kind\":\"ordinary\"}",
            1,
        ),
        manifest.replacen(
            "{\"schemaVersion\":",
            "{\"schemaVersion\":\"rangoon.instruction-candidate.v1\",\"schemaVersion\":",
            1,
        ),
    ] {
        assert_eq!(
            inspect_bundle(&pack_bundle(mutated.as_bytes(), artifact.as_bytes())),
            Err(BundleError::Invalid)
        );
    }
}

#[test]
fn verifier_rejects_equivalent_unicode_escaped_manifest_string() {
    let encoded = encode_bundle(&reviewed_report_with_title(
        Profile::AgentsMdV1,
        "café",
        "rules",
    ))
    .unwrap();
    let (manifest, artifact) = bundle_parts(&encoded.bytes);
    let escaped = manifest.replacen("café", "caf\\u00e9", 1);
    assert_eq!(
        inspect_bundle(&pack_bundle(escaped.as_bytes(), artifact.as_bytes())),
        Err(BundleError::Invalid)
    );
}

#[test]
fn verifier_rejects_invalid_utf8_manifest_and_artifact() {
    let encoded = encode_bundle(&reviewed_report(Profile::AgentsMdV1, "rules")).unwrap();
    let mut invalid_manifest = encoded.bytes.clone();
    invalid_manifest[HEADER_BYTES] = 0xff;
    rehash(&mut invalid_manifest);
    assert_eq!(inspect_bundle(&invalid_manifest), Err(BundleError::Invalid));

    let manifest_length = usize::try_from(u64::from_be_bytes(
        encoded.bytes[MAGIC.len()..MAGIC.len() + 8]
            .try_into()
            .unwrap(),
    ))
    .unwrap();
    let mut invalid_artifact = encoded.bytes.clone();
    invalid_artifact[HEADER_BYTES + manifest_length] = 0xff;
    rehash(&mut invalid_artifact);
    assert_eq!(inspect_bundle(&invalid_artifact), Err(BundleError::Invalid));
}

#[test]
fn verifier_checks_coherent_semantic_manifest_fields() {
    let report = reviewed_report(Profile::AgentsMdV1, "rules");
    for bytes in [
        coherent_bundle(&report, |manifest| {
            manifest.artifact.path = "CLAUDE.md".into();
        }),
        coherent_bundle(&report, |manifest| {
            manifest.selected_revision.revision_id = id("revision", 'b');
        }),
        coherent_bundle(&report, |manifest| {
            manifest.selected_revision.sha256 = "b".repeat(64);
        }),
        coherent_bundle(&report, |manifest| {
            manifest.selected_revision.provenance = RevisionProvenance::Composition {
                application_id: id("composition-application", 'b'),
                composition_id: id("composition", 'c'),
                output_index: 16,
            };
        }),
        coherent_bundle(&report, |manifest| {
            manifest.selected_revision.provenance = RevisionProvenance::Composition {
                application_id: "composition-application:bad".into(),
                composition_id: id("composition", 'c'),
                output_index: 0,
            };
        }),
    ] {
        assert_eq!(inspect_bundle(&bytes), Err(BundleError::Invalid));
    }
}

#[test]
fn verifier_accepts_coherent_review_time_as_unauthenticated_observation() {
    let report = reviewed_report(Profile::AgentsMdV1, "rules");
    let baseline = encode_bundle(&report).unwrap();
    let changed = inspect_bundle(&coherent_bundle(&report, |manifest| {
        manifest.review_observation.as_mut().unwrap().reviewed_at_ms = 3;
    }))
    .unwrap();
    assert_eq!(
        changed.compilation.compilation_id,
        baseline.inspection.compilation.compilation_id
    );
    assert_ne!(
        changed.compilation.candidate.as_ref().unwrap().candidate_id,
        baseline
            .inspection
            .compilation
            .candidate
            .as_ref()
            .unwrap()
            .candidate_id
    );
    assert_ne!(changed.bundle_id, baseline.inspection.bundle_id);
    assert_eq!(
        changed
            .compilation
            .review_observation
            .unwrap()
            .reviewed_at_ms,
        3
    );
}

#[test]
fn verifier_rejects_invalid_ids_and_composition_graph_bounds() {
    let encoded = encode_bundle(&reviewed_report(Profile::AgentsMdV1, "rules")).unwrap();
    let (manifest, artifact) = bundle_parts(&encoded.bytes);
    let invalid_id = manifest.replacen("capability:aaaaaaaa", "capability:Aaaaaaaa", 1);
    assert_eq!(
        inspect_bundle(&pack_bundle(invalid_id.as_bytes(), artifact.as_bytes())),
        Err(BundleError::Invalid)
    );

    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/compilation/instruction-bundle-v1.json"
    ))
    .unwrap();
    let case = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == "synthetic_composition_reference_shape_only")
        .unwrap();
    let mut composition = decode_hex(case["bundleHex"].as_str().unwrap());
    let target = b"\"outputIndex\":15";
    let position = composition
        .windows(target.len())
        .position(|value| value == target)
        .unwrap();
    composition[position..position + target.len()].copy_from_slice(b"\"outputIndex\":16");
    let trailer_start = composition.len() - 64;
    let checksum = byte_digest(&composition[..trailer_start]);
    composition[trailer_start..].copy_from_slice(checksum.as_bytes());
    assert_eq!(inspect_bundle(&composition), Err(BundleError::Invalid));
}

#[test]
fn verifier_rejects_overflow_component_lengths() {
    let encoded = encode_bundle(&reviewed_report(Profile::AgentsMdV1, "rules")).unwrap();
    let mut overflow = encoded.bytes.clone();
    overflow[MAGIC.len() + 8..HEADER_BYTES].copy_from_slice(&u64::MAX.to_be_bytes());
    assert_eq!(inspect_bundle(&overflow), Err(BundleError::TooLarge));
}

#[test]
fn verifier_classifies_exact_oversized_artifact_and_file() {
    let raw_artifact = pack_bundle(b"{}", &vec![b'x'; 262_145]);
    assert_eq!(inspect_bundle(&raw_artifact), Err(BundleError::TooLarge));
    assert_eq!(
        inspect_bundle(&vec![0; MAX_BUNDLE_BYTES + 1]),
        Err(BundleError::TooLarge)
    );
}

#[test]
fn independent_bundle_vectors_match_encoder_and_inspector() {
    let bundle_fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/compilation/instruction-bundle-v1.json"
    ))
    .unwrap();
    let compiler_fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/compilation/instruction-v1.json"
    ))
    .unwrap();
    assert_eq!(
        bundle_fixture["schemaVersion"],
        "rangoon.instruction-bundle-vectors.v1"
    );
    let compiler_cases = compiler_fixture["cases"].as_array().unwrap();
    for case in bundle_fixture["cases"].as_array().unwrap() {
        let source = compiler_cases
            .iter()
            .find(|candidate| candidate["name"] == case["name"])
            .unwrap();
        let revision: Revision = serde_json::from_value(source["revision"].clone()).unwrap();
        let profile: Profile = serde_json::from_value(source["profile"].clone()).unwrap();
        let requirements: Vec<String> =
            serde_json::from_value(source["requirements"].clone()).unwrap();
        let report = compile(
            source["capabilityId"].as_str().unwrap(),
            &revision,
            profile,
            &requirements,
        )
        .unwrap();
        let encoded = encode_bundle(&report).unwrap();
        assert_eq!(
            encoded.bytes,
            decode_hex(case["bundleHex"].as_str().unwrap())
        );
        assert_eq!(
            encoded.bytes.len(),
            case["byteLength"].as_u64().unwrap() as usize
        );
        assert_eq!(encoded.inspection.sha256, case["sha256"]);
        assert_eq!(encoded.inspection.bundle_id, case["bundleId"]);
        let candidate = encoded.inspection.compilation.candidate.as_ref().unwrap();
        assert_eq!(candidate.candidate_id, case["candidateId"]);
        assert_eq!(candidate.manifest_sha256, case["manifestSha256"]);
        assert_eq!(
            encoded.inspection.compilation.compilation_id,
            case["compilationId"]
        );
        assert_eq!(inspect_bundle(&encoded.bytes).unwrap().compilation, report);
    }
}

#[test]
fn maximum_artifact_vector_matches_bounds_and_identities() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/compilation/instruction-bundle-v1.json"
    ))
    .unwrap();
    let maximum = &fixture["maximumContent"];
    let input = &maximum["input"];
    let capability_id = input["capabilityId"].as_str().unwrap();
    let content = input["repeat"]
        .as_str()
        .unwrap()
        .repeat(input["repeatCount"].as_u64().unwrap() as usize);
    let title = input["title"].as_str().unwrap();
    let revision = Revision {
        id: capability::revision_id(capability_id, None, title, &content),
        parent_revision_id: None,
        title: title.into(),
        content,
        sha256: byte_digest(input["repeat"].as_str().unwrap().repeat(262_144).as_bytes()),
        created_at_ms: input["createdAtMs"].as_i64().unwrap(),
        review: Some(ContentReview {
            reviewer: Reviewer::LocalOperator,
            reviewed_at_ms: input["reviewedAtMs"].as_i64().unwrap(),
        }),
        provenance: RevisionProvenance::Ordinary {},
    };
    let profile: Profile = serde_json::from_value(input["profile"].clone()).unwrap();
    let encoded = encode_bundle(&compile(capability_id, &revision, profile, &[]).unwrap()).unwrap();
    assert_eq!(
        encoded.bytes.len(),
        maximum["byteLength"].as_u64().unwrap() as usize
    );
    assert_eq!(encoded.inspection.sha256, maximum["sha256"]);
    assert_eq!(encoded.inspection.bundle_id, maximum["bundleId"]);
    assert_eq!(
        encoded
            .inspection
            .compilation
            .candidate
            .as_ref()
            .unwrap()
            .candidate_id,
        maximum["candidateId"]
    );
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]))
        .collect()
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("fixture must contain lowercase hexadecimal"),
    }
}

fn bundle_parts(bytes: &[u8]) -> (String, String) {
    let manifest_length = usize::try_from(u64::from_be_bytes(
        bytes[MAGIC.len()..MAGIC.len() + 8].try_into().unwrap(),
    ))
    .unwrap();
    let artifact_length = usize::try_from(u64::from_be_bytes(
        bytes[MAGIC.len() + 8..HEADER_BYTES].try_into().unwrap(),
    ))
    .unwrap();
    let manifest_end = HEADER_BYTES + manifest_length;
    let artifact_end = manifest_end + artifact_length;
    (
        String::from_utf8(bytes[HEADER_BYTES..manifest_end].to_vec()).unwrap(),
        String::from_utf8(bytes[manifest_end..artifact_end].to_vec()).unwrap(),
    )
}

fn coherent_bundle(
    report: &CompilationReport,
    mutate: impl FnOnce(&mut CandidateManifest),
) -> Vec<u8> {
    let mut manifest = report.candidate.as_ref().unwrap().manifest.clone();
    mutate(&mut manifest);
    let artifact = Artifact {
        path: manifest.artifact.path.clone(),
        content: report.artifact.content.clone(),
        sha256: manifest.artifact.sha256.clone(),
        byte_length: manifest.artifact.byte_length,
    };
    manifest.compilation_id = crate::bundle::compilation_id(
        &manifest.profile_digest,
        &manifest.selected_revision,
        &artifact,
        &manifest.diagnostics,
    )
    .unwrap();
    let manifest_bytes = canonical(&manifest).unwrap();
    pack_bundle(&manifest_bytes, report.artifact.content.as_bytes())
}

fn pack_bundle(manifest: &[u8], artifact: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(manifest.len() as u64).to_be_bytes());
    bytes.extend_from_slice(&(artifact.len() as u64).to_be_bytes());
    bytes.extend_from_slice(manifest);
    bytes.extend_from_slice(artifact);
    bytes.extend_from_slice(byte_digest(&bytes).as_bytes());
    bytes
}

fn rehash(bytes: &mut [u8]) {
    let trailer_start = bytes.len() - 64;
    let checksum = byte_digest(&bytes[..trailer_start]);
    bytes[trailer_start..].copy_from_slice(checksum.as_bytes());
}
