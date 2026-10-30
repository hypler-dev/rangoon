//! Shared, unmounted native review -> input -> finite read callback.
//! No renderer, secret store or provider discovery participates in this path.
use super::*;
use rangoon_native_credential_input::{InputError, InputGuard, ReadMethod, ReviewDisplay};
use rangoon_openshell_client::OpenShellReadClient;
use std::time::{Duration, Instant};
use tauri::{Runtime, Window};
use zeroize::{Zeroize, Zeroizing};

struct Guard {
    active: Arc<Active>,
    deadline: Instant,
    observer: Option<Observer>,
}

impl InputGuard for Guard {
    fn check(&self) -> Result<(), InputError> {
        self.active
            .accept_credential(self.deadline)
            .map_err(|error| match error {
                ClientError::CredentialLeaseExpired => InputError::Expired,
                ClientError::Cancelled => InputError::Cancelled,
                _ => InputError::Unavailable,
            })
    }

    fn review_queued(&self) {
        notify_phase(self.observer.as_ref(), NativePhase::ReviewQueued);
    }

    fn input_presented(&self) {
        notify_phase(self.observer.as_ref(), NativePhase::InputPresented);
    }
}

// Qualification observers are diagnostics only. A failed observer must not
// unwind across an already queued native callback and release Host custody.
fn notify_phase(observer: Option<&Observer>, phase: NativePhase) {
    if let Some(observer) = observer {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observer(phase)));
    }
}

#[test]
fn diagnostic_observer_failure_does_not_unwind_callback_custody() {
    let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = Arc::clone(&seen);
    let observer: Observer = Arc::new(move |phase| {
        assert_eq!(phase.code(), "native_review_queued");
        count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        panic!("synthetic diagnostic failure");
    });
    notify_phase(None, NativePhase::ReviewQueued);
    notify_phase(Some(&observer), NativePhase::ReviewQueued);
    assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 1);
}

fn map_error(error: InputError) -> ClientError {
    match error {
        InputError::Expired => ClientError::CredentialLeaseExpired,
        InputError::Cancelled | InputError::ParentClosed => ClientError::Cancelled,
        InputError::InvalidInput => ClientError::InvalidConfiguration,
        InputError::Unavailable | InputError::Busy => ClientError::Source,
    }
}

#[derive(Clone, Copy)]
pub(super) enum NativePhase {
    ReviewQueued,
    ReviewAccepted,
    InputQueued,
    InputPresented,
    InputAccepted,
    ReadReturned,
}
impl NativePhase {
    pub(super) fn code(self) -> &'static str {
        match self {
            Self::ReviewQueued => "native_review_queued",
            Self::ReviewAccepted => "native_review_accepted",
            Self::InputQueued => "native_input_queued",
            Self::InputPresented => "native_input_presented",
            Self::InputAccepted => "native_input_accepted",
            Self::ReadReturned => "read_returned",
        }
    }
}
pub(super) type Observer = Arc<dyn Fn(NativePhase) + Send + Sync>;

/// C1-M is limited to the disposable fixture marker. This guard applies to
/// the shared native callback, before a lease or client can be constructed.
/// Removing it for real credentials requires a separately qualified mounting
/// packet. It is not a token parser or an authorization decision.
pub(super) fn validate_fixture_input(value: &mut Zeroizing<String>) -> Result<(), ClientError> {
    if value.as_str() != "test-token" {
        value.zeroize();
        Err(ClientError::InvalidConfiguration)
    } else {
        Ok(())
    }
}

/// The native callback and source regressions share this post-input boundary.
/// Refusal drops the owned input without reaching credential sealing or the
/// reader. This seam does not supply native confirmation or create a claim.
pub(super) fn finish_after_native_input(
    active: &Arc<Active>,
    claim: credential::CredentialClaim,
    mut value: Zeroizing<String>,
    observer: Option<&Observer>,
    read: impl FnOnce(rangoon_openshell_client::BearerLease) -> Result<ReadOutcome, ClientError>,
) -> Result<ReadOutcome, ClientError> {
    let deadline = claim.deadline();
    active.accept_credential(deadline)?;
    notify_phase(observer, NativePhase::InputAccepted);
    validate_fixture_input(&mut value)?;
    let acquired = claim.seal(value, None)?;
    active.bind_credential_deadline(acquired.deadline())?;
    let result = read(acquired.into_lease());
    notify_phase(observer, NativePhase::ReadReturned);
    active.accept_credential(deadline)?;
    result
}

