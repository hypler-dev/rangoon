//! Native-owned selection metadata. This is not an OS credential provider.
use super::{ClientError, HostError, Review, review_id};
use std::time::{Duration, Instant};

const MAX_SELECTION_LIFETIME: Duration = Duration::from_secs(120);

/// An inspection copy is only a lookup candidate, never owning custody.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct CredentialInspection {
    pub(super) reference: String,
    pub(super) review: Review,
}

/// Retained only in this Host's Pending state, alongside generation admission.
/// No Clone, Debug, serialization, lookup service, or ambient secret access.
pub(super) struct CredentialSelection {
    inspection: CredentialInspection,
    pub(super) deadline: Instant,
}

impl CredentialSelection {
    pub(super) fn new(review: &Review) -> Result<Self, HostError> {
        let deadline = Instant::now()
            .checked_add(MAX_SELECTION_LIFETIME)
            .ok_or(HostError::Unavailable)?;
        Ok(Self {
            inspection: CredentialInspection {
                reference: review_id()?,
                review: review.clone(),
            },
            deadline,
        })
    }

    pub(super) fn inspect(&self) -> CredentialInspection {
        self.inspection.clone()
    }

    pub(super) fn matches(&self, candidate: &CredentialInspection, review: &Review) -> bool {
        &self.inspection == candidate && &candidate.review == review
    }

    pub(super) fn check(&self) -> Result<(), ClientError> {
        check_deadline(self.deadline)
    }

    pub(super) fn claim(self) -> CredentialClaim {
        CredentialClaim { selection: self }
    }
}

pub(super) fn check_deadline(deadline: Instant) -> Result<(), ClientError> {
    if Instant::now() >= deadline {
        Err(ClientError::CredentialLeaseExpired)
    } else {
        Ok(())
    }
}

/// One-shot native claim. No command, renderer input or ambient source can
/// construct it; the existing Host transfers it after exact native consent.
pub(super) struct CredentialClaim {
    selection: CredentialSelection,
}

impl CredentialClaim {
    pub(super) fn deadline(&self) -> Instant {
        self.selection.deadline
    }

    /// Consumes the exact one-shot claim. The source can shorten its deadline,
    /// never renew it. The unmounted native path uses the same corrected client lease.
    pub(super) fn seal(
        self,
        mut value: zeroize::Zeroizing<String>,
        source_expiry: Option<Instant>,
    ) -> Result<AcquiredCredential, ClientError> {
        let deadline = source_expiry.map_or(self.selection.deadline, |expiry| {
            expiry.min(self.selection.deadline)
        });
        check_deadline(deadline)?;
        if value.is_empty()
            || value.len() > 16 * 1024
            || !value.bytes().all(|b| (0x20..=0x7e).contains(&b))
        {
            return Err(ClientError::Source);
        }
        // Transfer the owned allocation to the existing client's owning lease.
        // This does not promise erasure of
        // every transient allocation inside the client's String-taking API.
        let lease =
            rangoon_openshell_client::BearerLease::new(std::mem::take(&mut *value), deadline)
                .map_err(|_| {
                    check_deadline(deadline)
                        .err()
                        .unwrap_or(ClientError::Source)
                })?;
        Ok(AcquiredCredential { lease, deadline })
    }
}

pub(super) struct AcquiredCredential {
    lease: rangoon_openshell_client::BearerLease,
    deadline: Instant,
}

impl AcquiredCredential {
    pub(super) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(super) fn into_lease(self) -> rangoon_openshell_client::BearerLease {
        self.lease
    }
}

#[cfg(test)]
pub(super) trait CredentialAcquirer: Send + 'static {
    fn acquire(self: Box<Self>, claim: CredentialClaim) -> Result<AcquiredCredential, ClientError>;
}
