use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

const MAX_BYTES: usize = 16_384;
const MAX_ROOTS: usize = 32;
const MAX_CASES: usize = 64;
const SCHEMA_VERSION: &str = "rangoon.e1c5c.case-plan.v1";

pub(crate) const BYTES: &[u8] = include_bytes!("../native-encrypted-qualification/cases.v1.json");

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct CasePlan {
    schema_version: String,
    pub roots: Vec<String>,
    pub cases: Vec<Case>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Case {
    pub name: String,
    pub root: String,
    pub prerequisite: Prerequisite,
    pub seed: Seed,
    pub expectation: Expectation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Seed {
    None,
    LegacyEmpty,
    LegacyRich,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Prerequisite {
    Initial {},
    Completed { case: String },
    Running { case: String },
    AllStopped {},
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Expectation {
    ColdCancel,
    ColdRelaunch,
    Authoring,
    Relaunch,
    Export,
    LoseKey,
    MissingKey,
    BusyOwner,
    InterruptedSetup,
    InterruptedRelaunch,
    InterruptedProvisioning,
    AnchorFailure,
    CleanupFailure,
    WindowFailure,
    LegacyCopy,
    LegacyVerify,
    LegacyRelaunch,
    LegacyVerifyCancel,
    LegacyReviewCancel,
    LegacyReviewRelaunch,
    LegacyCopyCancel,
    CrashEmptyTarget,
    CrashMatchingTarget,
    CrashReopened,
    RecoveryInspectCancel,
    RecoveryInitializeEmptyCancel,
    RecoveryInitializeEmptyRestore,
    RecoveryRecordCancel,
    RecoverySelectionCancel,
    RecoveryRestore,
    RecoveryRecordCopied,
    RecoveryCompleteSelection,
    Cleanup,
}

impl Expectation {
    pub(crate) fn adoption_checkpoint(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::CrashEmptyTarget => Some(("destination", "after_confirm")),
            Self::CrashMatchingTarget => Some(("destination", "after_restore")),
            Self::CrashReopened => Some(("verification", "before_selection")),
            _ => None,
        }
    }
}

impl CasePlan {
    pub(crate) fn case(&self, name: &str) -> Option<&Case> {
        self.cases.iter().find(|case| case.name == name)
    }

    pub(crate) fn seed_for_root(&self, root: &str) -> Option<Seed> {
        self.cases
            .iter()
            .find(|case| {
                case.root == root && matches!(case.prerequisite, Prerequisite::Initial { .. })
            })
            .map(|case| case.seed)
    }
}

pub(crate) fn embedded() -> Result<CasePlan, ()> {
    parse(BYTES)
}

pub(crate) fn parse(bytes: &[u8]) -> Result<CasePlan, ()> {
    if bytes.len() > MAX_BYTES {
        return Err(());
    }
    let plan = serde_json::from_slice::<CasePlan>(bytes).map_err(|_| ())?;
    if plan.schema_version != SCHEMA_VERSION
        || !(1..=MAX_ROOTS).contains(&plan.roots.len())
        || !(1..=MAX_CASES).contains(&plan.cases.len())
    {
        return Err(());
    }
    validate(&plan)?;
    Ok(plan)
}

fn validate(plan: &CasePlan) -> Result<(), ()> {
    let mut roots = BTreeSet::new();
    for root in &plan.roots {
        valid_id(root)?;
        if !roots.insert(root.as_str()) {
            return Err(());
        }
    }

    let mut names = BTreeSet::new();
    let mut initial_roots = BTreeSet::new();
    for (index, case) in plan.cases.iter().enumerate() {
        valid_id(&case.name)?;
        valid_id(&case.root)?;
        if !roots.contains(case.root.as_str()) || !names.insert(case.name.as_str()) {
            return Err(());
        }

        match &case.prerequisite {
            Prerequisite::Initial { .. } => {
                if !initial_roots.insert(case.root.as_str()) {
                    return Err(());
                }
            }
            Prerequisite::AllStopped { .. } => {
                if case.seed != Seed::None {
                    return Err(());
                }
            }
            Prerequisite::Completed { case: required }
            | Prerequisite::Running { case: required } => {
                valid_id(required)?;
                if case.seed != Seed::None {
                    return Err(());
                }
                let Some(previous) = plan.cases[..index]
                    .iter()
                    .find(|previous| previous.name == *required)
                else {
                    return Err(());
                };
                if previous.root != case.root {
                    return Err(());
                }
            }
        }
    }

    if initial_roots.len() != roots.len() || !roots.iter().all(|root| initial_roots.contains(root))
    {
        return Err(());
    }
    Ok(())
}

fn valid_id(value: &str) -> Result<(), ()> {
    let bytes = value.as_bytes();
    if !(1..=48).contains(&bytes.len())
        || !bytes[0].is_ascii_lowercase()
        || bytes.last() == Some(&b'-')
        || bytes.windows(2).any(|pair| pair == b"--")
        || bytes
            .iter()
            .any(|byte| !byte.is_ascii_lowercase() && !byte.is_ascii_digit() && *byte != b'-')
    {
        return Err(());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const VERSION: &str = "rangoon.e1c5c.case-plan.v1";

    fn plan(roots: &str, cases: &str) -> String {
        format!(r#"{{"schemaVersion":"{VERSION}","roots":{roots},"cases":{cases}}}"#)
    }

    fn case(name: &str, root: &str, prerequisite: &str, seed: &str) -> String {
        format!(
            r#"{{"name":"{name}","root":"{root}","prerequisite":{prerequisite},"seed":"{seed}","expectation":"cold_cancel"}}"#
        )
    }

    fn valid_one_case() -> String {
        plan(
            "[\"root\"]",
            &format!(
                "[{}]",
                case("first", "root", r#"{"kind":"initial"}"#, "none")
            ),
        )
    }

    #[test]
    fn embedded_profile_matches_full_serialized_projection() {
        let parsed = embedded().expect("frozen profile parses");
        let frozen = serde_json::from_slice::<serde_json::Value>(BYTES).expect("frozen JSON");
        let projection = serde_json::to_value(&parsed).expect("case plan serializes");

        assert_eq!(projection, frozen);
        assert_eq!(parsed.roots.len(), 14);
        assert_eq!(parsed.cases.len(), 46);
        assert_eq!(
            parsed.case("legacy-rich-verify").map(|case| case.seed),
            Some(Seed::None)
        );
        assert_eq!(
            parsed.seed_for_root("legacy-empty"),
            Some(Seed::LegacyEmpty)
        );
        assert_eq!(
            parsed.seed_for_root("legacy-rich-provisioning"),
            Some(Seed::LegacyRich)
        );
        assert_eq!(parsed.seed_for_root("missing"), None);
    }

    #[test]
    fn retains_original_capacity_beyond_embedded_profile_and_refuses_excess() {
        let mut roots: Vec<_> = (0..32).map(|index| format!("root-{index}")).collect();
        let initial: Vec<_> = (0..32)
            .map(|index| {
                case(
                    &format!("initial-{index}"),
                    &roots[index],
                    r#"{"kind":"initial"}"#,
                    "none",
                )
            })
            .collect();
        let mut cases = initial.clone();
        for index in 32..64 {
            cases.push(case(
                &format!("follow-{index}"),
                "root-0",
                r#"{"kind":"completed","case":"initial-0"}"#,
                "none",
            ));
        }
        let capacity = plan(
            &serde_json::to_string(&roots).unwrap(),
            &format!("[{}]", cases.join(",")),
        );
        assert!(capacity.len() <= MAX_BYTES);
        assert!(parse(capacity.as_bytes()).is_ok());
        cases.push(case(
            "excess-case",
            "root-0",
            r#"{"kind":"completed","case":"initial-0"}"#,
            "none",
        ));
        assert!(
            parse(
                plan(
                    &serde_json::to_string(&roots).unwrap(),
                    &format!("[{}]", cases.join(","))
                )
                .as_bytes()
            )
            .is_err()
        );
        roots.push("excess-root".into());
        let mut cases = initial;
        cases.push(case(
            "excess-initial",
            "excess-root",
            r#"{"kind":"initial"}"#,
            "none",
        ));
        assert!(
            parse(
                plan(
                    &serde_json::to_string(&roots).unwrap(),
                    &format!("[{}]", cases.join(","))
                )
                .as_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn preserves_the_prior_forty_cases_and_adds_the_reviewed_provisioning_chain() {
        let parsed = embedded().expect("frozen profile parses");
        let prior: Vec<_> = parsed
            .cases
            .iter()
            .filter(|case| case.root != "legacy-rich-provisioning")
            .collect();
        assert_eq!(prior.len(), 40);
        assert_eq!(
            Sha256::digest(serde_json::to_vec(&prior).unwrap())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            "16dc0ab838334cecaba0815a6c270f0066bfa8c2f39de77eea4e51a3d5bfad5b"
        );

        for (name, prerequisite, seed, expectation) in [
            (
                "legacy-rich-provisioning-crash",
                Prerequisite::Initial {},
                Seed::LegacyRich,
                Expectation::InterruptedProvisioning,
            ),
            (
                "legacy-rich-provisioning-inspect-cancel",
                Prerequisite::Completed {
                    case: "legacy-rich-provisioning-crash".into(),
                },
                Seed::None,
                Expectation::RecoveryInspectCancel,
            ),
            (
                "legacy-rich-provisioning-initialize-cancel",
                Prerequisite::Completed {
                    case: "legacy-rich-provisioning-inspect-cancel".into(),
                },
                Seed::None,
                Expectation::RecoveryInitializeEmptyCancel,
            ),
            (
                "legacy-rich-provisioning-resume",
                Prerequisite::Completed {
                    case: "legacy-rich-provisioning-initialize-cancel".into(),
                },
                Seed::None,
                Expectation::RecoveryInitializeEmptyRestore,
            ),
            (
                "legacy-rich-provisioning-verify",
                Prerequisite::Completed {
                    case: "legacy-rich-provisioning-resume".into(),
                },
                Seed::None,
                Expectation::LegacyVerify,
            ),
            (
                "legacy-rich-provisioning-relaunch",
                Prerequisite::Completed {
                    case: "legacy-rich-provisioning-verify".into(),
                },
                Seed::None,
                Expectation::LegacyRelaunch,
            ),
        ] {
            let case = parsed.case(name).unwrap();
            assert_eq!(case.root, "legacy-rich-provisioning");
            assert_eq!(case.prerequisite, prerequisite);
            assert_eq!(case.seed, seed);
            assert_eq!(case.expectation, expectation);
        }
    }

    #[test]
    fn maps_only_closed_crash_expectations_to_adoption_checkpoints() {
        assert_eq!(
            Expectation::CrashEmptyTarget.adoption_checkpoint(),
            Some(("destination", "after_confirm"))
        );
        assert_eq!(
            Expectation::CrashMatchingTarget.adoption_checkpoint(),
            Some(("destination", "after_restore"))
        );
        assert_eq!(
            Expectation::CrashReopened.adoption_checkpoint(),
            Some(("verification", "before_selection"))
        );

        for expectation in [
            Expectation::ColdCancel,
            Expectation::ColdRelaunch,
            Expectation::Authoring,
            Expectation::Relaunch,
            Expectation::Export,
            Expectation::LoseKey,
            Expectation::MissingKey,
            Expectation::BusyOwner,
            Expectation::InterruptedSetup,
            Expectation::InterruptedRelaunch,
            Expectation::InterruptedProvisioning,
            Expectation::AnchorFailure,
            Expectation::CleanupFailure,
            Expectation::WindowFailure,
            Expectation::LegacyCopy,
            Expectation::LegacyVerify,
            Expectation::LegacyRelaunch,
            Expectation::LegacyVerifyCancel,
            Expectation::LegacyReviewCancel,
            Expectation::LegacyReviewRelaunch,
            Expectation::LegacyCopyCancel,
            Expectation::RecoveryInspectCancel,
            Expectation::RecoveryInitializeEmptyCancel,
            Expectation::RecoveryInitializeEmptyRestore,
            Expectation::RecoveryRecordCancel,
            Expectation::RecoverySelectionCancel,
            Expectation::RecoveryRestore,
            Expectation::RecoveryRecordCopied,
            Expectation::RecoveryCompleteSelection,
            Expectation::Cleanup,
        ] {
            assert_eq!(expectation.adoption_checkpoint(), None, "{expectation:?}");
        }
    }

    #[test]
    fn rejects_versions_trailing_bytes_and_oversize_input() {
        assert!(
            parse(
                valid_one_case()
                    .replace(VERSION, "rangoon.e1c5c.case-plan.v2")
                    .as_bytes()
            )
            .is_err()
        );
        assert!(parse(format!("{} x", valid_one_case()).as_bytes()).is_err());
        assert!(parse(&vec![b' '; MAX_BYTES + 1]).is_err());
    }

    #[test]
    fn rejects_bad_identifiers() {
        for value in ["A", "a-", "a--b", "a_b", "a b", &"a".repeat(49)] {
            let document = plan(
                &format!("[\"{value}\"]"),
                &format!(
                    "[{}]",
                    case("first", value, r#"{"kind":"initial"}"#, "none")
                ),
            );
            assert!(parse(document.as_bytes()).is_err(), "{value}");
        }
    }

    #[test]
    fn rejects_root_and_case_count_limits() {
        assert!(parse(plan("[]", "[]").as_bytes()).is_err());
        let roots = (0..33)
            .map(|index| format!("\"r{index}\""))
            .collect::<Vec<_>>()
            .join(",");
        assert!(parse(plan(&format!("[{roots}]"), "[]").as_bytes()).is_err());

        let mut cases = Vec::new();
        for index in 0..65 {
            let prerequisite = if index == 0 {
                r#"{"kind":"initial"}"#.to_owned()
            } else {
                format!(r#"{{"kind":"completed","case":"c{}"}}"#, index - 1)
            };
            cases.push(case(&format!("c{index}"), "root", &prerequisite, "none"));
        }
        assert!(parse(plan("[\"root\"]", &format!("[{}]", cases.join(","))).as_bytes()).is_err());
    }

    #[test]
    fn rejects_duplicate_json_keys_roots_and_cases() {
        assert!(parse(
            format!(
                r#"{{"schemaVersion":"{VERSION}","schemaVersion":"{VERSION}","roots":["root"],"cases":[{}]}}"#,
                case("first", "root", r#"{"kind":"initial"}"#, "none")
            )
            .as_bytes()
        )
        .is_err());
        assert!(parse(plan("[\"root\",\"root\"]", "[]").as_bytes()).is_err());
        let cases = format!(
            "[{},{}]",
            case("first", "root", r#"{"kind":"initial"}"#, "none"),
            case("first", "root", r#"{"kind":"all_stopped"}"#, "none")
        );
        assert!(parse(plan("[\"root\"]", &cases).as_bytes()).is_err());

        assert!(parse(
            plan(
                "[\"root\"]",
                r#"[{"name":"first","root":"root","prerequisite":{"kind":"initial","kind":"initial"},"seed":"none","expectation":"cold_cancel"}]"#,
            )
            .as_bytes()
        )
        .is_err());
        let repeated_case = format!(
            "[{},{}]",
            case("first", "root", r#"{"kind":"initial"}"#, "none"),
            r#"{"name":"second","root":"root","prerequisite":{"kind":"completed","case":"first","case":"first"},"seed":"none","expectation":"cold_cancel"}"#,
        );
        assert!(parse(plan("[\"root\"]", &repeated_case).as_bytes()).is_err());
        assert!(parse(
            format!(
                r#"{{"schemaVersion":"{VERSION}","schema\u0056ersion":"{VERSION}","roots":["root"],"cases":[{}]}}"#,
                case("first", "root", r#"{"kind":"initial"}"#, "none")
            )
            .as_bytes()
        )
        .is_err());
    }

    #[test]
    fn rejects_unknown_missing_and_wrong_typed_fields() {
        assert!(parse(
            plan(
                "[\"root\"]",
                r#"[{"name":"first","root":"root","prerequisite":{"kind":"initial"},"seed":"none","expectation":"cold_cancel","extra":true}]"#,
            )
            .as_bytes()
        )
        .is_err());
        assert!(parse(
            plan(
                "[\"root\"]",
                r#"[{"name":"first","root":"root","prerequisite":{"kind":"initial"},"seed":"none"}]"#,
            )
            .as_bytes()
        )
        .is_err());
        assert!(parse(
            plan(
                "[\"root\"]",
                r#"[{"name":false,"root":"root","prerequisite":{"kind":"initial"},"seed":"none","expectation":"cold_cancel"}]"#,
            )
            .as_bytes()
        )
        .is_err());
    }

    #[test]
    fn rejects_bad_prerequisite_shapes_and_seed_misuse() {
        assert!(parse(
            plan(
                "[\"root\"]",
                r#"[{"name":"first","root":"root","prerequisite":{"kind":"initial","case":"nope"},"seed":"none","expectation":"cold_cancel"}]"#,
            )
            .as_bytes()
        )
        .is_err());
        assert!(
            parse(
                plan(
                    "[\"root\"]",
                    &format!(
                        "[{},{}]",
                        case("first", "root", r#"{"kind":"initial"}"#, "none"),
                        case(
                            "later",
                            "root",
                            r#"{"kind":"completed","case":"first"}"#,
                            "legacy_rich"
                        )
                    ),
                )
                .as_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_unreferenced_roots_forward_refs_cycles_and_cross_root_refs() {
        assert!(
            parse(
                plan(
                    "[\"root\",\"unused\"]",
                    &format!(
                        "[{}]",
                        case("first", "root", r#"{"kind":"initial"}"#, "none")
                    )
                )
                .as_bytes()
            )
            .is_err()
        );

        let forward = format!(
            "[{},{}]",
            case(
                "first",
                "root",
                r#"{"kind":"completed","case":"later"}"#,
                "none"
            ),
            case("later", "root", r#"{"kind":"initial"}"#, "none")
        );
        assert!(parse(plan("[\"root\"]", &forward).as_bytes()).is_err());

        let cycle = format!(
            "[{},{}]",
            case(
                "first",
                "root",
                r#"{"kind":"completed","case":"second"}"#,
                "none"
            ),
            case(
                "second",
                "root",
                r#"{"kind":"completed","case":"first"}"#,
                "none"
            )
        );
        assert!(parse(plan("[\"root\"]", &cycle).as_bytes()).is_err());

        let cross_root = format!(
            "[{},{},{}]",
            case("left", "left", r#"{"kind":"initial"}"#, "none"),
            case("right", "right", r#"{"kind":"initial"}"#, "none"),
            case(
                "joined",
                "left",
                r#"{"kind":"completed","case":"right"}"#,
                "none"
            )
        );
        assert!(parse(plan("[\"left\",\"right\"]", &cross_root).as_bytes()).is_err());
    }
}
