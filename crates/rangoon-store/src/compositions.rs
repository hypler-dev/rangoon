//! Read-only composition preview against a validated saved workspace.
use super::*;
use composition_records::Records;
use rangoon_compose::application::{self, Request, Target, TargetHead};
use rangoon_domain::capability::valid_id;
use rangoon_domain::capability_v1::{CapabilityDetail, CapabilitySummary};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionPreview {
    pub expected_state_id: String,
    pub preview: application::ApplicationPreview,
}

impl Workspace {
    pub fn list_capabilities_v1(&self) -> Result<Vec<CapabilitySummary>, StoreError> {
        let Some(mut db) = self.connect(false)? else {
            return Ok(Vec::new());
        };
        let tx = db.transaction()?;
        let records = Records::load(&tx)?;
        let result = records
            .owners
            .keys()
            .map(|id| records.summary(id))
            .collect::<Result<_, _>>()?;
        tx.commit()?;
        Ok(result)
    }

    pub fn open_capability_v1(
        &self,
        id: &str,
        revision: Option<&str>,
    ) -> Result<CapabilityDetail, StoreError> {
        if !valid_id(id, "capability:") || revision.is_some_and(|r| !valid_id(r, "revision:")) {
            return Err(StoreError::CapabilityInvalid);
        }
        let mut db = self.connect(false)?.ok_or(StoreError::CapabilityNotFound)?;
        let tx = db.transaction()?;
        let result = Records::load(&tx)?.detail(id, revision)?;
        tx.commit()?;
        Ok(result)
    }

    /// Preview never creates a workspace, migrates its schema or writes an output.
    /// The native host must bind this result to its own exact-preview confirmation.
    pub fn preview_composition(&self, request: &Request) -> Result<CompositionPreview, StoreError> {
        if request.schema_version != application::REQUEST_SCHEMA {
            return Err(StoreError::CompositionInvalid);
        }
        application::application_identity_envelope_bytes(&request.draft, &request.targets)
            .map_err(|_| StoreError::CompositionInvalid)?;
        let mut db = self.connect(false)?.ok_or(StoreError::NotFound)?;
        let tx = db.transaction()?;
        let records = Records::load(&tx)?;
        let inputs = records.resolve_inputs(&request.draft)?;
        let heads = request
            .targets
            .iter()
            .filter_map(|target| match target {
                Target::New {} => None,
                Target::Append { capability_id, .. } => Some(
                    records
                        .owners
                        .get(capability_id)
                        .map(|owner| TargetHead {
                            capability_id: capability_id.clone(),
                            revision_id: owner.latest_revision_id.clone(),
                        })
                        .ok_or(StoreError::CapabilityNotFound),
                ),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let preview = application::preview(request, &inputs, &heads)
            .map_err(|_| StoreError::CompositionInvalid)?;
        for output in &preview.applied_outputs {
            if matches!(output.kind, application::AppliedKind::New)
                && records.owners.contains_key(&output.capability_id)
            {
                return Err(StoreError::CapabilityConflict);
            }
        }
        let result = CompositionPreview {
            expected_state_id: records.state_id()?,
            preview,
        };
        tx.commit()?;
        Ok(result)
    }
}
