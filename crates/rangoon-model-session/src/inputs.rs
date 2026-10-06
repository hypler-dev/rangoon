use crate::Diagnostic;
use rangoon_domain::{capability::valid_id, composition::InputReference};
use rangoon_model_assistance::ContextPack;
use rangoon_model_local::{LocalProfile, PreparedRequest};
use rangoon_store::Workspace;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io::Write};

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Task {
    ClassifyV1,
    DecomposeV1,
    CompareV1,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Selector {
    Source {
        source_id: String,
    },
    Capability {
        capability_id: String,
        revision_id: String,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Selection {
    schema_version: String,
    pub(crate) task: Task,
    pub(crate) inputs: Vec<Selector>,
}

impl Selection {
    pub(crate) fn parse(raw: &[u8]) -> Result<Self, Diagnostic> {
        if raw.len() > 8192 {
            return Err(Diagnostic::InvalidRequest);
        }
        let value: Self = serde_json::from_slice(raw).map_err(|_| Diagnostic::InvalidRequest)?;
        if value.schema_version != "rangoon.local-selection.v1"
            || !(1..=16).contains(&value.inputs.len())
            || value.inputs.iter().collect::<HashSet<_>>().len() != value.inputs.len()
            || value.inputs.iter().any(|input| !match input {
                Selector::Source { source_id } => valid_id(source_id, "source:"),
                Selector::Capability {
                    capability_id,
                    revision_id,
                } => valid_id(capability_id, "capability:") && valid_id(revision_id, "revision:"),
            })
            || (value.task == Task::CompareV1
                && (value.inputs.len() != 2
                    || value
                        .inputs
                        .iter()
                        .any(|s| !matches!(s, Selector::Capability { .. }))))
        {
            return Err(Diagnostic::InvalidRequest);
        }
        Ok(value)
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub input: InputReference,
    pub observed_head: Option<String>,
    pub byte_length: usize,
}

pub(crate) struct Resolved {
    pub(crate) selector: Selector,
    pub(crate) dependency: Dependency,
    pub(crate) content: String,
}

impl Resolved {
    pub(crate) fn read(store: &Workspace, selector: &Selector) -> Result<Self, Diagnostic> {
        let (input, observed_head, content) = match selector {
            Selector::Source { source_id } => {
                let report = store
                    .open(source_id)
                    .map_err(|_| Diagnostic::InputUnavailable)?;
                (
                    InputReference::Source {
                        source_id: source_id.clone(),
                        sha256: report.source.sha256,
                    },
                    None,
                    report.source.content,
                )
            }
            Selector::Capability {
                capability_id,
                revision_id,
            } => {
                let detail = store
                    .open_capability_v1(capability_id, Some(revision_id))
                    .map_err(|_| Diagnostic::InputUnavailable)?;
                (
                    InputReference::Revision {
                        capability_id: capability_id.clone(),
                        revision_id: revision_id.clone(),
                        sha256: detail.revision.sha256,
                    },
                    Some(detail.latest_revision_id),
                    detail.revision.content,
                )
            }
        };
        if content.is_empty() {
            return Err(Diagnostic::InputEmpty);
        }
        if content.len() > 262144 {
            return Err(Diagnostic::PackOverBudget);
        }
        Ok(Self {
            selector: selector.clone(),
            dependency: Dependency {
                input,
                observed_head,
                byte_length: content.len(),
            },
            content,
        })
    }

    pub(crate) fn matches(&self, other: &Self) -> bool {
        self.dependency.input == other.dependency.input
            && self.dependency.observed_head == other.dependency.observed_head
            && self.content == other.content
    }
}

pub(crate) struct Packed {
    pub(crate) request: PreparedRequest,
    pub(crate) pack: ContextPack,
    pub(crate) inputs: Vec<Resolved>,
}

pub(crate) fn prepare(
    store: &Workspace,
    selection: &Selection,
    profile: &LocalProfile,
) -> Result<Packed, Diagnostic> {
    let inputs = selection
        .inputs
        .iter()
        .map(|s| Resolved::read(store, s))
        .collect::<Result<Vec<_>, _>>()?;
    let resolved: Vec<_> = inputs.iter().map(|input| {
        let length = input.content.len();
        let scope = match input.selector { Selector::Source { .. } => "saved-source", Selector::Capability { .. } => "saved-capability-revision" };
        let selections = vec![serde_json::json!({"startByte":0,"endByte":length,"protected":true})];
        let protected = vec![serde_json::json!({"startByte":0,"endByte":length})];
        serde_json::json!({"input":input.dependency.input,"scope":scope,"content":input.content,"selections":selections,"requiredProtectedRanges":protected})
    }).collect();
    let value = serde_json::json!({
        "schemaVersion":"rangoon.context-pack-request.v1", "task":selection.task,
        "target":{"profileId":profile.profile_id(),"profileSha256":profile.profile_sha256(),"model":profile.model(),"maxOutputTokens":profile.max_output_tokens()},
        "maxBodyBytes":262144, "inputs":resolved
    });
    let mut buffer = Capped(Vec::new());
    serde_json::to_writer(&mut buffer, &value).map_err(|_| Diagnostic::PackOverBudget)?;
    let pack = rangoon_model_assistance::prepare_pack(&buffer.0).map_err(|error| match error {
        rangoon_model_assistance::Diagnostic::PackOverBudget
        | rangoon_model_assistance::Diagnostic::InputLimit => Diagnostic::PackOverBudget,
        _ => Diagnostic::PackInvalid,
    })?;
    let request = PreparedRequest::new(profile, pack.clone()).map_err(|error| match error {
        rangoon_model_local::Diagnostic::RequestOverBudget => Diagnostic::PackOverBudget,
        _ => Diagnostic::PackInvalid,
    })?;
    Ok(Packed {
        request,
        pack,
        inputs,
    })
}

struct Capped(Vec<u8>);
impl Write for Capped {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > (8 * 1024 * 1024usize).saturating_sub(self.0.len()) {
            return Err(std::io::ErrorKind::OutOfMemory.into());
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
