//! Ordinary content editing across source-born and composition-born capabilities.
use super::*;
use composition_records::{Birth, Owner, Records, RevisionRecord};
use rangoon_domain::capability_v1::{
    CapabilityDetail, Revision, RevisionProvenance, RevisionSummary,
};
use rangoon_domain::{
    byte_digest,
    capability::{self, valid_content, valid_id, valid_title},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityReceiptV1 {
    pub capability: CapabilityDetail,
    pub already_applied: bool,
}

fn revision(
    id: &str,
    parent: Option<&str>,
    title: &str,
    content: &str,
) -> Result<Revision, StoreError> {
    Ok(Revision {
        id: capability::revision_id(id, parent, title, content),
        parent_revision_id: parent.map(str::to_owned),
        title: title.into(),
        content: content.into(),
        sha256: byte_digest(content.as_bytes()),
        created_at_ms: capabilities::now_ms()?,
        review: None,
        provenance: RevisionProvenance::Ordinary {},
    })
}
fn check_ids(id: &str, expected: &str) -> Result<(), StoreError> {
    if !valid_id(id, "capability:") || !valid_id(expected, "revision:") {
        return Err(StoreError::CapabilityInvalid);
    }
    Ok(())
}

impl Workspace {
    pub fn create_capability_v1(
        &self,
        source_id: &str,
        fragment_id: &str,
        title: &str,
    ) -> Result<CapabilityReceiptV1, StoreError> {
        if !valid_id(source_id, "source:")
            || !valid_id(fragment_id, "fragment:")
            || !valid_title(title)
        {
            return Err(StoreError::CapabilityInvalid);
        }
        let mut db = self.connect(false)?.ok_or(StoreError::NotFound)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = Records::load(&tx)?;
        let source = current.sources.get(source_id).ok_or(StoreError::NotFound)?;
        let fragment = source
            .report
            .fragments
            .iter()
            .find(|f| f.id == fragment_id)
            .ok_or(StoreError::CapabilityInvalid)?;
        if !valid_content(&fragment.text) {
            return Err(StoreError::CapabilityInvalid);
        }
        let id = capability::capability_id(source_id, fragment_id, title);
        if current.owners.contains_key(&id) {
            let capability = current.detail(&id, None)?;
            tx.commit()?;
            return Ok(CapabilityReceiptV1 {
                capability,
                already_applied: true,
            });
        }
        let next = revision(&id, None, title, &fragment.text)?;
        let birth = Birth::Source {
            source_id: source_id.into(),
            fragment_id: fragment_id.into(),
        };
        let mut candidate = current.clone();
        candidate.version = current.version.max(2);
        candidate.owners.insert(
            id.clone(),
            Owner {
                birth: birth.clone(),
                latest_revision_id: next.id.clone(),
            },
        );
        candidate.revisions.insert(
            next.id.clone(),
            RevisionRecord {
                capability_id: id.clone(),
                revision: next.clone(),
            },
        );
        let candidate = compositions::validate_candidate(&candidate)?;
        composition_backup::ensure_schema(&tx, candidate.version)?;
        composition_backup::insert_owner(&tx, &id, &birth, &next.id)?;
        composition_backup::insert_revision(
            &tx,
            &id,
            &RevisionSummary::from(&next),
            next.content.as_bytes(),
        )?;
        let stored = Records::load(&tx)?;
        if stored.state_id()? != candidate.state_id()? {
            return Err(StoreError::Corrupt);
        }
        let capability = stored.detail(&id, None)?;
        tx.commit()?;
        Ok(CapabilityReceiptV1 {
            capability,
            already_applied: false,
        })
    }

    pub fn revise_capability_v1(
        &self,
        id: &str,
        expected: &str,
        title: &str,
        content: &str,
    ) -> Result<CapabilityReceiptV1, StoreError> {
        check_ids(id, expected)?;
        if !valid_title(title) || !valid_content(content) {
            return Err(StoreError::CapabilityInvalid);
        }
        let mut db = self.connect(false)?.ok_or(StoreError::CapabilityNotFound)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = Records::load(&tx)?;
        let detail = current.detail(id, None)?;
        let retry = capability::revision_id(id, Some(expected), title, content)
            == detail.latest_revision_id;
        if !retry && detail.latest_revision_id != expected {
            return Err(StoreError::CapabilityConflict);
        }
        if retry || (title == detail.revision.title && content == detail.revision.content) {
            tx.commit()?;
            return Ok(CapabilityReceiptV1 {
                capability: detail,
                already_applied: true,
            });
        }
        let next = revision(id, Some(expected), title, content)?;
        let mut candidate = current.clone();
        let owner = candidate
            .owners
            .get_mut(id)
            .ok_or(StoreError::CapabilityNotFound)?;
        owner.latest_revision_id = next.id.clone();
        candidate.revisions.insert(
            next.id.clone(),
            RevisionRecord {
                capability_id: id.into(),
                revision: next.clone(),
            },
        );
        let candidate = compositions::validate_candidate(&candidate)?;
        composition_backup::insert_revision(
            &tx,
            id,
            &RevisionSummary::from(&next),
            content.as_bytes(),
        )?;
        compositions::update_head(&tx, &candidate.owners[id].birth, id, expected, &next.id)?;
        let stored = Records::load(&tx)?;
        if stored.state_id()? != candidate.state_id()? {
            return Err(StoreError::Corrupt);
        }
        let capability = stored.detail(id, None)?;
        tx.commit()?;
        Ok(CapabilityReceiptV1 {
            capability,
            already_applied: false,
        })
    }

    pub fn review_capability_v1(
        &self,
        id: &str,
        expected: &str,
    ) -> Result<CapabilityReceiptV1, StoreError> {
        check_ids(id, expected)?;
        let mut db = self.connect(false)?.ok_or(StoreError::CapabilityNotFound)?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = Records::load(&tx)?;
        let detail = current.detail(id, None)?;
        if detail.latest_revision_id != expected {
            return Err(StoreError::CapabilityConflict);
        }
        let already_applied = detail.revision.review.is_some();
        if !already_applied {
            tx.execute(
                "INSERT INTO reviews VALUES (?1,'local_operator',?2)",
                params![expected, capabilities::now_ms()?],
            )?;
        }
        let capability = Records::load(&tx)?.detail(id, None)?;
        tx.commit()?;
        Ok(CapabilityReceiptV1 {
            capability,
            already_applied,
        })
    }
}
