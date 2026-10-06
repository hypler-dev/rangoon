//! Read-only instruction compilation from validated saved revision records.

use crate::{StoreError, Workspace};
use rangoon_compile::{CompilationReport, Profile, compile};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCompilation {
    /// Head observed when the selected revision was read, not an export grant.
    pub observed_current_head: String,
    pub compilation: CompilationReport,
}

impl Workspace {
    /// Resolve exact saved content and review facts before invoking the compiler.
    /// This never creates, migrates, reviews, or writes workspace records.
    pub fn compile_capability(
        &self,
        capability_id: &str,
        revision_id: &str,
        profile: Profile,
    ) -> Result<WorkspaceCompilation, StoreError> {
        // The existing read path checks both IDs before opening the workspace,
        // validates the complete stored graph, and rejects wrong-owner revisions.
        let detail = self.open_capability_v1(capability_id, Some(revision_id))?;
        let compilation = compile(&detail.id, &detail.revision, profile, &[])
            .map_err(|_| StoreError::CompilationInvalid)?;
        Ok(WorkspaceCompilation {
            observed_current_head: detail.latest_revision_id,
            compilation,
        })
    }
}