pub(super) fn begin<R: Runtime>(host: &Host, review_id: &str, parent: Window<R>) -> HostStatus {
    begin_inner(host, review_id, parent, None)
}

/// Qualification observes fixed phase boundaries only. It cannot supply
/// confirmation, credentials, results or a replacement custody implementation.
#[cfg(all(test, feature = "native-credential-qualification"))]
pub(super) fn begin_observed<R: Runtime>(
    host: &Host,
    review_id: &str,
    parent: Window<R>,
    observer: Observer,
) -> HostStatus {
    begin_inner(host, review_id, parent, Some(observer))
}

fn begin_inner<R: Runtime>(
    host: &Host,
    review_id: &str,
    parent: Window<R>,
    observer: Option<Observer>,
) -> HostStatus {
    let Some(review) = host
        .inspect_review()
        .filter(|review| review.id == review_id)
    else {
        return HostStatus::Unavailable;
    };
    let label = parent.label().to_owned();
    host.begin_callback(
        review_id,
        None,
        CallbackStart::NativeReview,
        Some(label),
        move |config, kind, control, active, selection| {
            let deadline = selection.deadline;
            active.bind_credential_deadline(deadline)?;
            // Clone only the exact existing flight Arc, never admission, selection
            // or credential custody. Native UI checks this same Host generation.
            let guard = Guard {
                active: Arc::clone(active),
                deadline,
                observer: observer.clone(),
            };
            let guard: Arc<dyn InputGuard> = Arc::new(guard);
            let display = ReviewDisplay {
                origin: review.origin.clone(),
                selected_peer: review.selected_peer.to_string(),
                installation: review.installation.clone(),
                workspace: review.workspace.clone(),
                sandbox: review.sandbox.clone(),
                product_version: review.product_version.to_owned(),
                root_sha256: review.root_sha256.clone(),
                peer_sha256: review.peer_pin_sha256.clone(),
                method: match kind {
                    ReadKind::ProtectedCheck => ReadMethod::ProtectedCheck,
                    ReadKind::PolicyStatus => ReadMethod::PolicyStatus,
                    ReadKind::RecentLogs => ReadMethod::RecentLogs,
                    ReadKind::RecentWatch => ReadMethod::RecentWatch,
                },
            };
            let notify = |phase| notify_phase(observer.as_ref(), phase);
            rangoon_native_credential_input::confirm(parent.clone(), display, Arc::clone(&guard))
                .map_err(map_error)?;
            active.accept_credential(deadline)?;
            notify(NativePhase::ReviewAccepted);
            let claim = selection.claim(); // never consumed before real native confirmation
            notify(NativePhase::InputQueued);
            let value =
                rangoon_native_credential_input::acquire(parent, guard).map_err(map_error)?;
            finish_after_native_input(
                active,
                claim,
                value.into_value(),
                observer.as_ref(),
                |lease| read_once(config, lease, kind, control),
            )
        },
    )
}

pub(super) fn read_once(
    config: ClientConfig,
    lease: rangoon_openshell_client::BearerLease,
    kind: ReadKind,
    control: WatchControl,
) -> Result<ReadOutcome, ClientError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| ClientError::Source)?;
    let result = runtime.block_on(async move {
        let mut client = OpenShellReadClient::connect(config, lease).await?;
        let protected_check = client.check_protected_read().await?;
        let observation = match kind {
            ReadKind::ProtectedCheck => Observation::ProtectedCheck(protected_check.clone()),
            ReadKind::PolicyStatus => {
                Observation::PolicyStatus(client.read_policy_status(0).await?)
            }
            ReadKind::RecentLogs => Observation::RecentLogs(client.read_recent_logs().await?),
            ReadKind::RecentWatch => Observation::RecentWatch(
                client
                    .watch_recent(&control, None, Duration::from_secs(30))
                    .await?,
            ),
        };
        Ok(ReadOutcome {
            protected_check,
            observation,
        })
    });
    drop(runtime);
    result
}
