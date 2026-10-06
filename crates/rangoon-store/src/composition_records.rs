//! Validated records for revision-level composition provenance.
use super::*;
use rangoon_compose::{Draft, InputReference, ResolvedInput, application};
use rangoon_domain::{
    Authority, byte_digest,
    capability::{self, ContentReview, Reviewer, valid_content, valid_id, valid_title},
    capability_v1::{self as v1, Origin, OriginSummary, RevisionProvenance},
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) const MAX_COMPOSITIONS: usize = 128;
pub(super) const MAX_APPLICATIONS: usize = 128;
const MAX_DEPTH: usize = 128;
const MAX_TARGET_BYTES: usize = 16 * 1024;

pub(super) const SCHEMAS: &[(&str, &str)] = &[
    (
        "compositions",
        "CREATE TABLE compositions (composition_id TEXT PRIMARY KEY NOT NULL, draft_json BLOB NOT NULL, transformation_version TEXT NOT NULL, created_at_ms INTEGER NOT NULL)",
    ),
    (
        "composition_applications",
        "CREATE TABLE composition_applications (application_id TEXT PRIMARY KEY NOT NULL, composition_id TEXT NOT NULL, targets_json BLOB NOT NULL, acknowledged_by TEXT NOT NULL, created_at_ms INTEGER NOT NULL)",
    ),
    (
        "derived_capabilities",
        "CREATE TABLE derived_capabilities (capability_id TEXT PRIMARY KEY NOT NULL, composition_id TEXT NOT NULL, output_index INTEGER NOT NULL, latest_revision_id TEXT NOT NULL, UNIQUE(composition_id,output_index))",
    ),
    (
        "revision_derivations",
        "CREATE TABLE revision_derivations (revision_id TEXT PRIMARY KEY NOT NULL, application_id TEXT NOT NULL, output_index INTEGER NOT NULL, UNIQUE(application_id,output_index))",
    ),
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(super) enum Birth {
    Source {
        source_id: String,
        fragment_id: String,
    },
    Composition {
        composition_id: String,
        output_index: u32,
    },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Owner {
    pub birth: Birth,
    pub latest_revision_id: String,
}

#[derive(Clone, Debug)]
pub(super) struct SourceRecord {
    pub metadata: SnapshotMetadata,
    pub report: AnalysisReport,
}
#[derive(Clone, Debug)]
pub(super) struct RevisionRecord {
    pub capability_id: String,
    pub revision: v1::Revision,
}
#[derive(Clone, Debug)]
pub(super) struct Recipe {
    pub draft: Draft,
    pub created_at_ms: i64,
}
#[derive(Clone, Debug)]
pub(super) struct Application {
    pub composition_id: String,
    pub targets: Vec<application::Target>,
    pub created_at_ms: i64,
}

/// Internal records loaded with complete row and dependency validation.
/// Recovery candidates must be revalidated before using histories or committing.
#[derive(Clone, Debug)]
pub(super) struct Records {
    pub version: i64,
    pub sources: BTreeMap<String, SourceRecord>,
    pub owners: BTreeMap<String, Owner>,
    pub revisions: BTreeMap<String, RevisionRecord>,
    pub recipes: BTreeMap<String, Recipe>,
    pub applications: BTreeMap<String, Application>,
    histories: BTreeMap<String, Vec<String>>,
}

#[derive(Default)]
struct Validation {
    revisions: BTreeMap<String, usize>,
    recipes: BTreeMap<String, usize>,
    visiting_revisions: BTreeSet<String>,
    visiting_recipes: BTreeSet<String>,
}

impl Records {
    pub fn empty() -> Self {
        Self {
            version: 1,
            sources: BTreeMap::new(),
            owners: BTreeMap::new(),
            revisions: BTreeMap::new(),
            recipes: BTreeMap::new(),
            applications: BTreeMap::new(),
            histories: BTreeMap::new(),
        }
    }

    pub fn load(db: &Connection) -> Result<Self, StoreError> {
        verify_schema(db)?;
        let version = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let mut records = Self {
            version,
            sources: BTreeMap::new(),
            owners: BTreeMap::new(),
            revisions: BTreeMap::new(),
            recipes: BTreeMap::new(),
            applications: BTreeMap::new(),
            histories: BTreeMap::new(),
        };
        for metadata in list_metadata(db)? {
            let report = read_report(db, &metadata.source_id)?;
            records.sources.insert(
                metadata.source_id.clone(),
                SourceRecord { metadata, report },
            );
        }
        if version >= 2 {
            records.load_owners(db)?;
            records.load_revisions(db)?;
        }
        if version == 3 {
            records.load_compositions(db)?;
        }
        records.validate()?;
        Ok(records)
    }

    fn load_owners(&mut self, db: &Connection) -> Result<(), StoreError> {
        check_count(db, "SELECT count(*) FROM capabilities", MAX_CAPABILITIES)?;
        let mut q = db.prepare("SELECT capability_id,source_id,fragment_id,latest_revision_id FROM capabilities ORDER BY capability_id LIMIT 129")?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, source_id, fragment_id, latest_revision_id) in rows {
            self.owners.insert(
                id,
                Owner {
                    birth: Birth::Source {
                        source_id,
                        fragment_id,
                    },
                    latest_revision_id,
                },
            );
        }
        if self.version == 3 {
            check_count(
                db,
                "SELECT count(*) FROM derived_capabilities",
                MAX_CAPABILITIES,
            )?;
            let mut q = db.prepare("SELECT capability_id,composition_id,output_index,latest_revision_id FROM derived_capabilities ORDER BY capability_id LIMIT 129")?;
            let rows = q
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, u32>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (id, composition_id, output_index, latest_revision_id) in rows {
                if self
                    .owners
                    .insert(
                        id,
                        Owner {
                            birth: Birth::Composition {
                                composition_id,
                                output_index,
                            },
                            latest_revision_id,
                        },
                    )
                    .is_some()
                {
                    return Err(StoreError::Corrupt);
                }
            }
        }
        if self.owners.len() > MAX_CAPABILITIES {
            return Err(StoreError::CapabilityFull);
        }
        Ok(())
    }

    fn load_revisions(&mut self, db: &Connection) -> Result<(), StoreError> {
        check_count(db, "SELECT count(*) FROM revisions", MAX_TOTAL_REVISIONS)?;
        check_count(db, "SELECT count(*) FROM reviews", MAX_TOTAL_REVISIONS)?;
        let malformed: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM revisions WHERE length(content)>262144 OR length(title)>160) OR EXISTS(SELECT 1 FROM reviews v LEFT JOIN revisions r ON v.revision_id=r.revision_id WHERE r.revision_id IS NULL)", [], |r| r.get(0))?;
        if malformed {
            return Err(StoreError::Corrupt);
        }
        let mut q = db.prepare("SELECT r.revision_id,r.capability_id,r.parent_revision_id,r.title,r.content,r.sha256,r.created_at_ms,v.reviewer,v.reviewed_at_ms FROM revisions r LEFT JOIN reviews v ON v.revision_id=r.revision_id ORDER BY r.revision_id LIMIT 1025")?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, Vec<u8>>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<i64>>(8)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (
            id,
            capability_id,
            parent_revision_id,
            title,
            bytes,
            sha256,
            created_at_ms,
            reviewer,
            reviewed_at_ms,
        ) in rows
        {
            let content = String::from_utf8(bytes).map_err(|_| StoreError::Corrupt)?;
            if !valid_id(&id, "revision:")
                || !valid_id(&capability_id, "capability:")
                || parent_revision_id
                    .as_ref()
                    .is_some_and(|p| !valid_id(p, "revision:"))
                || !valid_title(&title)
                || !valid_content(&content)
                || !valid_time(created_at_ms)
                || sha256 != byte_digest(content.as_bytes())
            {
                return Err(StoreError::Corrupt);
            }
            let review = match (reviewer.as_deref(), reviewed_at_ms) {
                (None, None) => None,
                (Some("local_operator"), Some(time)) if valid_time(time) => Some(ContentReview {
                    reviewer: Reviewer::LocalOperator,
                    reviewed_at_ms: time,
                }),
                _ => return Err(StoreError::Corrupt),
            };
            self.revisions.insert(
                id.clone(),
                RevisionRecord {
                    capability_id,
                    revision: v1::Revision {
                        id,
                        parent_revision_id,
                        title,
                        content,
                        sha256,
                        created_at_ms,
                        review,
                        provenance: RevisionProvenance::Ordinary {},
                    },
                },
            );
        }
        Ok(())
    }

    fn load_compositions(&mut self, db: &Connection) -> Result<(), StoreError> {
        check_count(db, "SELECT count(*) FROM compositions", MAX_COMPOSITIONS)?;
        check_count(
            db,
            "SELECT count(*) FROM composition_applications",
            MAX_APPLICATIONS,
        )?;
        check_count(
            db,
            "SELECT count(*) FROM revision_derivations",
            MAX_TOTAL_REVISIONS,
        )?;
        let malformed: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM compositions WHERE length(draft_json)>8388608) OR EXISTS(SELECT 1 FROM composition_applications WHERE length(targets_json)>16384)",[],|r|r.get(0))?;
        if malformed {
            return Err(StoreError::Corrupt);
        }
        let mut q = db.prepare("SELECT composition_id,draft_json,transformation_version,created_at_ms FROM compositions ORDER BY composition_id LIMIT 129")?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Vec<u8>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, bytes, version, created_at_ms) in rows {
            let draft =
                rangoon_compose::decode_draft_json(&bytes).map_err(|_| StoreError::Corrupt)?;
            if !valid_time(created_at_ms)
                || version != rangoon_compose::TRANSFORMATION_VERSION
                || rangoon_compose::canonical_draft_bytes(&draft)
                    .map_err(|_| StoreError::Corrupt)?
                    != bytes
                || rangoon_compose::composition_id(&draft).map_err(|_| StoreError::Corrupt)? != id
            {
                return Err(StoreError::Corrupt);
            }
            self.recipes.insert(
                id,
                Recipe {
                    draft,
                    created_at_ms,
                },
            );
        }
        let mut q = db.prepare("SELECT application_id,composition_id,targets_json,acknowledged_by,created_at_ms FROM composition_applications ORDER BY application_id LIMIT 129")?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Vec<u8>>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, composition_id, bytes, acknowledged_by, created_at_ms) in rows {
            let recipe = self
                .recipes
                .get(&composition_id)
                .ok_or(StoreError::Corrupt)?;
            let targets = decode_targets(&bytes)?;
            if acknowledged_by != "local_operator"
                || !valid_time(created_at_ms)
                || application::application_id(&recipe.draft, &targets)
                    .map_err(|_| StoreError::Corrupt)?
                    != id
            {
                return Err(StoreError::Corrupt);
            }
            self.applications.insert(
                id,
                Application {
                    composition_id,
                    targets,
                    created_at_ms,
                },
            );
        }
        let mut q = db.prepare("SELECT revision_id,application_id,output_index FROM revision_derivations ORDER BY revision_id LIMIT 1025")?;
        let rows = q
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, u32>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut memberships = BTreeSet::new();
        for (revision_id, application_id, output_index) in rows {
            let app = self
                .applications
                .get(&application_id)
                .ok_or(StoreError::Corrupt)?;
            if output_index as usize >= app.targets.len()
                || !memberships.insert((application_id.clone(), output_index))
            {
                return Err(StoreError::Corrupt);
            }
            let row = self
                .revisions
                .get_mut(&revision_id)
                .ok_or(StoreError::Corrupt)?;
            row.revision.provenance = RevisionProvenance::Composition {
                application_id,
                composition_id: app.composition_id.clone(),
                output_index,
            };
        }
        if self
            .applications
            .keys()
            .any(|id| !memberships.iter().any(|(app, _)| app == id))
            || self.recipes.keys().any(|id| {
                !self
                    .applications
                    .values()
                    .any(|app| &app.composition_id == id)
            })
        {
            return Err(StoreError::Corrupt);
        }
        Ok(())
    }

    fn validate(&mut self) -> Result<(), StoreError> {
        if self
            .revisions
            .values()
            .any(|r| !self.owners.contains_key(&r.capability_id))
        {
            return Err(StoreError::Corrupt);
        }
        for (id, owner) in &self.owners {
            if !valid_id(id, "capability:") || !valid_id(&owner.latest_revision_id, "revision:") {
                return Err(StoreError::Corrupt);
            }
            let count = self
                .revisions
                .values()
                .filter(|r| &r.capability_id == id)
                .count();
            if count == 0 || count > MAX_REVISIONS {
                return Err(StoreError::Corrupt);
            }
            let mut chain = Vec::new();
            let mut current = Some(owner.latest_revision_id.as_str());
            while let Some(revision_id) = current {
                let row = self.revisions.get(revision_id).ok_or(StoreError::Corrupt)?;
                if &row.capability_id != id || chain.len() >= count {
                    return Err(StoreError::Corrupt);
                }
                chain.push(revision_id.to_owned());
                current = row.revision.parent_revision_id.as_deref();
            }
            if chain.len() != count {
                return Err(StoreError::Corrupt);
            }
            chain.reverse();
            let root = &self.revisions[&chain[0]].revision;
            match &owner.birth {
                Birth::Source {
                    source_id,
                    fragment_id,
                } => {
                    let source = self.sources.get(source_id).ok_or(StoreError::Corrupt)?;
                    let fragment = source
                        .report
                        .fragments
                        .iter()
                        .find(|f| &f.id == fragment_id)
                        .ok_or(StoreError::Corrupt)?;
                    if root.content != fragment.text
                        || capability::capability_id(source_id, fragment_id, &root.title) != *id
                        || !matches!(root.provenance, RevisionProvenance::Ordinary {})
                    {
                        return Err(StoreError::Corrupt);
                    }
                }
                Birth::Composition {
                    composition_id,
                    output_index,
                } => {
                    if !self.recipes.contains_key(composition_id)
                        || rangoon_compose::composed_capability_id(composition_id, *output_index)
                            != *id
                        || !matches!(&root.provenance,RevisionProvenance::Composition {composition_id:c,output_index:i,..} if c==composition_id && i==output_index)
                    {
                        return Err(StoreError::Corrupt);
                    }
                }
            }
            self.histories.insert(id.clone(), chain);
        }
        let mut validation = Validation::default();
        for id in self.revisions.keys() {
            self.validate_revision(id, &mut validation, 0)?;
        }
        Ok(())
    }

    fn validate_revision(
        &self,
        id: &str,
        work: &mut Validation,
        depth: usize,
    ) -> Result<usize, StoreError> {
        if let Some(height) = work.revisions.get(id) {
            return if depth + height <= MAX_DEPTH {
                Ok(*height)
            } else {
                Err(StoreError::Corrupt)
            };
        }
        if depth > MAX_DEPTH || !work.visiting_revisions.insert(id.into()) {
            return Err(StoreError::Corrupt);
        }
        let row = self.revisions.get(id).ok_or(StoreError::Corrupt)?;
        let revision = &row.revision;
        let mut height = 0;
        if let Some(parent) = &revision.parent_revision_id {
            height = self.validate_revision(parent, work, depth + 1)? + 1;
        }
        match &revision.provenance {
            RevisionProvenance::Ordinary {} => {
                if revision.id
                    != capability::revision_id(
                        &row.capability_id,
                        revision.parent_revision_id.as_deref(),
                        &revision.title,
                        &revision.content,
                    )
                {
                    return Err(StoreError::Corrupt);
                }
            }
            RevisionProvenance::Composition { composition_id, .. } => {
                height = height.max(self.validate_recipe(composition_id, work, depth + 1)? + 1);
            }
        }
        work.visiting_revisions.remove(id);
        work.revisions.insert(id.into(), height);
        Ok(height)
    }

    fn validate_recipe(
        &self,
        id: &str,
        work: &mut Validation,
        depth: usize,
    ) -> Result<usize, StoreError> {
        if let Some(height) = work.recipes.get(id) {
            return if depth + height <= MAX_DEPTH {
                Ok(*height)
            } else {
                Err(StoreError::Corrupt)
            };
        }
        if depth > MAX_DEPTH || !work.visiting_recipes.insert(id.into()) {
            return Err(StoreError::Corrupt);
        }
        let recipe = self.recipes.get(id).ok_or(StoreError::Corrupt)?;
        // Resolve dependencies before allocating materialization buffers. Only one
        // recipe preview remains resident. Cache dependency height as well as
        // identity so a later longer path cannot bypass the depth bound.
        let mut height = 0;
        for input in &recipe.draft.inputs {
            if let InputReference::Revision { revision_id, .. } = input {
                height = height.max(self.validate_revision(revision_id, work, depth + 1)? + 1);
            }
        }
        let inputs = self.resolve_inputs(&recipe.draft)?;
        let preview =
            rangoon_compose::preview(&recipe.draft, &inputs).map_err(|_| StoreError::Corrupt)?;
        if !preview.saveable || preview.composition_id != id {
            return Err(StoreError::Corrupt);
        }
        for row in self.revisions.values() {
            let RevisionProvenance::Composition {
                application_id,
                composition_id,
                output_index,
            } = &row.revision.provenance
            else {
                continue;
            };
            if composition_id != id {
                continue;
            }
            let app = self
                .applications
                .get(application_id)
                .ok_or(StoreError::Corrupt)?;
            let output = preview
                .outputs
                .get(*output_index as usize)
                .ok_or(StoreError::Corrupt)?;
            if app.composition_id != id
                || output.title != row.revision.title
                || output.content != row.revision.content
            {
                return Err(StoreError::Corrupt);
            }
            let expected = match app
                .targets
                .get(*output_index as usize)
                .ok_or(StoreError::Corrupt)?
            {
                application::Target::New {} => {
                    if row.capability_id != output.capability_id
                        || row.revision.parent_revision_id.is_some()
                        || !matches!(&self.owners[&row.capability_id].birth,Birth::Composition {composition_id:c,output_index:i} if c==id && i==output_index)
                    {
                        return Err(StoreError::Corrupt);
                    }
                    output.revision_id.clone()
                }
                application::Target::Append {
                    capability_id,
                    expected_revision_id,
                } => {
                    if &row.capability_id != capability_id
                        || row.revision.parent_revision_id.as_ref() != Some(expected_revision_id)
                    {
                        return Err(StoreError::Corrupt);
                    }
                    application::composition_revision_id(
                        capability_id,
                        expected_revision_id,
                        application_id,
                        *output_index,
                        &output.title,
                        &output.content,
                    )
                    .map_err(|_| StoreError::Corrupt)?
                }
            };
            if expected != row.revision.id {
                return Err(StoreError::Corrupt);
            }
        }
        work.visiting_recipes.remove(id);
        work.recipes.insert(id.into(), height);
        Ok(height)
    }

    pub fn resolve_inputs(&self, draft: &Draft) -> Result<Vec<ResolvedInput>, StoreError> {
        draft
            .inputs
            .iter()
            .map(|input| match input {
                InputReference::Source { source_id, sha256 } => {
                    let source = &self
                        .sources
                        .get(source_id)
                        .ok_or(StoreError::NotFound)?
                        .report
                        .source;
                    if &source.sha256 != sha256 {
                        return Err(StoreError::Corrupt);
                    }
                    Ok(ResolvedInput::Source {
                        source_id: source_id.clone(),
                        sha256: sha256.clone(),
                        content: source.content.clone(),
                    })
                }
                InputReference::Revision {
                    capability_id,
                    revision_id,
                    sha256,
                } => {
                    let row = self
                        .revisions
                        .get(revision_id)
                        .ok_or(StoreError::CapabilityNotFound)?;
                    if &row.capability_id != capability_id || &row.revision.sha256 != sha256 {
                        return Err(StoreError::Corrupt);
                    }
                    Ok(ResolvedInput::Revision {
                        capability_id: capability_id.clone(),
                        revision_id: revision_id.clone(),
                        sha256: sha256.clone(),
                        content: row.revision.content.clone(),
                    })
                }
            })
            .collect()
    }

    pub fn detail(
        &self,
        id: &str,
        selected: Option<&str>,
    ) -> Result<v1::CapabilityDetail, StoreError> {
        let owner = self.owners.get(id).ok_or(StoreError::CapabilityNotFound)?;
        let row = self
            .revisions
            .get(selected.unwrap_or(&owner.latest_revision_id))
            .ok_or(StoreError::CapabilityNotFound)?;
        if row.capability_id != id {
            return Err(StoreError::CapabilityNotFound);
        }
        let origin = match &owner.birth {
            Birth::Source {
                source_id,
                fragment_id,
            } => {
                let source = &self.sources[source_id].report;
                let fragment = source
                    .fragments
                    .iter()
                    .find(|f| &f.id == fragment_id)
                    .ok_or(StoreError::Corrupt)?;
                Origin::Source {
                    source_id: source_id.clone(),
                    fragment_id: fragment_id.clone(),
                    source_name: source.source.display_name.clone(),
                    span: fragment.span,
                    original_text: fragment.text.clone(),
                }
            }
            Birth::Composition {
                composition_id,
                output_index,
            } => {
                let recipe = &self.recipes[composition_id];
                Origin::Composition {
                    composition_id: composition_id.clone(),
                    operation: recipe.draft.operation.clone(),
                    output_index: *output_index,
                    inputs: recipe.draft.inputs.clone(),
                }
            }
        };
        Ok(v1::CapabilityDetail {
            schema_version: v1::SCHEMA_VERSION.into(),
            id: id.into(),
            origin,
            latest_revision_id: owner.latest_revision_id.clone(),
            revision: row.revision.clone(),
            history: self.histories[id]
                .iter()
                .map(|r| v1::RevisionSummary::from(&self.revisions[r].revision))
                .collect(),
            authority: Authority::None,
        })
    }

    pub fn summary(&self, id: &str) -> Result<v1::CapabilitySummary, StoreError> {
        let owner = self.owners.get(id).ok_or(StoreError::CapabilityNotFound)?;
        let revision = &self.revisions[&owner.latest_revision_id].revision;
        let origin = match &owner.birth {
            Birth::Source {
                source_id,
                fragment_id,
            } => OriginSummary::Source {
                source_id: source_id.clone(),
                fragment_id: fragment_id.clone(),
            },
            Birth::Composition {
                composition_id,
                output_index,
            } => OriginSummary::Composition {
                composition_id: composition_id.clone(),
                operation: self.recipes[composition_id].draft.operation.clone(),
                output_index: *output_index,
            },
        };
        Ok(v1::CapabilitySummary {
            id: id.into(),
            origin,
            latest_revision_id: owner.latest_revision_id.clone(),
            title: revision.title.clone(),
            reviewed: revision.review.is_some(),
            revision_count: self.histories[id].len() as u32,
        })
    }

    pub fn state_id(&self) -> Result<String, StoreError> {
        let sources: Vec<_> = self.sources.values().map(|s| &s.metadata).collect();
        let revisions: BTreeMap<_, _> = self
            .revisions
            .iter()
            .map(|(id, r)| {
                (
                    id,
                    (&r.capability_id, v1::RevisionSummary::from(&r.revision)),
                )
            })
            .collect();
        let recipes: BTreeMap<_, _> = self
            .recipes
            .iter()
            .map(|(id, r)| (id, r.created_at_ms))
            .collect();
        let applications: BTreeMap<_, _> = self
            .applications
            .iter()
            .map(|(id, a)| (id, (&a.composition_id, a.created_at_ms)))
            .collect();
        // Validated content and canonical recipes/targets are bound by their IDs.
        // This is a stale-state digest, never an authentication seal.
        let bytes = serde_json::to_vec(&(
            "rangoon.workspace-state.v1",
            self.version,
            sources,
            &self.owners,
            revisions,
            recipes,
            applications,
        ))
        .map_err(|_| StoreError::Corrupt)?;
        Ok(format!("workspace:{}", byte_digest(&bytes)))
    }
}

