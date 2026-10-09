//! Prepared composition previews and atomic application to saved capabilities.
use super::*;
use composition_records::{Application, Birth, Owner, Recipe, Records, RevisionRecord};
use rangoon_compose::application::{self, Request, Target, TargetHead};
use rangoon_domain::capability_v1::{
    CapabilityDetail, CapabilitySummary, Revision, RevisionProvenance, RevisionSummary,
};
use rangoon_domain::{Authority, capability::valid_id};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionPreview {
    pub expected_state_id: String,
    pub preview: application::ApplicationPreview,
    #[serde(skip)]
    request: Request,
    #[serde(skip)]
    bound_state_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositionReceipt {
    pub schema_version: &'static str,
    pub composition_id: String,
    pub application_id: String,
    pub committed_state_id: String,
    pub capabilities: Vec<CapabilityDetail>,
    pub authority: Authority,
}

fn checked_preview(
    records: &Records,
    request: &Request,
) -> Result<application::ApplicationPreview, StoreError> {
    let inputs = records.resolve_inputs(&request.draft)?;
    let heads = request
        .targets
        .iter()
        .filter_map(|target| match target {
            Target::New {} => None,
            Target::Append {
                capability_id,
                expected_revision_id,
            } => Some(
                records
                    .owners
                    .get(capability_id)
                    .ok_or(StoreError::CapabilityNotFound)
                    .and_then(|owner| {
                        if &owner.latest_revision_id != expected_revision_id {
                            return Err(StoreError::CapabilityConflict);
                        }
                        Ok(TargetHead {
                            capability_id: capability_id.clone(),
                            revision_id: owner.latest_revision_id.clone(),
                        })
                    }),
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
    Ok(preview)
}

fn candidate(
    records: &Records,
    request: &Request,
    preview: &application::ApplicationPreview,
    time: i64,
) -> Result<Records, StoreError> {
    if !preview.saveable {
        return Err(StoreError::CompositionInvalid);
    }
    let mut result = records.clone();
    let composition_id = &preview.core.composition_id;
    match result.recipes.get(composition_id) {
        Some(existing) if existing.draft != request.draft => return Err(StoreError::Corrupt),
        Some(_) => (),
        None => {
            result.recipes.insert(
                composition_id.clone(),
                Recipe {
                    draft: request.draft.clone(),
                    created_at_ms: time,
                },
            );
        }
    }
    match result.applications.get(&preview.application_id) {
        Some(existing)
            if existing.composition_id != *composition_id
                || existing.targets != request.targets =>
        {
            return Err(StoreError::Corrupt);
        }
        Some(_) => (),
        None => {
            result.applications.insert(
                preview.application_id.clone(),
                Application {
                    composition_id: composition_id.clone(),
                    targets: request.targets.clone(),
                    created_at_ms: time,
                },
            );
        }
    }
    for (output, content) in preview.applied_outputs.iter().zip(&preview.core.outputs) {
        let birth = match output.kind {
            application::AppliedKind::New => Birth::Composition {
                composition_id: composition_id.clone(),
                output_index: output.output_index,
            },
            application::AppliedKind::Append => result
                .owners
                .get(&output.capability_id)
                .ok_or(StoreError::CapabilityNotFound)?
                .birth
                .clone(),
        };
        result.owners.insert(
            output.capability_id.clone(),
            Owner {
                birth,
                latest_revision_id: output.revision_id.clone(),
            },
        );
        let revision = Revision {
            id: output.revision_id.clone(),
            parent_revision_id: output.parent_revision_id.clone(),
            title: content.title.clone(),
            content: content.content.clone(),
            sha256: content.sha256.clone(),
            created_at_ms: time,
            review: None,
            provenance: RevisionProvenance::Composition {
                application_id: preview.application_id.clone(),
                composition_id: composition_id.clone(),
                output_index: output.output_index,
            },
        };
        if result
            .revisions
            .insert(
                revision.id.clone(),
                RevisionRecord {
                    capability_id: output.capability_id.clone(),
                    revision,
                },
            )
            .is_some()
        {
            return Err(StoreError::CapabilityConflict);
        }
    }
    result.version = records.version.max(3);
    validate_candidate(&result)
}

pub(super) fn validate_candidate(records: &Records) -> Result<Records, StoreError> {
    if records.owners.len() > MAX_CAPABILITIES
        || records.revisions.len() > MAX_TOTAL_REVISIONS
        || records.recipes.len() > 128
        || records.applications.len() > 128
    {
        return Err(StoreError::CapabilityFull);
    }
    for id in records.owners.keys() {
        if records
            .revisions
            .values()
            .filter(|r| &r.capability_id == id)
            .count()
            > MAX_REVISIONS
        {
            return Err(StoreError::CapabilityFull);
        }
    }
    Records::load_complete(&composition_backup::canonical_database(records)?)
}

pub(super) fn update_head(
    db: &Connection,
    birth: &Birth,
    id: &str,
    expected: &str,
    next: &str,
) -> Result<(), StoreError> {
    let sql = match birth {
        Birth::Source { .. } => {
            "UPDATE capabilities SET latest_revision_id=?1 WHERE capability_id=?2 AND latest_revision_id=?3"
        }
        Birth::Composition { .. } => {
            "UPDATE derived_capabilities SET latest_revision_id=?1 WHERE capability_id=?2 AND latest_revision_id=?3"
        }
    };
    if db.execute(sql, params![next, id, expected])? != 1 {
        return Err(StoreError::CapabilityConflict);
    }
    Ok(())
}

impl Workspace {
    pub fn list_capabilities_v1(&self) -> Result<Vec<CapabilitySummary>, StoreError> {
        let Some(mut db) = self.connect_complete(false)? else {
            return Ok(Vec::new());
        };
        let tx = db.transaction()?;
        let records = Records::load_complete(&tx)?;
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
        let mut db = self
            .connect_complete(false)?
            .ok_or(StoreError::CapabilityNotFound)?;
        let tx = db.transaction()?;
        let result = Records::load_complete(&tx)?.detail(id, revision)?;
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
        let mut db = self.connect_complete(false)?.ok_or(StoreError::NotFound)?;
        let tx = db.transaction()?;
        let records = Records::load_complete(&tx)?;
        let bytes = rangoon_compose::canonical_draft_bytes(&request.draft)
            .map_err(|_| StoreError::CompositionInvalid)?;
        let request = Request {
            schema_version: request.schema_version.clone(),
            draft: serde_json::from_slice(&bytes).map_err(|_| StoreError::CompositionInvalid)?,
            targets: request.targets.clone(),
        };
        let preview = checked_preview(&records, &request)?;
        if preview.saveable {
            candidate(&records, &request, &preview, 0)?;
        }
        let bound_state_id = records.state_id()?;
        let result = CompositionPreview {
            expected_state_id: bound_state_id.clone(),
            bound_state_id,
            preview,
            request,
        };
        tx.commit()?;
        Ok(result)
    }

    /// Accept only a retained store-issued preview. The native host must additionally
    /// bind it to an opaque per-selection handle and the user's exact acknowledgment.
    pub fn apply_composition(
        &self,
        prepared: &CompositionPreview,
        acknowledged: bool,
    ) -> Result<CompositionReceipt, StoreError> {
        if !acknowledged {
            return Err(StoreError::CompositionAcknowledgmentRequired);
        }
        // The public display ID cannot rebind the private issuance state.
        if prepared.expected_state_id != prepared.bound_state_id {
            return Err(StoreError::CompositionInvalid);
        }
        let mut db = self
            .connect_complete(false)?
            .ok_or(StoreError::WorkspaceChanged)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = Records::load_complete(&tx)?;
        if current.state_id()? != prepared.bound_state_id {
            return Err(StoreError::WorkspaceChanged);
        }
        let preview = checked_preview(&current, &prepared.request)?;
        if preview != prepared.preview {
            return Err(StoreError::CompositionInvalid);
        }
        let result = candidate(
            &current,
            &prepared.request,
            &preview,
            capabilities::now_ms()?,
        )?;
        #[cfg(test)]
        composition_commit_tests::fault_checkpoint(&tx, "before_migration")?;
        workflow_mutations::ensure_schema(&tx, result.version)?;
        #[cfg(test)]
        composition_commit_tests::fault_checkpoint(&tx, "after_migration")?;
        if !current.recipes.contains_key(&preview.core.composition_id) {
            composition_backup::insert_recipe(
                &tx,
                &preview.core.composition_id,
                &result.recipes[&preview.core.composition_id],
            )?;
        }
        if !current.applications.contains_key(&preview.application_id) {
            let app = &result.applications[&preview.application_id];
            composition_backup::insert_application(
                &tx,
                &preview.application_id,
                &app.composition_id,
                &app.targets,
                app.created_at_ms,
            )?;
        }
        for output in &preview.applied_outputs {
            let owner = &result.owners[&output.capability_id];
            if let Some(parent) = &output.parent_revision_id {
                update_head(
                    &tx,
                    &owner.birth,
                    &output.capability_id,
                    parent,
                    &output.revision_id,
                )?;
            } else {
                composition_backup::insert_owner(
                    &tx,
                    &output.capability_id,
                    &owner.birth,
                    &output.revision_id,
                )?;
            }
            let revision = &result.revisions[&output.revision_id].revision;
            composition_backup::insert_revision(
                &tx,
                &output.capability_id,
                &RevisionSummary::from(revision),
                revision.content.as_bytes(),
            )?;
            #[cfg(test)]
            composition_commit_tests::fault_checkpoint(&tx, "after_output")?;
        }
        let stored = Records::load_complete(&tx)?;
        if stored.state_id()? != result.state_id()? {
            return Err(StoreError::Corrupt);
        }
        let receipt = CompositionReceipt {
            schema_version: "rangoon.composition-receipt.v0",
            composition_id: preview.core.composition_id,
            application_id: preview.application_id,
            committed_state_id: stored.state_id()?,
            capabilities: preview
                .applied_outputs
                .iter()
                .map(|o| stored.detail(&o.capability_id, None))
                .collect::<Result<_, _>>()?,
            authority: Authority::None,
        };
        tx.commit()?;
        Ok(receipt)
    }
}