fn valid_time(time: i64) -> bool {
    (0..=8_640_000_000_000_000).contains(&time)
}
fn check_count(db: &Connection, sql: &str, maximum: usize) -> Result<(), StoreError> {
    let count: i64 = db.query_row(sql, [], |r| r.get(0))?;
    if count < 0 || count as u64 > maximum as u64 {
        Err(StoreError::Corrupt)
    } else {
        Ok(())
    }
}

fn decode_targets(bytes: &[u8]) -> Result<Vec<application::Target>, StoreError> {
    if bytes.len() > MAX_TARGET_BYTES {
        return Err(StoreError::Corrupt);
    }
    struct Targets;
    impl<'de> serde::de::Visitor<'de> for Targets {
        type Value = Vec<application::Target>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("bounded composition targets")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let mut targets = Vec::new();
            while targets.len() < rangoon_compose::MAX_OUTPUTS {
                match seq.next_element()? {
                    Some(target) => targets.push(target),
                    None => return Ok(targets),
                }
            }
            if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom("target limit"));
            }
            Ok(targets)
        }
    }
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    let targets = serde::Deserializer::deserialize_seq(&mut decoder, Targets)
        .map_err(|_| StoreError::Corrupt)?;
    decoder.end().map_err(|_| StoreError::Corrupt)?;
    if serde_json::to_vec(&targets).map_err(|_| StoreError::Corrupt)? != bytes {
        return Err(StoreError::Corrupt);
    }
    Ok(targets)
}
