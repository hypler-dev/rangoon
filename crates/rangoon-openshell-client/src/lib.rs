//! Bounded, read-only OpenShell v0.1.4 transport.
//!
//! This crate is intentionally unmounted. It accepts one explicit HTTPS
//! destination, a caller-supplied short-lived bearer lease, a named workspace
//! and one sandbox. It has no discovery, environment fallback, proxy, redirect,
//! token-refresh, filesystem, process, LNSAT, persistence, or write-RPC path.

#![forbid(unsafe_code)]

use std::{
    error::Error,
    fmt,
    future::Future,
    net::{IpAddr, SocketAddr},
    pin::Pin,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use http::Uri;
use hyper_util::rt::TokioIo;
use rustls::{
    ClientConfig as RustlsClientConfig, DigitallySignedStruct, Error as RustlsError, RootCertStore,
    SignatureScheme,
    client::{
        WebPkiServerVerifier,
        danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    },
    pki_types::{CertificateDer, ServerName, UnixTime},
};
use sha2::{Digest, Sha256};
use tokio::{net::TcpStream, sync::Notify};
use tokio_rustls::{TlsConnector, client::TlsStream};
use tonic::{
    Code, Request, Status,
    transport::{Channel, Endpoint},
};
use tower::Service;
use zeroize::{Zeroize, Zeroizing};

// These private, generated bindings reflect the pinned provider protocol and
// are not project-authored API. Keep Clippy suppression scoped to this module.
#[allow(dead_code, clippy::all)]
mod generated {
    pub(crate) mod datamodel {
        pub(crate) mod v1 {
            include!(concat!(env!("OUT_DIR"), "/openshell.datamodel.v1.rs"));
        }
    }

    pub(crate) mod options {
        pub(crate) mod v1 {
            include!(concat!(env!("OUT_DIR"), "/openshell.options.v1.rs"));
        }
    }

    pub(crate) mod sandbox {
        pub(crate) mod v1 {
            include!(concat!(env!("OUT_DIR"), "/openshell.sandbox.v1.rs"));
        }
    }

    pub(crate) mod v1 {
        include!(concat!(env!("OUT_DIR"), "/openshell.v1.rs"));
    }
}

const MAX_IDENTIFIER_BYTES: usize = 128;
const MAX_CURSOR_BYTES: usize = 512;
const MAX_PAGE_SIZE: i32 = 64;
const MAX_LOG_LINES: u32 = 64;
const MAX_WATCH_ITEMS: usize = 128;
const MAX_MESSAGE_BYTES: usize = 256 * 1024;
const MAX_WINDOW_BYTES: usize = 64 * 1024;
const MAX_WATCH_DURATION: Duration = Duration::from_secs(30);
const RPC_DEADLINE: Duration = Duration::from_secs(5);
const OPEN_SHELL_VERSION: &str = "v0.1.4";

/// Fixed client outcome. `Display` intentionally contains no provider text,
/// origin, credential, raw request, TLS material, or response payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientError {
    InvalidConfiguration,
    CredentialLeaseExpired,
    Unreachable,
    Tls,
    Unauthenticated,
    PermissionDenied,
    Unsupported,
    Cancelled,
    Stale,
    DeadlineExceeded,
    InvalidCursor,
    Gap,
    Bounds,
    Source,
}

impl ClientError {
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidConfiguration => "invalid_configuration",
            Self::CredentialLeaseExpired => "credential_lease_expired",
            Self::Unreachable => "unreachable",
            Self::Tls => "tls",
            Self::Unauthenticated => "unauthenticated",
            Self::PermissionDenied => "permission_denied",
            Self::Unsupported => "unsupported",
            Self::Cancelled => "cancelled",
            Self::Stale => "stale",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::InvalidCursor => "invalid_cursor",
            Self::Gap => "gap",
            Self::Bounds => "bounds",
            Self::Source => "source_error",
        }
    }
}

impl fmt::Display for ClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl Error for ClientError {}

/// Exact TLS certificate pin. The caller must derive this from explicitly
/// selected trust material; this crate never fetches or rotates it.
#[derive(Clone, Eq, PartialEq)]
pub struct CertificatePin([u8; 32]);

impl CertificatePin {
    pub fn from_certificate_der(certificate_der: &[u8]) -> Result<Self, ClientError> {
        if certificate_der.is_empty() || certificate_der.len() > MAX_MESSAGE_BYTES {
            return Err(ClientError::InvalidConfiguration);
        }
        Ok(Self(Sha256::digest(certificate_der).into()))
    }

    pub fn from_sha256_hex(value: &str) -> Result<Self, ClientError> {
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(ClientError::InvalidConfiguration);
        }
        let mut pin = [0_u8; 32];
        for (index, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
            pin[index] = hex_byte(pair).ok_or(ClientError::InvalidConfiguration)?;
        }
        Ok(Self(pin))
    }

    fn matches(&self, certificate_der: &[u8]) -> bool {
        self.0.as_slice() == Sha256::digest(certificate_der).as_slice()
    }
}

impl fmt::Debug for CertificatePin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CertificatePin([redacted])")
    }
}

fn hex_byte(pair: &[u8]) -> Option<u8> {
    let digit = |byte: u8| match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    };
    Some((digit(pair.first().copied()?)? << 4) | digit(pair.get(1).copied()?)?)
}

/// Caller-selected, short-lived bearer credential. It cannot refresh itself;
/// Debug is redacted and no credential appears in an error or projection.
pub struct BearerLease {
    value: Zeroizing<String>,
    expires_at: Instant,
}

impl BearerLease {
    pub fn new(value: String, expires_at: Instant) -> Result<Self, ClientError> {
        // Establish cleanup custody before validation, including error and
        // unwind paths. No credential clone is needed for this transfer.
        let mut value = Zeroizing::new(value);
        Self::validate_owned_value(&mut value, expires_at)?;
        Ok(Self { value, expires_at })
    }

    fn validate_owned_value(
        value: &mut Zeroizing<String>,
        expires_at: Instant,
    ) -> Result<(), ClientError> {
        if value.is_empty() || value.len() > 16 * 1024 || expires_at <= Instant::now() {
            // Clear before returning the fixed error, even if a trusted caller
            // retains the guard. Its Drop remains cleanup defense in depth.
            value.zeroize();
            return Err(ClientError::InvalidConfiguration);
        }
        Ok(())
    }

    fn deadline(&self) -> LeaseDeadline {
        LeaseDeadline(self.expires_at)
    }

    fn authorization_value(
        &self,
    ) -> Result<tonic::metadata::MetadataValue<tonic::metadata::Ascii>, ClientError> {
        self.deadline().check(None)?;
        let authorization = Zeroizing::new(format!("Bearer {}", self.value.as_str()));
        authorization
            .parse()
            .map_err(|_| ClientError::InvalidConfiguration)
    }
}

impl fmt::Debug for BearerLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BearerLease([redacted])")
    }
}

/// Explicit local configuration. A selected root validates the certificate
/// chain, while the separate leaf pin rejects replacement by another otherwise
/// valid certificate for the same hostname.
pub struct ClientConfig {
    origin: String,
    host: String,
    selected_peer: SocketAddr,
    installation: String,
    workspace: String,
    sandbox: String,
    expected_product_version: String,
    trusted_root_certificate_der: Vec<u8>,
    peer_pin: CertificatePin,
}

impl ClientConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        origin: String,
        selected_peer: SocketAddr,
        installation: String,
        workspace: String,
        sandbox: String,
        expected_product_version: String,
        trusted_root_certificate_der: Vec<u8>,
        peer_pin: CertificatePin,
    ) -> Result<Self, ClientError> {
        let uri = Uri::from_str(&origin).map_err(|_| ClientError::InvalidConfiguration)?;
        if uri.scheme_str() != Some("https")
            || uri
                .path_and_query()
                .is_some_and(|path| path.as_str() != "/")
            || uri.query().is_some()
            || uri.authority().is_none()
            || uri
                .authority()
                .is_some_and(|authority| authority.as_str().contains('@'))
        {
            return Err(ClientError::InvalidConfiguration);
        }
        let host = uri
            .host()
            .ok_or(ClientError::InvalidConfiguration)?
            .to_owned();
        if host.is_empty() || IpAddr::from_str(&host).is_ok() {
            return Err(ClientError::InvalidConfiguration);
        }
        if selected_peer.port() != uri.port_u16().unwrap_or(443) {
            return Err(ClientError::InvalidConfiguration);
        }
        validate_identifier(&installation)?;
        validate_identifier(&workspace)?;
        validate_identifier(&sandbox)?;
        if expected_product_version != OPEN_SHELL_VERSION {
            return Err(ClientError::InvalidConfiguration);
        }
        if trusted_root_certificate_der.is_empty()
            || trusted_root_certificate_der.len() > MAX_MESSAGE_BYTES
        {
            return Err(ClientError::InvalidConfiguration);
        }
        Ok(Self {
            origin,
            host,
            selected_peer,
            installation,
            workspace,
            sandbox,
            expected_product_version,
            trusted_root_certificate_der,
            peer_pin,
        })
    }
}

impl fmt::Debug for ClientConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ClientConfig")
            .field("installation", &self.installation)
            .field("workspace", &self.workspace)
            .field("sandbox", &self.sandbox)
            .field("expected_product_version", &self.expected_product_version)
            .finish_non_exhaustive()
    }
}

fn validate_identifier(value: &str) -> Result<(), ClientError> {
    if value.is_empty()
        || value.len() > MAX_IDENTIFIER_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b':' | b'-'))
    {
        return Err(ClientError::InvalidConfiguration);
    }
    Ok(())
}

fn validate_cursor(value: &str) -> Result<(), ClientError> {
    if value.len() > MAX_CURSOR_BYTES || value.chars().any(char::is_control) {
        return Err(ClientError::InvalidCursor);
    }
    Ok(())
}

/// Source authenticity remains unqualified until the separate ingress and
/// host-custody packets qualify it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Authenticity {
    Unqualified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyLoadStatus {
    Unspecified,
    Pending,
    Loaded,
    Failed,
    Superseded,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HealthStatus {
    Unspecified,
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Loss {
    PolicyHashMissing,
    PolicyHashInvalid,
    PolicyBodyRedacted,
    PolicyLoadErrorRedacted,
    PolicyProvenanceRedacted,
    LogTextRedacted,
    LogFieldsRedacted,
    PlatformTextRedacted,
    ProducerInferredFromLegacyEmpty,
    ProducerUnrecognized,
    MissingNativeEventId,
    StreamWarning,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Continuity {
    Bounded,
    Gap,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyRevision {
    pub version: u32,
    pub policy_hash: Option<String>,
    pub status: PolicyLoadStatus,
    pub created_time_millis: Option<i64>,
    pub loaded_time_millis: Option<i64>,
    pub losses: Vec<Loss>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyRead {
    pub authenticity: Authenticity,
    pub installation: String,
    pub workspace: String,
    pub sandbox: String,
    pub active_version: u32,
    pub queried: PolicyRevision,
    pub revisions: Vec<PolicyRevision>,
    pub received_time_millis: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionCheck {
    pub authenticity: Authenticity,
    pub installation: String,
    pub workspace: String,
    pub sandbox: String,
    pub reported_product_version: String,
    pub health: HealthStatus,
    pub protected_read: PolicyRevision,
    pub received_time_millis: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogWindow {
    pub authenticity: Authenticity,
    pub installation: String,
    pub workspace: String,
    pub sandbox: String,
    pub line_count: u32,
    pub buffer_total: u32,
    pub native_sandbox_id: Option<String>,
    pub source_times_millis: Vec<Option<i64>>,
    /// Aligned with `source_times_millis`; never contains raw provider text.
    pub producers: Vec<ProducerKind>,
    pub received_time_millis: i64,
    pub losses: Vec<Loss>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchEvent {
    pub kind: WatchEventKind,
    pub producer: Option<ProducerKind>,
    pub cursor: Option<String>,
    pub native_sandbox_id: Option<String>,
    pub source_time_millis: Option<i64>,
    pub received_time_millis: i64,
    pub losses: Vec<Loss>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchEventKind {
    Status,
    Log,
    Platform,
    Warning,
}

/// Closed projection of the provider's free-text log/platform source field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProducerKind {
    Gateway,
    Sandbox,
    Kubernetes,
    Docker,
    Process,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchWindow {
    pub authenticity: Authenticity,
    pub installation: String,
    pub workspace: String,
    pub sandbox: String,
    pub events: Vec<WatchEvent>,
    pub continuity: Continuity,
}

/// Explicit finite watch cancellation. It has no provider side effect: dropping
/// the read stream ends the local HTTP/2 stream; no policy or sandbox changes.
#[derive(Clone, Debug, Default)]
pub struct WatchControl(Arc<WatchControlInner>);

#[derive(Debug, Default)]
struct WatchControlInner {
    cancelled: AtomicBool,
    notify: Notify,
}

impl WatchControl {
    pub fn cancel(&self) {
        self.0.cancelled.store(true, Ordering::Release);
        self.0.notify.notify_waiters();
    }

    fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }

    async fn cancelled(&self) {
        loop {
            if self.is_cancelled() {
                return;
            }
            let notified = self.0.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.is_cancelled() {
                return;
            }
            notified.await;
        }
    }
}

// One immutable monotonic deadline follows the lease into every connection
// copy and read. Recheck after polling: a ready future may itself cross the
// deadline before returning. Cancellation has priority for watch operations.
#[derive(Clone, Copy)]
struct LeaseDeadline(Instant);

impl LeaseDeadline {
    fn check(self, control: Option<&WatchControl>) -> Result<(), ClientError> {
        if control.is_some_and(WatchControl::is_cancelled) {
            return Err(ClientError::Cancelled);
        }
        if Instant::now() >= self.0 {
            return Err(ClientError::CredentialLeaseExpired);
        }
        Ok(())
    }

    fn finish<T>(
        self,
        control: Option<&WatchControl>,
        result: Result<T, ClientError>,
    ) -> Result<T, ClientError> {
        self.check(control)?;
        result
    }

    async fn wait<T, F>(
        self,
        control: Option<&WatchControl>,
        operation: F,
    ) -> Result<T, ClientError>
    where
        F: Future<Output = Result<T, ClientError>>,
    {
        self.check(control)?;
        let cancelled = async {
            match control {
                Some(control) => control.cancelled().await,
                None => std::future::pending::<()>().await,
            }
        };
        let result = tokio::select! {
            biased;
            _ = cancelled => Err(ClientError::Cancelled),
            _ = tokio::time::sleep_until(tokio::time::Instant::from_std(self.0)) =>
                Err(ClientError::CredentialLeaseExpired),
            result = operation => result,
        };
        self.finish(control, result)
    }
}

// Test-only acceptance barrier: cross the real lease deadline after a real
// stream completion is ready, before the actual guard accepts it. No synthetic
// clock or provider-supplied expiry error participates in these regressions.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WatchBoundary {
    Event,
    Eof,
    Duration,
    FinalWindow,
}

#[cfg(test)]
struct WatchBoundaryProbe {
    boundary: WatchBoundary,
    expires: Instant,
    cancel: Option<WatchControl>,
    entered: std::sync::Mutex<Option<Instant>>,
}

#[cfg(test)]
impl WatchBoundaryProbe {
    fn cross(&self, boundary: WatchBoundary) {
        if self.boundary != boundary {
            return;
        }
        let entered = Instant::now();
        *self.entered.lock().unwrap() = Some(entered);
        // The test owns this deadline (one second); this is a finite CPU-poll
        // race, not an asynchronous wait that would make the timer win first.
        std::thread::sleep(
            self.expires.saturating_duration_since(entered) + Duration::from_millis(5),
        );
        if let Some(control) = &self.cancel {
            control.cancel();
        }
    }
}

/// Closed wrapper over generated bindings. Generated code is private so callers
/// cannot select arbitrary OpenShell RPC names.
pub struct OpenShellReadClient {
    config: ClientConfig,
    lease: BearerLease,
    client: generated::v1::open_shell_client::OpenShellClient<Channel>,
    #[cfg(test)]
    watch_boundary_probe: Option<Arc<WatchBoundaryProbe>>,
}

impl OpenShellReadClient {
    pub async fn connect(config: ClientConfig, lease: BearerLease) -> Result<Self, ClientError> {
        let deadline = lease.deadline();
        deadline.check(None)?;
        let connector = DirectPinnedConnector::new(&config, deadline)?;
        // Tonic uses an endpoint URI to decide whether *it* must perform TLS.
        // DirectPinnedConnector has already established authenticated TLS with
        // the caller-selected root and exact leaf pin, so configuring this
        // internal plumbing URI as HTTPS would apply a second, unrelated TLS
        // layer. The public configuration remains HTTPS-only and the socket
        // returned by the connector is always TLS; no plaintext connection is
        // possible through this path.
        let transport_uri = format!("http://{}:{}", config.host, config.selected_peer.port());
        let public_origin =
            Uri::from_str(&config.origin).map_err(|_| ClientError::InvalidConfiguration)?;
        let endpoint = Endpoint::from_shared(transport_uri)
            .map_err(|_| ClientError::InvalidConfiguration)?
            .origin(public_origin)
            .connect_timeout(RPC_DEADLINE)
            .timeout(RPC_DEADLINE)
            .http2_keep_alive_interval(Duration::from_secs(15))
            .keep_alive_timeout(Duration::from_secs(5));
        let channel = deadline
            .wait(None, async {
                tokio::time::timeout(RPC_DEADLINE, endpoint.connect_with_connector(connector))
                    .await
                    .map_err(|_| ClientError::DeadlineExceeded)?
                    .map_err(|error| map_connection_error(&error))
            })
            .await?;
        let client = generated::v1::open_shell_client::OpenShellClient::new(channel)
            .max_decoding_message_size(MAX_MESSAGE_BYTES)
            .max_encoding_message_size(MAX_MESSAGE_BYTES);
        deadline.finish(
            None,
            Ok(Self {
                config,
                lease,
                client,
                #[cfg(test)]
                watch_boundary_probe: None,
            }),
        )
    }

    /// Health only establishes reachability/version. This method then requires
    /// one sandbox-scoped policy read before reporting a protected result.
    pub async fn check_protected_read(&mut self) -> Result<ConnectionCheck, ClientError> {
        let deadline = self.lease.deadline();
        deadline.check(None)?;
        let health_request = Request::new(generated::v1::HealthRequest {});
        let health = Self::with_deadline(deadline, self.client.health(health_request)).await?;
        let health = health.into_inner();
        if health.version != self.config.expected_product_version {
            return Err(ClientError::Unsupported);
        }
        let health_status = health_status(health.status);
        if matches!(
            health_status,
            HealthStatus::Unspecified | HealthStatus::Unhealthy | HealthStatus::Unknown
        ) {
            return Err(ClientError::Source);
        }
        let protected_request =
            self.protected_request(generated::v1::GetSandboxPolicyStatusRequest {
                workspace_scope: Some(named_workspace(&self.config.workspace)),
                version: 0,
                global: false,
                sandbox: self.config.sandbox.clone(),
            })?;
        let response = Self::with_deadline(
            deadline,
            self.client.get_sandbox_policy_status(protected_request),
        )
        .await?;
        let snapshot = policy_revision(response.into_inner().revision)?;
        deadline.finish(
            None,
            Ok(ConnectionCheck {
                authenticity: Authenticity::Unqualified,
                installation: self.config.installation.clone(),
                workspace: self.config.workspace.clone(),
                sandbox: self.config.sandbox.clone(),
                // The provider text matched the packet-pinned value above; retain
                // the pinned constant rather than returning unbounded provider text.
                reported_product_version: OPEN_SHELL_VERSION.to_owned(),
                health: health_status,
                protected_read: snapshot,
                received_time_millis: receipt_time_millis()?,
            }),
        )
    }

    /// Reads only the selected sandbox's policy metadata. Full policy bodies,
    /// load diagnostics and provenance maps do not leave this crate.
    pub async fn read_policy_status(&mut self, version: u32) -> Result<PolicyRead, ClientError> {
        let deadline = self.lease.deadline();
        deadline.check(None)?;
        let status_request =
            self.protected_request(generated::v1::GetSandboxPolicyStatusRequest {
                workspace_scope: Some(named_workspace(&self.config.workspace)),
                version,
                global: false,
                sandbox: self.config.sandbox.clone(),
            })?;
        let status = Self::with_deadline(
            deadline,
            self.client.get_sandbox_policy_status(status_request),
        )
        .await?
        .into_inner();
        if version != 0
            && status
                .revision
                .as_ref()
                .is_none_or(|revision| revision.version != version)
        {
            return Err(ClientError::Stale);
        }
        let list_request = self.protected_request(generated::v1::ListSandboxPoliciesRequest {
            workspace_scope: Some(named_workspace(&self.config.workspace)),
            page_size: MAX_PAGE_SIZE,
            page_token: String::new(),
            global: false,
            sandbox: self.config.sandbox.clone(),
        })?;
        let list = Self::with_deadline(deadline, self.client.list_sandbox_policies(list_request))
            .await?
            .into_inner();
        if list.revisions.len() > MAX_PAGE_SIZE as usize || !list.next_page_token.is_empty() {
            return Err(ClientError::Bounds);
        }
        let queried = policy_revision(status.revision)?;
        let revisions = list
            .revisions
            .into_iter()
            .map(|revision| policy_revision(Some(revision)))
            .collect::<Result<Vec<_>, _>>()?;
        deadline.finish(
            None,
            Ok(PolicyRead {
                authenticity: Authenticity::Unqualified,
                installation: self.config.installation.clone(),
                workspace: self.config.workspace.clone(),
                sandbox: self.config.sandbox.clone(),
                active_version: status.active_version,
                queried,
                revisions,
                received_time_millis: receipt_time_millis()?,
            }),
        )
    }

    /// Reads bounded log metadata. Log messages, targets, fields, URLs and any
    /// apparent provider decision text remain redacted.
    pub async fn read_recent_logs(&mut self) -> Result<LogWindow, ClientError> {
        let deadline = self.lease.deadline();
        deadline.check(None)?;
        let request = self.protected_request(generated::v1::GetSandboxLogsRequest {
            workspace_scope: Some(named_workspace(&self.config.workspace)),
            sandbox: self.config.sandbox.clone(),
            lines: MAX_LOG_LINES,
            since_time: None,
            sources: Vec::new(),
            min_level: String::new(),
        })?;
        let response = Self::with_deadline(deadline, self.client.get_sandbox_logs(request))
            .await?
            .into_inner();
        if response.logs.len() > MAX_LOG_LINES as usize {
            return Err(ClientError::Bounds);
        }
        let mut native_sandbox_id = None;
        let mut source_times_millis = Vec::with_capacity(response.logs.len());
        let mut producers = Vec::with_capacity(response.logs.len());
        let mut inferred_producer = false;
        let mut unknown_producer = false;
        for log in &response.logs {
            validate_identifier(&log.sandbox_id).map_err(|_| ClientError::Source)?;
            source_times_millis.push(timestamp_millis(log.event_time)?);
            let (producer, inferred, unknown) = log_producer(&log.source);
            producers.push(producer);
            inferred_producer |= inferred;
            unknown_producer |= unknown;
            match &native_sandbox_id {
                Some(existing) if existing != &log.sandbox_id => return Err(ClientError::Source),
                Some(_) => {}
                None => native_sandbox_id = Some(log.sandbox_id.clone()),
            }
        }
        let mut losses = if response.logs.is_empty() {
            Vec::new()
        } else {
            vec![
                Loss::LogTextRedacted,
                Loss::LogFieldsRedacted,
                Loss::MissingNativeEventId,
            ]
        };
        if inferred_producer {
            losses.push(Loss::ProducerInferredFromLegacyEmpty);
        }
        if unknown_producer {
            losses.push(Loss::ProducerUnrecognized);
        }
        deadline.finish(
            None,
            Ok(LogWindow {
                authenticity: Authenticity::Unqualified,
                installation: self.config.installation.clone(),
                workspace: self.config.workspace.clone(),
                sandbox: self.config.sandbox.clone(),
                line_count: response.logs.len() as u32,
                buffer_total: response.buffer_total,
                native_sandbox_id,
                source_times_millis,
                producers,
                received_time_millis: receipt_time_millis()?,
                losses,
            }),
        )
    }

    /// One finite, explicitly cancellable watch. It cannot resume by itself and
    /// a warning becomes a visible continuity gap.
    pub async fn watch_recent(
        &mut self,
        control: &WatchControl,
        resume_after_cursor: Option<&str>,
        duration: Duration,
    ) -> Result<WatchWindow, ClientError> {
        if duration.is_zero() || duration > MAX_WATCH_DURATION {
            return Err(ClientError::Bounds);
        }
        let cursor = resume_after_cursor.unwrap_or_default();
        validate_cursor(cursor)?;
        let deadline = self.lease.deadline();
        deadline.check(Some(control))?;
        let request = self.protected_request(generated::v1::WatchSandboxRequest {
            workspace_scope: Some(named_workspace(&self.config.workspace)),
            sandbox: self.config.sandbox.clone(),
            follow_status: true,
            follow_logs: true,
            follow_events: true,
            log_tail_lines: MAX_LOG_LINES,
            event_tail: MAX_LOG_LINES,
            stop_on_terminal: false,
            since_time: None,
            log_sources: Vec::new(),
            log_min_level: String::new(),
            resume_after_cursor: cursor.to_owned(),
        })?;
        let response =
            Self::with_watch_deadline(deadline, control, self.client.watch_sandbox(request))
                .await?;
        let mut stream = response.into_inner();
        let until = tokio::time::Instant::now() + duration;
        let mut events = Vec::new();
        let mut continuity = Continuity::Bounded;
        let mut total_bytes = 0_usize;
        let mut native_sandbox_id: Option<String> = None;
        loop {
            deadline.check(Some(control))?;
            if events.len() == MAX_WATCH_ITEMS {
                return Err(ClientError::Bounds);
            }
            let next = deadline
                .wait(Some(control), async {
                    tokio::select! {
                        biased;
                        _ = tokio::time::sleep_until(until) => {
                            #[cfg(test)]
                            if let Some(probe) = &self.watch_boundary_probe {
                                probe.cross(WatchBoundary::Duration);
                            }
                            Ok(None)
                        },
                        message = stream.message() => {
                            #[cfg(test)]
                            if let Some(probe) = &self.watch_boundary_probe {
                                match &message {
                                    Ok(Some(_)) => probe.cross(WatchBoundary::Event),
                                    Ok(None) => probe.cross(WatchBoundary::Eof),
                                    Err(_) => {},
                                }
                            }
                            message.map_err(map_watch_status)
                        },
                    }
                })
                .await?;
            let Some(event) = next else { break };
            total_bytes = total_bytes
                .checked_add(prost::Message::encoded_len(&event))
                .ok_or(ClientError::Bounds)?;
            if total_bytes > MAX_WINDOW_BYTES {
                return Err(ClientError::Bounds);
            }
            let projected = watch_event(event, receipt_time_millis()?)?;
            if let Some(id) = projected.native_sandbox_id.as_ref() {
                match native_sandbox_id.as_ref() {
                    Some(existing) if existing != id => return Err(ClientError::Source),
                    Some(_) => {}
                    None => native_sandbox_id = Some(id.clone()),
                }
            }
            if projected.kind == WatchEventKind::Warning {
                continuity = Continuity::Gap;
            }
            events.push(projected);
        }
        #[cfg(test)]
        if let Some(probe) = &self.watch_boundary_probe {
            probe.cross(WatchBoundary::FinalWindow);
        }
        deadline.finish(
            Some(control),
            Ok(WatchWindow {
                authenticity: Authenticity::Unqualified,
                installation: self.config.installation.clone(),
                workspace: self.config.workspace.clone(),
                sandbox: self.config.sandbox.clone(),
                events,
                continuity,
            }),
        )
    }

    pub fn stop(&self, control: &WatchControl) {
        control.cancel();
    }

    fn protected_request<T>(&self, message: T) -> Result<Request<T>, ClientError> {
        let mut request = Request::new(message);
        request
            .metadata_mut()
            .insert("authorization", self.lease.authorization_value()?);
        Ok(request)
    }

    async fn with_deadline<T, F>(deadline: LeaseDeadline, operation: F) -> Result<T, ClientError>
    where
        F: Future<Output = Result<T, Status>>,
    {
        deadline
            .wait(None, async {
                match tokio::time::timeout(RPC_DEADLINE, operation).await {
                    Ok(Ok(value)) => Ok(value),
                    Ok(Err(status)) => Err(map_status(status)),
                    Err(_) => Err(ClientError::DeadlineExceeded),
                }
            })
            .await
    }

    async fn with_watch_deadline<T, F>(
        deadline: LeaseDeadline,
        control: &WatchControl,
        operation: F,
    ) -> Result<T, ClientError>
    where
        F: Future<Output = Result<T, Status>>,
    {
        deadline
            .wait(Some(control), async {
                match tokio::time::timeout(RPC_DEADLINE, operation).await {
                    Ok(Ok(value)) => Ok(value),
                    Ok(Err(status)) => Err(map_watch_status(status)),
                    Err(_) => Err(ClientError::DeadlineExceeded),
                }
            })
            .await
    }
}

fn named_workspace(workspace: &str) -> generated::datamodel::v1::WorkspaceSelector {
    generated::datamodel::v1::WorkspaceSelector {
        selection: Some(
            generated::datamodel::v1::workspace_selector::Selection::Workspace(
                workspace.to_owned(),
            ),
        ),
    }
}

fn map_status(status: Status) -> ClientError {
    match status.code() {
        Code::Unauthenticated => ClientError::Unauthenticated,
        Code::PermissionDenied => ClientError::PermissionDenied,
        Code::Unimplemented => ClientError::Unsupported,
        Code::Cancelled => ClientError::Cancelled,
        Code::DeadlineExceeded => ClientError::DeadlineExceeded,
        Code::OutOfRange => ClientError::Gap,
        Code::ResourceExhausted => ClientError::Bounds,
        Code::Unavailable => ClientError::Unreachable,
        _ => ClientError::Source,
    }
}

fn map_watch_status(status: Status) -> ClientError {
    // The wire uses INVALID_ARGUMENT for a never-issued cursor, but it can
    // also reject another request field or scope with the same code. Its
    // free-text message is not a trustworthy discriminator. Only local cursor
    // validation can classify InvalidCursor without overstating the cause.
    map_status(status)
}

fn log_producer(value: &str) -> (ProducerKind, bool, bool) {
    match value {
        "gateway" => (ProducerKind::Gateway, false, false),
        "sandbox" => (ProducerKind::Sandbox, false, false),
        "" => (ProducerKind::Gateway, true, false),
        _ => (ProducerKind::Unknown, false, true),
    }
}

fn platform_producer(value: &str) -> (ProducerKind, bool) {
    match value {
        "kubernetes" => (ProducerKind::Kubernetes, false),
        "docker" => (ProducerKind::Docker, false),
        "process" => (ProducerKind::Process, false),
        _ => (ProducerKind::Unknown, true),
    }
}

fn map_connection_error(error: &(dyn Error + 'static)) -> ClientError {
    let mut current = Some(error);
    while let Some(candidate) = current {
        if let Some(connector) = candidate.downcast_ref::<ConnectorError>() {
            return match connector {
                ConnectorError::LeaseExpired => ClientError::CredentialLeaseExpired,
                ConnectorError::Tls => ClientError::Tls,
                ConnectorError::DestinationMismatch => ClientError::InvalidConfiguration,
                ConnectorError::Unreachable => ClientError::Unreachable,
            };
        }
        current = candidate.source();
    }
    ClientError::Unreachable
}

fn policy_revision(
    revision: Option<generated::v1::SandboxPolicyRevision>,
) -> Result<PolicyRevision, ClientError> {
    let revision = revision.ok_or(ClientError::Source)?;
    let mut losses = vec![
        Loss::PolicyBodyRedacted,
        Loss::PolicyLoadErrorRedacted,
        Loss::PolicyProvenanceRedacted,
    ];
    let policy_hash = if revision.policy_hash.is_empty() {
        losses.push(Loss::PolicyHashMissing);
        None
    } else if is_sha256_hex(&revision.policy_hash) {
        Some(revision.policy_hash)
    } else {
        losses.push(Loss::PolicyHashInvalid);
        None
    };
    Ok(PolicyRevision {
        version: revision.version,
        policy_hash,
        status: policy_status(revision.status),
        created_time_millis: timestamp_millis(revision.created_time)?,
        loaded_time_millis: timestamp_millis(revision.loaded_time)?,
        losses,
    })
}

fn policy_status(value: i32) -> PolicyLoadStatus {
    match value {
        1 => PolicyLoadStatus::Pending,
        2 => PolicyLoadStatus::Loaded,
        3 => PolicyLoadStatus::Failed,
        4 => PolicyLoadStatus::Superseded,
        0 => PolicyLoadStatus::Unspecified,
        _ => PolicyLoadStatus::Unknown,
    }
}

fn health_status(value: i32) -> HealthStatus {
    match value {
        1 => HealthStatus::Healthy,
        2 => HealthStatus::Degraded,
        3 => HealthStatus::Unhealthy,
        0 => HealthStatus::Unspecified,
        _ => HealthStatus::Unknown,
    }
}

fn watch_event(
    event: generated::v1::SandboxStreamEvent,
    received_time_millis: i64,
) -> Result<WatchEvent, ClientError> {
    let cursor = if event.cursor.is_empty() {
        None
    } else {
        validate_cursor(&event.cursor)?;
        Some(event.cursor)
    };
    let (kind, producer, native_sandbox_id, source_time_millis, losses) = match event.payload {
        Some(generated::v1::sandbox_stream_event::Payload::Sandbox(sandbox)) => {
            let metadata = sandbox.metadata.ok_or(ClientError::Source)?;
            validate_identifier(&metadata.id).map_err(|_| ClientError::Source)?;
            (
                WatchEventKind::Status,
                None,
                Some(metadata.id),
                None,
                vec![Loss::MissingNativeEventId],
            )
        }
        Some(generated::v1::sandbox_stream_event::Payload::Log(log)) => {
            validate_identifier(&log.sandbox_id).map_err(|_| ClientError::Source)?;
            let (producer, inferred, unknown) = log_producer(&log.source);
            let mut losses = vec![
                Loss::LogTextRedacted,
                Loss::LogFieldsRedacted,
                Loss::MissingNativeEventId,
            ];
            if inferred {
                losses.push(Loss::ProducerInferredFromLegacyEmpty);
            }
            if unknown {
                losses.push(Loss::ProducerUnrecognized);
            }
            (
                WatchEventKind::Log,
                Some(producer),
                Some(log.sandbox_id),
                timestamp_millis(log.event_time)?,
                losses,
            )
        }
        Some(generated::v1::sandbox_stream_event::Payload::Event(event)) => {
            let (producer, unknown) = platform_producer(&event.source);
            let mut losses = vec![Loss::PlatformTextRedacted, Loss::MissingNativeEventId];
            if unknown {
                losses.push(Loss::ProducerUnrecognized);
            }
            (
                WatchEventKind::Platform,
                Some(producer),
                None,
                timestamp_millis(event.event_time)?,
                losses,
            )
        }
        Some(generated::v1::sandbox_stream_event::Payload::Warning(_)) => (
            WatchEventKind::Warning,
            None,
            None,
            None,
            vec![Loss::StreamWarning],
        ),
        _ => return Err(ClientError::Unsupported),
    };
    Ok(WatchEvent {
        kind,
        producer,
        cursor,
        native_sandbox_id,
        source_time_millis,
        received_time_millis,
        losses,
    })
}

fn receipt_time_millis() -> Result<i64, ClientError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ClientError::Source)
        .and_then(|duration| i64::try_from(duration.as_millis()).map_err(|_| ClientError::Source))
}

fn timestamp_millis(timestamp: Option<prost_types::Timestamp>) -> Result<Option<i64>, ClientError> {
    let Some(timestamp) = timestamp else {
        return Ok(None);
    };
    if !(0..=999_999_999).contains(&timestamp.nanos) {
        return Err(ClientError::Source);
    }
    timestamp
        .seconds
        .checked_mul(1_000)
        .and_then(|seconds| seconds.checked_add(i64::from(timestamp.nanos / 1_000_000)))
        .map(Some)
        .ok_or(ClientError::Source)
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Clone)]
struct DirectPinnedConnector {
    origin_host: String,
    selected_peer: SocketAddr,
    server_name: ServerName<'static>,
    tls: TlsConnector,
    deadline: LeaseDeadline,
}

impl DirectPinnedConnector {
    fn new(config: &ClientConfig, deadline: LeaseDeadline) -> Result<Self, ClientError> {
        deadline.check(None)?;
        let certificate = CertificateDer::from(config.trusted_root_certificate_der.clone());
        let mut roots = RootCertStore::empty();
        roots
            .add(certificate)
            .map_err(|_| ClientError::InvalidConfiguration)?;
        let standard = WebPkiServerVerifier::builder(Arc::new(roots))
            .build()
            .map_err(|_| ClientError::InvalidConfiguration)?;
        let verifier = Arc::new(PinnedVerifier {
            standard,
            pin: config.peer_pin.clone(),
        });
        let mut tls_config = RustlsClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
        tls_config.alpn_protocols = vec![b"h2".to_vec()];
        let server_name = ServerName::try_from(config.host.clone())
            .map_err(|_| ClientError::InvalidConfiguration)?;
        Ok(Self {
            origin_host: config.host.clone(),
            selected_peer: config.selected_peer,
            server_name,
            tls: TlsConnector::from(Arc::new(tls_config)),
            deadline,
        })
    }
}

impl Service<Uri> for DirectPinnedConnector {
    type Response = TokioIo<TlsStream<TcpStream>>;
    type Error = ConnectorError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        _context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, uri: Uri) -> Self::Future {
        let expected_host = self.origin_host.clone();
        let expected_port = self.selected_peer.port();
        let selected_peer = self.selected_peer;
        let server_name = self.server_name.clone();
        let tls = self.tls.clone();
        let deadline = self.deadline;
        Box::pin(async move {
            deadline
                .wait(None, async move {
                    // This is Tonic's internal connector URI. It is deliberately
                    // plain-scheme so Tonic does not add a second TLS layer after this
                    // connector returns its already-authenticated TLS stream.
                    if uri.scheme_str() != Some("http")
                        || uri.host() != Some(expected_host.as_str())
                        || uri.port_u16().unwrap_or(443) != expected_port
                    {
                        return Err(ClientError::InvalidConfiguration);
                    }
                    let stream = TcpStream::connect(selected_peer)
                        .await
                        .map_err(|_| ClientError::Unreachable)?;
                    deadline.check(None)?;
                    let stream = tls
                        .connect(server_name, stream)
                        .await
                        .map_err(|_| ClientError::Tls)?;
                    Ok(TokioIo::new(stream))
                })
                .await
                .map_err(|error| match error {
                    ClientError::CredentialLeaseExpired => ConnectorError::LeaseExpired,
                    ClientError::InvalidConfiguration => ConnectorError::DestinationMismatch,
                    ClientError::Tls => ConnectorError::Tls,
                    _ => ConnectorError::Unreachable,
                })
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum ConnectorError {
    LeaseExpired,
    DestinationMismatch,
    Unreachable,
    Tls,
}

impl fmt::Display for ConnectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LeaseExpired => "credential_lease_expired",
            Self::DestinationMismatch => "pinned_destination_mismatch",
            Self::Unreachable => "unreachable",
            Self::Tls => "tls",
        })
    }
}

impl Error for ConnectorError {}

#[derive(Debug)]
struct PinnedVerifier {
    standard: Arc<WebPkiServerVerifier>,
    pin: CertificatePin,
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, RustlsError> {
        if !self.pin.matches(end_entity.as_ref()) {
            return Err(RustlsError::General(
                "pinned peer certificate changed".to_owned(),
            ));
        }
        self.standard
            .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        self.standard.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, RustlsError> {
        self.standard.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.standard.supported_verify_schemes()
    }
}

/// Disposable loopback TLS peer for synthetic client and host tests.
///
/// This module exists only for this crate's unit tests and consumers that
/// explicitly enable `synthetic-peer-test-support`. It exposes finite fixed
/// protocol cases, never generated bindings or generic RPC dispatch.
#[cfg(any(test, feature = "synthetic-peer-test-support"))]
pub mod synthetic_peer {
    use super::{ClientError, MAX_LOG_LINES, MAX_PAGE_SIZE, generated};
    use std::{
        convert::Infallible,
        future::Future,
        net::SocketAddr,
        pin::Pin,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context, Poll},
        time::Duration,
    };

    use tokio::{
        net::TcpListener,
        sync::{Notify, oneshot},
        task::JoinHandle,
    };
    #[cfg(test)]
    use tokio_stream::StreamExt;
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic as tonic_test;
    use tonic::{
        body::Body,
        codegen::http::Response as HttpResponse,
        server::{Grpc, NamedService, ServerStreamingService, UnaryService},
        transport::{Identity, Server, ServerTlsConfig},
    };
    use tower::Service;

    const SUPPORT_WAIT: Duration = Duration::from_secs(5);
    const MAX_SYNTHETIC_CALLS: usize = 16;
    const ORIGIN_HOST: &str = "gateway.test";
    const EXPECTED_TOKEN: &str = "Bearer test-token";

    /// Fixed server behavior. No caller-provided RPC or response exists.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SyntheticScenario {
        Healthy,
        WrongReportedVersion,
        Unhealthy,
        SelectedReadFailure,
        StalledWatch,
    }

    /// Fixed ledger entries. `Unexpected` and `InvalidScheme` prove an
    /// attempted escape from the closed protocol surface.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum SyntheticRpc {
        Health,
        GetSandboxPolicyStatus,
        ListSandboxPolicies,
        GetSandboxLogs,
        WatchSandbox,
        Unexpected,
        InvalidScheme,
    }

    /// One disposable peer. Its root and leaf are generated at startup for
    /// `gateway.test`; its socket is always IPv4 loopback.
    pub struct SyntheticPeer {
        origin: String,
        selected_peer: SocketAddr,
        root_der: Vec<u8>,
        leaf_der: Vec<u8>,
        calls: Arc<Ledger>,
        watch_started: Arc<WatchStarted>,
        shutdown: Option<oneshot::Sender<()>>,
        task: Option<JoinHandle<Result<(), tonic_test::transport::Error>>>,
        #[cfg(test)]
        transport_socket: Arc<Mutex<Option<std::net::TcpStream>>>,
        #[cfg(test)]
        probes: Arc<ReadProbes>,
    }

    impl SyntheticPeer {
        /// Starts only a generated-cert loopback peer for one fixed scenario.
        pub async fn start(scenario: SyntheticScenario) -> Result<Self, ClientError> {
            let mut root_parameters = rcgen::CertificateParams::new(vec!["test-root".to_owned()])
                .map_err(|_| ClientError::Source)?;
            root_parameters.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
            let root_key = rcgen::KeyPair::generate().map_err(|_| ClientError::Source)?;
            let root = root_parameters
                .self_signed(&root_key)
                .map_err(|_| ClientError::Source)?;
            let leaf_parameters = rcgen::CertificateParams::new(vec![ORIGIN_HOST.to_owned()])
                .map_err(|_| ClientError::Source)?;
            let leaf_key = rcgen::KeyPair::generate().map_err(|_| ClientError::Source)?;
            let leaf = leaf_parameters
                .signed_by(&leaf_key, &root, &root_key)
                .map_err(|_| ClientError::Source)?;
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .map_err(|_| ClientError::Unreachable)?;
            let selected_peer = listener
                .local_addr()
                .map_err(|_| ClientError::Unreachable)?;
            if !selected_peer.ip().is_loopback() {
                return Err(ClientError::InvalidConfiguration);
            }
            let calls = Arc::new(Ledger::default());
            let watch_started = Arc::new(WatchStarted::default());
            let (shutdown_tx, shutdown_rx) = oneshot::channel();
            #[cfg(test)]
            let probes = Arc::new(ReadProbes::default());
            let peer = FixedPeer {
                #[cfg(test)]
                probes: Arc::clone(&probes),
                scenario,
                calls: Arc::clone(&calls),
                watch_started: Arc::clone(&watch_started),
            };
            let certificate_pem = leaf.pem();
            let key_pem = leaf_key.serialize_pem();
            #[cfg(test)]
            let transport_socket = Arc::new(Mutex::new(None));
            #[cfg(test)]
            let captured_socket = Arc::clone(&transport_socket);
            #[cfg(test)]
            let socket_probes = Arc::clone(&probes);
            let task = tokio::spawn(async move {
                #[cfg(not(test))]
                let incoming = TcpListenerStream::new(listener);
                #[cfg(test)]
                let incoming = TcpListenerStream::new(listener).map(move |accepted| {
                    let stream = accepted?.into_std()?;
                    socket_probes.accepted.fetch_add(1, Ordering::AcqRel);
                    let observer = stream.try_clone()?;
                    *captured_socket.lock().expect("synthetic socket custody") = Some(observer);
                    tokio::net::TcpStream::from_std(stream)
                });
                Server::builder()
                    .tls_config(
                        ServerTlsConfig::new()
                            .identity(Identity::from_pem(certificate_pem, key_pem)),
                    )
                    .expect("generated synthetic TLS identity")
                    .add_service(peer)
                    .serve_with_incoming_shutdown(incoming, async {
                        let _ = shutdown_rx.await;
                    })
                    .await
            });
            Ok(Self {
                origin: format!("https://{ORIGIN_HOST}:{}", selected_peer.port()),
                selected_peer,
                root_der: root.der().to_vec(),
                leaf_der: leaf.der().to_vec(),
                calls,
                watch_started,
                shutdown: Some(shutdown_tx),
                task: Some(task),
                #[cfg(test)]
                transport_socket,
                #[cfg(test)]
                probes,
            })
        }

        #[cfg(test)]
        pub(super) fn accepted_connections(&self) -> usize {
            self.probes.accepted.load(Ordering::Acquire)
        }

        #[cfg(test)]
        pub(super) fn hold(&self, method: SyntheticRpc) {
            *self.probes.held.lock().unwrap() = Some(method);
        }

        #[cfg(test)]
        pub(super) async fn wait_held(&self) {
            tokio::time::timeout(SUPPORT_WAIT, self.probes.entered.wait())
                .await
                .unwrap();
        }

        #[cfg(test)]
        pub(super) fn release(&self) {
            self.probes.release.signal();
        }

        /// Internal fault injection severs the actual accepted TCP transport.
        /// It is not exported by the opt-in dependency feature.
        #[cfg(test)]
        pub(super) fn sever_transport(&self) -> Result<(), ClientError> {
            self.transport_socket
                .lock()
                .map_err(|_| ClientError::Source)?
                .take()
                .ok_or(ClientError::Source)?
                .shutdown(std::net::Shutdown::Both)
                .map_err(|_| ClientError::Source)
        }

        pub fn origin(&self) -> &str {
            &self.origin
        }

        pub const fn selected_peer(&self) -> SocketAddr {
            self.selected_peer
        }

        pub fn root_der(&self) -> &[u8] {
            &self.root_der
        }

        pub fn leaf_der(&self) -> &[u8] {
            &self.leaf_der
        }

        /// Snapshot of only fixed method names; no headers or payloads enter it.
        pub fn calls(&self) -> Result<Vec<SyntheticRpc>, ClientError> {
            self.calls.snapshot()
        }

        /// Waits for first stalled-watch stream poll. Bound prevents a test
        /// deadlock from becoming synthetic success.
        pub async fn wait_watch_started(&self) -> Result<(), ClientError> {
            tokio::time::timeout(SUPPORT_WAIT, self.watch_started.wait())
                .await
                .map_err(|_| ClientError::DeadlineExceeded)
        }

        /// Stops accept and active server work, then joins boundedly. A forced
        /// abort returns an error so tests cannot count an undrained peer.
        pub async fn shutdown(mut self) -> Result<(), ClientError> {
            let sender = self.shutdown.take().ok_or(ClientError::Source)?;
            sender.send(()).map_err(|_| ClientError::Source)?;
            let mut task = self.task.take().ok_or(ClientError::Source)?;
            match tokio::time::timeout(SUPPORT_WAIT, &mut task).await {
                Ok(Ok(Ok(()))) => Ok(()),
                Ok(Ok(Err(_))) | Ok(Err(_)) => Err(ClientError::Source),
                Err(_) => {
                    task.abort();
                    let _ = task.await;
                    Err(ClientError::DeadlineExceeded)
                }
            }
        }
    }

    impl Drop for SyntheticPeer {
        fn drop(&mut self) {
            if let Some(task) = self.task.take() {
                task.abort();
            }
        }
    }

    // Private test barriers never accept provider text, credentials, or custom
    // replies. Production and the opt-in external peer API cannot configure them.
    #[cfg(test)]
    #[derive(Default)]
    struct ReadProbes {
        accepted: std::sync::atomic::AtomicUsize,
        held: Mutex<Option<SyntheticRpc>>,
        entered: WatchStarted,
        release: WatchStarted,
    }

    #[cfg(test)]
    impl ReadProbes {
        async fn before_reply(&self, method: SyntheticRpc) {
            let held = *self.held.lock().unwrap() == Some(method);
            if held {
                self.entered.signal();
                self.release.wait().await;
            }
        }
    }

    #[derive(Default)]
    struct WatchStarted {
        started: AtomicBool,
        notify: Notify,
    }

    #[derive(Default)]
    struct Ledger {
        calls: Mutex<Vec<SyntheticRpc>>,
        overflowed: AtomicBool,
    }

    impl Ledger {
        fn record(&self, call: SyntheticRpc) {
            let mut calls = self.calls.lock().expect("synthetic call ledger");
            if calls.len() == MAX_SYNTHETIC_CALLS {
                self.overflowed.store(true, Ordering::Release);
                return;
            }
            calls.push(call);
        }

        fn snapshot(&self) -> Result<Vec<SyntheticRpc>, ClientError> {
            if self.overflowed.load(Ordering::Acquire) {
                return Err(ClientError::Bounds);
            }
            Ok(self.calls.lock().expect("synthetic call ledger").clone())
        }
    }

    impl WatchStarted {
        fn signal(&self) {
            self.started.store(true, Ordering::Release);
            self.notify.notify_waiters();
        }

        async fn wait(&self) {
            loop {
                if self.started.load(Ordering::Acquire) {
                    return;
                }
                let notified = self.notify.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                if self.started.load(Ordering::Acquire) {
                    return;
                }
                notified.await;
            }
        }
    }

    #[derive(Clone)]
    struct FixedPeer {
        #[cfg(test)]
        probes: Arc<ReadProbes>,
        scenario: SyntheticScenario,
        calls: Arc<Ledger>,
        watch_started: Arc<WatchStarted>,
    }

    impl FixedPeer {
        fn record(&self, call: SyntheticRpc) {
            self.calls.record(call);
        }
    }

    impl NamedService for FixedPeer {
        const NAME: &'static str = "openshell.v1.OpenShell";
    }

    impl Service<http::Request<Body>> for FixedPeer {
        type Response = HttpResponse<Body>;
        type Error = Infallible;
        type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, request: http::Request<Body>) -> Self::Future {
            if request.uri().scheme_str() != Some("https") {
                self.record(SyntheticRpc::InvalidScheme);
            }
            let peer = self.clone();
            match request.uri().path() {
                "/openshell.v1.OpenShell/Health" => Box::pin(async move {
                    Ok(Grpc::new(tonic_test::codec::ProstCodec::default())
                        .unary(FixedHealth { peer }, request)
                        .await)
                }),
                "/openshell.v1.OpenShell/GetSandboxPolicyStatus" => Box::pin(async move {
                    Ok(Grpc::new(tonic_test::codec::ProstCodec::default())
                        .unary(FixedPolicyStatus { peer }, request)
                        .await)
                }),
                "/openshell.v1.OpenShell/ListSandboxPolicies" => Box::pin(async move {
                    Ok(Grpc::new(tonic_test::codec::ProstCodec::default())
                        .unary(FixedPolicyList { peer }, request)
                        .await)
                }),
                "/openshell.v1.OpenShell/GetSandboxLogs" => Box::pin(async move {
                    Ok(Grpc::new(tonic_test::codec::ProstCodec::default())
                        .unary(FixedLogs { peer }, request)
                        .await)
                }),
                "/openshell.v1.OpenShell/WatchSandbox" => Box::pin(async move {
                    Ok(Grpc::new(tonic_test::codec::ProstCodec::default())
                        .server_streaming(FixedWatch { peer }, request)
                        .await)
                }),
                _ => Box::pin(async move {
                    peer.record(SyntheticRpc::Unexpected);
                    Ok(HttpResponse::builder()
                        .status(200)
                        .header("content-type", "application/grpc")
                        .header("grpc-status", "12")
                        .body(Body::empty())
                        .expect("fixed gRPC response"))
                }),
            }
        }
    }

    #[allow(clippy::result_large_err)]
    pub(super) fn allow_protected<T>(
        request: &tonic_test::Request<T>,
    ) -> Result<(), tonic_test::Status> {
        if request
            .metadata()
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            != Some(EXPECTED_TOKEN)
        {
            return Err(tonic_test::Status::permission_denied("synthetic denied"));
        }
        Ok(())
    }

    pub(super) fn valid_scope(
        scope: Option<generated::datamodel::v1::WorkspaceSelector>,
        sandbox: &str,
    ) -> bool {
        matches!(scope.and_then(|scope| scope.selection),
            Some(generated::datamodel::v1::workspace_selector::Selection::Workspace(workspace))
                if workspace == "workspace-1" && sandbox == "sandbox-1")
    }

    pub(super) fn revision() -> generated::v1::SandboxPolicyRevision {
        generated::v1::SandboxPolicyRevision {
            version: 7,
            policy_hash: "a3c0c0201ae623590c9deb6b0027a86b5193eb727d0e79c6852b40704d1a4d7d"
                .to_owned(),
            status: 2,
            load_error: "secret-like-provider-error".to_owned(),
            created_time: Some(prost_types::Timestamp {
                seconds: 10,
                nanos: 0,
            }),
            loaded_time: Some(prost_types::Timestamp {
                seconds: 11,
                nanos: 0,
            }),
            policy: None,
            provenance: [("secret".to_owned(), "not-for-projection".to_owned())]
                .into_iter()
                .collect(),
        }
    }

    pub(super) fn logs_fixture() -> generated::v1::GetSandboxLogsResponse {
        generated::v1::GetSandboxLogsResponse {
            logs: vec![generated::v1::SandboxLogLine {
                sandbox_id: "native-sandbox-1".to_owned(),
                event_time: None,
                level: "INFO".to_owned(),
                target: "secret-target".to_owned(),
                message: "allowed secret-token".to_owned(),
                source: "gateway".to_owned(),
                fields: [("secret".to_owned(), "secret-token".to_owned())]
                    .into_iter()
                    .collect(),
            }],
            buffer_total: 1,
        }
    }

    struct FixedHealth {
        peer: FixedPeer,
    }
    impl UnaryService<generated::v1::HealthRequest> for FixedHealth {
        type Response = generated::v1::HealthResponse;
        type Future = Pin<
            Box<
                dyn Future<
                        Output = Result<tonic_test::Response<Self::Response>, tonic_test::Status>,
                    > + Send,
            >,
        >;
        fn call(&mut self, _: tonic_test::Request<generated::v1::HealthRequest>) -> Self::Future {
            self.peer.record(SyntheticRpc::Health);
            let version = if self.peer.scenario == SyntheticScenario::WrongReportedVersion {
                "v0.0.0"
            } else {
                "v0.1.4"
            };
            let status = if self.peer.scenario == SyntheticScenario::Unhealthy {
                3
            } else {
                1
            };
            #[cfg(test)]
            let probes = Arc::clone(&self.peer.probes);
            Box::pin(async move {
                #[cfg(test)]
                probes.before_reply(SyntheticRpc::Health).await;
                Ok(tonic_test::Response::new(generated::v1::HealthResponse {
                    status,
                    version: version.to_owned(),
                }))
            })
        }
    }

    struct FixedPolicyStatus {
        peer: FixedPeer,
    }
    impl UnaryService<generated::v1::GetSandboxPolicyStatusRequest> for FixedPolicyStatus {
        type Response = generated::v1::GetSandboxPolicyStatusResponse;
        type Future = Pin<
            Box<
                dyn Future<
                        Output = Result<tonic_test::Response<Self::Response>, tonic_test::Status>,
                    > + Send,
            >,
        >;
        fn call(
            &mut self,
            request: tonic_test::Request<generated::v1::GetSandboxPolicyStatusRequest>,
        ) -> Self::Future {
            self.peer.record(SyntheticRpc::GetSandboxPolicyStatus);
            #[cfg(test)]
            let probes = Arc::clone(&self.peer.probes);
            Box::pin(async move {
                #[cfg(test)]
                probes
                    .before_reply(SyntheticRpc::GetSandboxPolicyStatus)
                    .await;
                allow_protected(&request)?;
                let request = request.into_inner();
                if request.global || !valid_scope(request.workspace_scope, &request.sandbox) {
                    return Err(tonic_test::Status::invalid_argument("synthetic scope"));
                }
                Ok(tonic_test::Response::new(
                    generated::v1::GetSandboxPolicyStatusResponse {
                        revision: Some(revision()),
                        active_version: 7,
                    },
                ))
            })
        }
    }

    struct FixedPolicyList {
        peer: FixedPeer,
    }
    impl UnaryService<generated::v1::ListSandboxPoliciesRequest> for FixedPolicyList {
        type Response = generated::v1::ListSandboxPoliciesResponse;
        type Future = Pin<
            Box<
                dyn Future<
                        Output = Result<tonic_test::Response<Self::Response>, tonic_test::Status>,
                    > + Send,
            >,
        >;
        fn call(
            &mut self,
            request: tonic_test::Request<generated::v1::ListSandboxPoliciesRequest>,
        ) -> Self::Future {
            self.peer.record(SyntheticRpc::ListSandboxPolicies);
            #[cfg(test)]
            let probes = Arc::clone(&self.peer.probes);
            Box::pin(async move {
                #[cfg(test)]
                probes.before_reply(SyntheticRpc::ListSandboxPolicies).await;
                allow_protected(&request)?;
                let request = request.into_inner();
                if request.global
                    || request.page_size != MAX_PAGE_SIZE
                    || !request.page_token.is_empty()
                    || !valid_scope(request.workspace_scope, &request.sandbox)
                {
                    return Err(tonic_test::Status::invalid_argument("synthetic scope"));
                }
                Ok(tonic_test::Response::new(
                    generated::v1::ListSandboxPoliciesResponse {
                        revisions: vec![revision()],
                        next_page_token: String::new(),
                    },
                ))
            })
        }
    }

    struct FixedLogs {
        peer: FixedPeer,
    }
    impl UnaryService<generated::v1::GetSandboxLogsRequest> for FixedLogs {
        type Response = generated::v1::GetSandboxLogsResponse;
        type Future = Pin<
            Box<
                dyn Future<
                        Output = Result<tonic_test::Response<Self::Response>, tonic_test::Status>,
                    > + Send,
            >,
        >;
        fn call(
            &mut self,
            request: tonic_test::Request<generated::v1::GetSandboxLogsRequest>,
        ) -> Self::Future {
            self.peer.record(SyntheticRpc::GetSandboxLogs);
            let scenario = self.peer.scenario;
            #[cfg(test)]
            let probes = Arc::clone(&self.peer.probes);
            Box::pin(async move {
                #[cfg(test)]
                probes.before_reply(SyntheticRpc::GetSandboxLogs).await;
                allow_protected(&request)?;
                let request = request.into_inner();
                if request.lines != MAX_LOG_LINES
                    || !valid_scope(request.workspace_scope, &request.sandbox)
                {
                    return Err(tonic_test::Status::invalid_argument("synthetic scope"));
                }
                if scenario == SyntheticScenario::SelectedReadFailure {
                    return Err(tonic_test::Status::unavailable("synthetic selected read"));
                }
                Ok(tonic_test::Response::new(logs_fixture()))
            })
        }
    }

    struct FixedWatch {
        peer: FixedPeer,
    }
    struct StalledStream {
        started: Arc<WatchStarted>,
        signalled: bool,
        initial: Option<generated::v1::SandboxStreamEvent>,
    }
    struct InitialThenPending {
        initial: Option<Result<generated::v1::SandboxStreamEvent, tonic_test::Status>>,
    }
    impl tokio_stream::Stream for StalledStream {
        type Item = Result<generated::v1::SandboxStreamEvent, tonic_test::Status>;
        fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            if !self.signalled {
                self.signalled = true;
                self.started.signal();
            }
            match self.initial.take() {
                Some(event) => Poll::Ready(Some(Ok(event))),
                None => Poll::Pending,
            }
        }
    }
    impl tokio_stream::Stream for InitialThenPending {
        type Item = Result<generated::v1::SandboxStreamEvent, tonic_test::Status>;
        fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            match self.initial.take() {
                Some(event) => Poll::Ready(Some(event)),
                None => Poll::Pending,
            }
        }
    }
    fn warning_event(message: String) -> generated::v1::SandboxStreamEvent {
        generated::v1::SandboxStreamEvent {
            payload: Some(generated::v1::sandbox_stream_event::Payload::Warning(
                generated::v1::SandboxStreamWarning { message },
            )),
            cursor: String::new(),
        }
    }
    fn log_event(sandbox_id: &str) -> generated::v1::SandboxStreamEvent {
        generated::v1::SandboxStreamEvent {
            payload: Some(generated::v1::sandbox_stream_event::Payload::Log(
                generated::v1::SandboxLogLine {
                    sandbox_id: sandbox_id.to_owned(),
                    event_time: None,
                    level: "INFO".to_owned(),
                    target: String::new(),
                    message: "secret-like provider text".to_owned(),
                    source: "gateway".to_owned(),
                    fields: Default::default(),
                },
            )),
            cursor: String::new(),
        }
    }
    impl ServerStreamingService<generated::v1::WatchSandboxRequest> for FixedWatch {
        type Response = generated::v1::SandboxStreamEvent;
        type ResponseStream = Pin<
            Box<dyn tokio_stream::Stream<Item = Result<Self::Response, tonic_test::Status>> + Send>,
        >;
        type Future = Pin<
            Box<
                dyn Future<
                        Output = Result<
                            tonic_test::Response<Self::ResponseStream>,
                            tonic_test::Status,
                        >,
                    > + Send,
            >,
        >;
        fn call(
            &mut self,
            request: tonic_test::Request<generated::v1::WatchSandboxRequest>,
        ) -> Self::Future {
            self.peer.record(SyntheticRpc::WatchSandbox);
            let peer = self.peer.clone();
            #[cfg(test)]
            let probes = Arc::clone(&self.peer.probes);
            Box::pin(async move {
                #[cfg(test)]
                probes.before_reply(SyntheticRpc::WatchSandbox).await;
                allow_protected(&request)?;
                let request = request.into_inner();
                if !valid_scope(request.workspace_scope, &request.sandbox)
                    || !request.follow_status
                    || !request.follow_logs
                    || !request.follow_events
                    || request.log_tail_lines != MAX_LOG_LINES
                    || request.event_tail != MAX_LOG_LINES
                {
                    return Err(tonic_test::Status::invalid_argument("synthetic scope"));
                }
                if request.resume_after_cursor == "delay-headers" {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                let stream: Self::ResponseStream = if peer.scenario
                    == SyntheticScenario::StalledWatch
                {
                    Box::pin(StalledStream {
                        started: peer.watch_started,
                        signalled: false,
                        initial: None,
                    })
                } else {
                    match request.resume_after_cursor.as_str() {
                        "" => Box::pin(tokio_stream::iter(vec![Ok(warning_event(
                            "secret-like warning".to_owned(),
                        ))])),
                        "out-of-range" => Box::pin(tokio_stream::iter(vec![Err(
                            tonic_test::Status::out_of_range("trimmed"),
                        )])),
                        "disconnect" => Box::pin(tokio_stream::iter(vec![Err(
                            tonic_test::Status::unavailable("disconnected"),
                        )])),
                        "oversized" => Box::pin(tokio_stream::iter(vec![Ok(warning_event(
                            "x".repeat(super::MAX_WINDOW_BYTES + 1),
                        ))])),
                        "foreign-sandbox" => Box::pin(tokio_stream::iter(vec![
                            Ok(log_event("native-sandbox-1")),
                            Ok(log_event("native-sandbox-2")),
                        ])),
                        "stall" => Box::pin(InitialThenPending {
                            initial: Some(Ok(warning_event("initial warning".to_owned()))),
                        }),
                        "delay-headers" => Box::pin(tokio_stream::iter(Vec::new())),
                        #[cfg(test)]
                        "empty" => Box::pin(tokio_stream::iter(Vec::new())),
                        "drop-transport" => Box::pin(StalledStream {
                            started: peer.watch_started,
                            signalled: false,
                            initial: Some(warning_event("initial warning".to_owned())),
                        }),
                        "invalid-cursor" => {
                            return Err(tonic_test::Status::invalid_argument("invalid cursor"));
                        }
                        _ => return Err(tonic_test::Status::invalid_argument("unexpected cursor")),
                    }
                };
                Ok(tonic_test::Response::new(stream))
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn ledger_bounds_and_reports_overflow() {
            let ledger = Ledger::default();
            for _ in 0..=MAX_SYNTHETIC_CALLS {
                ledger.record(SyntheticRpc::Health);
            }
            assert_eq!(ledger.snapshot(), Err(ClientError::Bounds));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_constructor_rejected_values_clear_before_error_return() {
        let future = Instant::now() + Duration::from_secs(30);
        for (plain, expiry) in [
            (String::new(), future),
            ("x".repeat(16 * 1024 + 1), future),
            ("expired-dummy-credential".to_owned(), Instant::now()),
        ] {
            let mut owned = Zeroizing::new(plain);
            assert_eq!(
                BearerLease::validate_owned_value(&mut owned, expiry),
                Err(ClientError::InvalidConfiguration)
            );
            // Observe the live owned value, never deallocated or spare memory.
            assert!(owned.is_empty());
        }
    }

    #[test]
    fn credential_constructor_preserves_rejection_boundaries() {
        let future = Instant::now() + Duration::from_secs(30);
        for (plain, expiry) in [
            (String::new(), future),
            ("x".repeat(16 * 1024 + 1), future),
            ("expired-dummy-credential".to_owned(), Instant::now()),
        ] {
            let error = BearerLease::new(plain, expiry).unwrap_err();
            assert_eq!(error, ClientError::InvalidConfiguration);
            assert_eq!(error.code(), "invalid_configuration");
        }
    }

    #[test]
    fn credential_constructor_valid_value_keeps_bound_and_immutable_deadline() {
        let expiry = Instant::now() + Duration::from_secs(30);
        let mut owned = Zeroizing::new("x".repeat(16 * 1024));
        assert_eq!(
            BearerLease::validate_owned_value(&mut owned, expiry),
            Ok(())
        );
        assert_eq!(owned.len(), 16 * 1024);
        assert!(owned.bytes().all(|byte| byte == b'x'));
        let lease = BearerLease::new(std::mem::take(&mut *owned), expiry).unwrap();
        assert_eq!(lease.value.len(), 16 * 1024);
        assert_eq!(lease.deadline().0, expiry);
        assert_eq!(format!("{lease:?}"), "BearerLease([redacted])");
    }

    fn pinned_config(origin: &str) -> Result<ClientConfig, ClientError> {
        let certificate = vec![1, 2, 3, 4];
        let pin = CertificatePin::from_certificate_der(&certificate)?;
        ClientConfig::new(
            origin.to_owned(),
            "127.0.0.1:443".parse().unwrap(),
            "installation-1".to_owned(),
            "workspace-1".to_owned(),
            "sandbox-1".to_owned(),
            "v0.1.4".to_owned(),
            certificate,
            pin,
        )
    }

    #[test]
    fn configuration_rejects_non_https_ip_and_path_destinations() {
        for origin in [
            "http://gateway.example",
            "https://127.0.0.1",
            "https://gateway.example/not-allowed",
            "https://user@gateway.example",
            "https://gateway.example?proxy=1",
        ] {
            assert!(matches!(
                pinned_config(origin),
                Err(ClientError::InvalidConfiguration)
            ));
        }
        assert!(pinned_config("https://gateway.example").is_ok());
        assert!(matches!(
            ClientConfig::new(
                "https://gateway.example".to_owned(),
                "127.0.0.1:443".parse().unwrap(),
                "installation-1".to_owned(),
                "workspace-1".to_owned(),
                "sandbox-1".to_owned(),
                "v0.1.5".to_owned(),
                vec![1],
                CertificatePin::from_certificate_der(b"leaf").unwrap(),
            ),
            Err(ClientError::InvalidConfiguration)
        ));
    }

    #[test]
    fn pin_parser_rejects_malformed_values_without_echoing_them() {
        let malformed = "bad-secret-like-pin";
        assert_eq!(
            CertificatePin::from_sha256_hex(malformed).unwrap_err(),
            ClientError::InvalidConfiguration
        );
        let pin = CertificatePin::from_certificate_der(b"leaf-a").unwrap();
        assert!(pin.matches(b"leaf-a"));
        assert!(!pin.matches(b"leaf-b"));
    }

    #[test]
    fn policy_projection_redacts_provider_free_text_and_invalid_hash() {
        let projected = policy_revision(Some(generated::v1::SandboxPolicyRevision {
            version: 9,
            policy_hash: "not-a-hash-secret-like-text".to_owned(),
            status: 3,
            load_error: "secret-like-load-error".to_owned(),
            created_time: Some(prost_types::Timestamp {
                seconds: 1,
                nanos: 500_000_000,
            }),
            loaded_time: None,
            policy: None,
            provenance: [("secret".to_owned(), "do-not-echo".to_owned())]
                .into_iter()
                .collect(),
        }))
        .unwrap();
        assert_eq!(projected.policy_hash, None);
        assert_eq!(projected.status, PolicyLoadStatus::Failed);
        assert_eq!(projected.created_time_millis, Some(1_500));
        assert!(projected.losses.contains(&Loss::PolicyHashInvalid));
        assert!(projected.losses.contains(&Loss::PolicyLoadErrorRedacted));
        assert!(projected.losses.contains(&Loss::PolicyProvenanceRedacted));
        let output = format!("{projected:?}");
        for forbidden in ["secret-like", "do-not-echo", "not-a-hash"] {
            assert!(!output.contains(forbidden));
        }
    }

    #[test]
    fn watch_projection_never_classifies_log_text_as_decision() {
        let event = generated::v1::SandboxStreamEvent {
            payload: Some(generated::v1::sandbox_stream_event::Payload::Log(
                generated::v1::SandboxLogLine {
                    sandbox_id: "sandbox-1".to_owned(),
                    event_time: Some(prost_types::Timestamp {
                        seconds: 11,
                        nanos: 0,
                    }),
                    level: "WARN".to_owned(),
                    target: "target".to_owned(),
                    message: "allowed secret-token denied tool-call".to_owned(),
                    source: "gateway".to_owned(),
                    fields: [("credential".to_owned(), "secret-token".to_owned())]
                        .into_iter()
                        .collect(),
                },
            )),
            cursor: "opaque-cursor-1".to_owned(),
        };
        let projected = watch_event(event, 12_000).unwrap();
        assert_eq!(projected.kind, WatchEventKind::Log);
        assert_eq!(projected.producer, Some(ProducerKind::Gateway));
        assert_eq!(projected.cursor.as_deref(), Some("opaque-cursor-1"));
        assert!(projected.losses.contains(&Loss::LogTextRedacted));
        assert!(projected.losses.contains(&Loss::LogFieldsRedacted));
        let output = format!("{projected:?}");
        for forbidden in ["allowed", "denied", "secret-token", "tool-call"] {
            assert!(!output.contains(forbidden));
        }
    }

    #[test]
    fn producer_projection_preserves_known_kinds_without_free_text() {
        let sandbox =
            watch_event(fake_log_stream_event("native-sandbox-1", "sandbox"), 12_000).unwrap();
        assert_eq!(sandbox.producer, Some(ProducerKind::Sandbox));
        let legacy_gateway =
            watch_event(fake_log_stream_event("native-sandbox-1", ""), 12_000).unwrap();
        assert_eq!(legacy_gateway.producer, Some(ProducerKind::Gateway));
        assert!(
            legacy_gateway
                .losses
                .contains(&Loss::ProducerInferredFromLegacyEmpty)
        );
        let unknown = watch_event(
            fake_log_stream_event("native-sandbox-1", "secret-like-producer"),
            12_000,
        )
        .unwrap();
        assert_eq!(unknown.producer, Some(ProducerKind::Unknown));
        assert!(unknown.losses.contains(&Loss::ProducerUnrecognized));
        assert!(!format!("{unknown:?}").contains("secret-like"));
        let platform = watch_event(
            generated::v1::SandboxStreamEvent {
                payload: Some(generated::v1::sandbox_stream_event::Payload::Event(
                    generated::v1::PlatformEvent {
                        event_time: None,
                        source: "kubernetes".to_owned(),
                        r#type: "Normal".to_owned(),
                        reason: "Started".to_owned(),
                        message: "secret-like-platform-message".to_owned(),
                        metadata: Default::default(),
                    },
                )),
                cursor: String::new(),
            },
            12_000,
        )
        .unwrap();
        assert_eq!(platform.producer, Some(ProducerKind::Kubernetes));
        assert!(!format!("{platform:?}").contains("secret-like"));
    }

    #[test]
    fn warning_is_gap_and_invalid_cursor_is_not_gap() {
        let warning = watch_event(
            generated::v1::SandboxStreamEvent {
                payload: Some(generated::v1::sandbox_stream_event::Payload::Warning(
                    generated::v1::SandboxStreamWarning {
                        message: "dropped secret-like detail".to_owned(),
                    },
                )),
                cursor: String::new(),
            },
            12_000,
        )
        .unwrap();
        assert_eq!(warning.kind, WatchEventKind::Warning);
        assert_eq!(warning.losses, vec![Loss::StreamWarning]);
        assert_eq!(
            map_status(Status::out_of_range("trimmed")),
            ClientError::Gap
        );
        assert_eq!(
            map_status(Status::invalid_argument("never-issued-cursor")),
            ClientError::Source
        );
        assert_eq!(
            map_watch_status(Status::invalid_argument("never-issued-cursor")),
            ClientError::Source
        );
        assert_eq!(
            validate_cursor("bad\nresume"),
            Err(ClientError::InvalidCursor)
        );
    }

    #[test]
    fn control_cancels_without_provider_operation() {
        let control = WatchControl::default();
        assert!(!control.is_cancelled());
        control.cancel();
        assert!(control.is_cancelled());
    }

    #[test]
    fn fixed_errors_never_echo_sensitive_input() {
        let lease = BearerLease::new(
            "very-secret-bearer-value".to_owned(),
            Instant::now() + Duration::from_secs(1),
        )
        .unwrap();
        assert!(!format!("{lease:?}").contains("very-secret"));
        for error in [
            ClientError::InvalidConfiguration,
            ClientError::PermissionDenied,
            ClientError::Source,
        ] {
            assert!(!error.to_string().contains("secret"));
            assert!(!error.to_string().contains("gateway"));
        }
    }

    fn synthetic_config(peer: &synthetic_peer::SyntheticPeer, workspace: &str) -> ClientConfig {
        ClientConfig::new(
            peer.origin().to_owned(),
            peer.selected_peer(),
            "installation-1".to_owned(),
            workspace.to_owned(),
            "sandbox-1".to_owned(),
            OPEN_SHELL_VERSION.to_owned(),
            peer.root_der().to_vec(),
            CertificatePin::from_certificate_der(peer.leaf_der()).unwrap(),
        )
        .unwrap()
    }

    fn fake_log_stream_event(sandbox_id: &str, source: &str) -> generated::v1::SandboxStreamEvent {
        generated::v1::SandboxStreamEvent {
            payload: Some(generated::v1::sandbox_stream_event::Payload::Log(
                generated::v1::SandboxLogLine {
                    sandbox_id: sandbox_id.to_owned(),
                    event_time: None,
                    level: "INFO".to_owned(),
                    target: String::new(),
                    message: "secret-like provider text".to_owned(),
                    source: source.to_owned(),
                    fields: Default::default(),
                },
            )),
            cursor: String::new(),
        }
    }

    #[tokio::test]
    async fn synthetic_peer_feature_has_fixed_cases_and_joined_shutdown() {
        use synthetic_peer::{SyntheticPeer, SyntheticRpc, SyntheticScenario};

        let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
            .await
            .unwrap();
        assert!(peer.selected_peer().ip().is_loopback());
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client =
            OpenShellReadClient::connect(synthetic_config(&peer, "workspace-1"), lease)
                .await
                .unwrap();
        let check = client.check_protected_read().await.unwrap();
        let logs = client.read_recent_logs().await.unwrap();
        let output = format!("{check:?}{logs:?}");
        for forbidden in [
            "test-token",
            "secret-token",
            "secret-target",
            "secret-like-provider-error",
            "not-for-projection",
            "secret-like provider text",
        ] {
            assert!(!output.contains(forbidden));
        }
        assert_eq!(
            peer.calls().unwrap(),
            vec![
                SyntheticRpc::Health,
                SyntheticRpc::GetSandboxPolicyStatus,
                SyntheticRpc::GetSandboxLogs,
            ]
        );
        drop(client);
        peer.shutdown().await.unwrap();

        let peer = SyntheticPeer::start(SyntheticScenario::WrongReportedVersion)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client =
            OpenShellReadClient::connect(synthetic_config(&peer, "workspace-1"), lease)
                .await
                .unwrap();
        assert_eq!(
            client.check_protected_read().await,
            Err(ClientError::Unsupported)
        );
        drop(client);
        peer.shutdown().await.unwrap();

        let peer = SyntheticPeer::start(SyntheticScenario::Unhealthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client =
            OpenShellReadClient::connect(synthetic_config(&peer, "workspace-1"), lease)
                .await
                .unwrap();
        assert_eq!(
            client.check_protected_read().await,
            Err(ClientError::Source)
        );
        assert_eq!(peer.calls().unwrap(), vec![SyntheticRpc::Health]);
        drop(client);
        peer.shutdown().await.unwrap();

        let peer = SyntheticPeer::start(SyntheticScenario::SelectedReadFailure)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client =
            OpenShellReadClient::connect(synthetic_config(&peer, "workspace-1"), lease)
                .await
                .unwrap();
        client.check_protected_read().await.unwrap();
        assert_eq!(
            client.read_recent_logs().await,
            Err(ClientError::Unreachable)
        );
        drop(client);
        peer.shutdown().await.unwrap();

        let peer = SyntheticPeer::start(SyntheticScenario::StalledWatch)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client =
            OpenShellReadClient::connect(synthetic_config(&peer, "workspace-1"), lease)
                .await
                .unwrap();
        let control = WatchControl::default();
        let task_control = control.clone();
        let watch = tokio::spawn(async move {
            client
                .watch_recent(&task_control, None, Duration::from_secs(1))
                .await
        });
        peer.wait_watch_started().await.unwrap();
        control.cancel();
        assert_eq!(watch.await.unwrap(), Err(ClientError::Cancelled));
        peer.shutdown().await.unwrap();
    }

    async fn bounded<T>(future: impl Future<Output = T>) -> T {
        tokio::time::timeout(Duration::from_secs(3), future)
            .await
            .expect("independent test deadline")
    }

    fn expiring_lease() -> (BearerLease, Instant) {
        let expires = Instant::now() + Duration::from_secs(1);
        (
            BearerLease::new("test-token".to_owned(), expires).unwrap(),
            expires,
        )
    }

    async fn past_deadline(expires: Instant) {
        tokio::time::sleep_until(
            tokio::time::Instant::from_std(expires) + Duration::from_millis(5),
        )
        .await;
        assert!(Instant::now() >= expires);
    }

    #[tokio::test]
    async fn expired_connect_and_deferred_connector_copy_open_no_socket() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let config = fake_config(&peer);
        let (lease, expires) = expiring_lease();
        let connector = DirectPinnedConnector::new(&config, lease.deadline()).unwrap();
        let uri: Uri = format!("http://gateway.test:{}", peer.selected_peer().port())
            .parse()
            .unwrap();
        // Create the connector future while valid, then poll the retained copy
        // after expiry, just as a deferred Channel connection could do.
        let deferred = connector.clone().call(uri.clone());
        past_deadline(expires).await;
        assert!(matches!(
            bounded(OpenShellReadClient::connect(config, lease)).await,
            Err(ClientError::CredentialLeaseExpired)
        ));
        assert!(matches!(
            bounded(deferred).await,
            Err(ConnectorError::LeaseExpired)
        ));
        assert!(matches!(
            bounded(connector.clone().call(uri)).await,
            Err(ConnectorError::LeaseExpired)
        ));
        tokio::task::yield_now().await;
        assert_eq!(peer.accepted_connections(), 0);
        assert!(peer.calls().unwrap().is_empty());
        assert_eq!(
            map_connection_error(&ConnectorError::LeaseExpired),
            ClientError::CredentialLeaseExpired
        );
        peer.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn expired_client_refuses_health_and_all_selected_reads() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let (lease, expires) = expiring_lease();
        let mut client = bounded(OpenShellReadClient::connect(fake_config(&peer), lease))
            .await
            .unwrap();
        assert!(Instant::now() < expires);
        past_deadline(expires).await;
        assert_eq!(
            client.check_protected_read().await,
            Err(ClientError::CredentialLeaseExpired)
        );
        assert_eq!(
            client.read_policy_status(0).await,
            Err(ClientError::CredentialLeaseExpired)
        );
        assert_eq!(
            client.read_recent_logs().await,
            Err(ClientError::CredentialLeaseExpired)
        );
        let control = WatchControl::default();
        assert_eq!(
            client
                .watch_recent(&control, None, Duration::from_secs(1))
                .await,
            Err(ClientError::CredentialLeaseExpired)
        );
        control.cancel();
        assert_eq!(
            client
                .watch_recent(&control, None, Duration::from_secs(1))
                .await,
            Err(ClientError::Cancelled)
        );
        assert_eq!(peer.accepted_connections(), 1);
        assert!(peer.calls().unwrap().is_empty());
        drop(client);
        peer.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn expiry_interrupts_accepted_socket_with_held_tls_handshake() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = fake_config(&peer);
        config.selected_peer = listener.local_addr().unwrap();
        config.origin = format!("https://gateway.test:{}", config.selected_peer.port());
        let (lease, expires) = expiring_lease();
        let connect = tokio::spawn(OpenShellReadClient::connect(config, lease));
        let (held_socket, _) = bounded(listener.accept()).await.unwrap();
        assert!(
            Instant::now() < expires,
            "socket accepted while lease valid"
        );
        assert!(matches!(
            bounded(connect).await.unwrap(),
            Err(ClientError::CredentialLeaseExpired)
        ));
        assert!(Instant::now() >= expires);
        assert!(peer.calls().unwrap().is_empty());
        drop(held_socket);
        drop(listener);
        peer.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn expiry_interrupts_each_unary_and_prevents_followup_reads() {
        use synthetic_peer::{SyntheticPeer, SyntheticRpc, SyntheticScenario};
        for method in [
            SyntheticRpc::Health,
            SyntheticRpc::GetSandboxPolicyStatus,
            SyntheticRpc::ListSandboxPolicies,
            SyntheticRpc::GetSandboxLogs,
        ] {
            let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
                .await
                .unwrap();
            peer.hold(method);
            let (lease, expires) = expiring_lease();
            let mut client = bounded(OpenShellReadClient::connect(fake_config(&peer), lease))
                .await
                .unwrap();
            let read = tokio::spawn(async move {
                match method {
                    SyntheticRpc::Health => client.check_protected_read().await.map(|_| ()),
                    SyntheticRpc::GetSandboxLogs => client.read_recent_logs().await.map(|_| ()),
                    _ => client.read_policy_status(0).await.map(|_| ()),
                }
            });
            peer.wait_held().await;
            assert!(Instant::now() < expires, "RPC arrived while lease valid");
            assert_eq!(
                bounded(read).await.unwrap(),
                Err(ClientError::CredentialLeaseExpired)
            );
            peer.release();
            let expected = if method == SyntheticRpc::ListSandboxPolicies {
                vec![SyntheticRpc::GetSandboxPolicyStatus, method]
            } else {
                vec![method]
            };
            assert_eq!(peer.calls().unwrap(), expected);
            peer.shutdown().await.unwrap();
        }
    }

    #[tokio::test]
    async fn expiry_interrupts_held_watch_headers_without_completed_window() {
        use synthetic_peer::{SyntheticPeer, SyntheticRpc, SyntheticScenario};
        let peer = SyntheticPeer::start(SyntheticScenario::Healthy)
            .await
            .unwrap();
        peer.hold(SyntheticRpc::WatchSandbox);
        let (lease, expires) = expiring_lease();
        let mut client = bounded(OpenShellReadClient::connect(fake_config(&peer), lease))
            .await
            .unwrap();
        let read = tokio::spawn(async move {
            client
                .watch_recent(&WatchControl::default(), None, Duration::from_secs(2))
                .await
        });
        peer.wait_held().await;
        assert!(
            Instant::now() < expires,
            "header RPC arrived while lease valid"
        );
        assert_eq!(
            bounded(read).await.unwrap(),
            Err(ClientError::CredentialLeaseExpired)
        );
        peer.release();
        assert_eq!(peer.calls().unwrap(), vec![SyntheticRpc::WatchSandbox]);
        peer.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn expiry_interrupts_established_idle_and_partial_watch() {
        use synthetic_peer::{SyntheticPeer, SyntheticRpc, SyntheticScenario};
        for partial in [false, true] {
            let scenario = if partial {
                SyntheticScenario::Healthy
            } else {
                SyntheticScenario::StalledWatch
            };
            let peer = SyntheticPeer::start(scenario).await.unwrap();
            let (lease, expires) = expiring_lease();
            let mut client = bounded(OpenShellReadClient::connect(fake_config(&peer), lease))
                .await
                .unwrap();
            let read = tokio::spawn(async move {
                client
                    .watch_recent(
                        &WatchControl::default(),
                        partial.then_some("drop-transport"),
                        Duration::from_secs(2),
                    )
                    .await
            });
            peer.wait_watch_started().await.unwrap();
            assert!(Instant::now() < expires, "body polled while lease valid");
            assert_eq!(
                bounded(read).await.unwrap(),
                Err(ClientError::CredentialLeaseExpired)
            );
            assert_eq!(peer.calls().unwrap(), vec![SyntheticRpc::WatchSandbox]);
            peer.shutdown().await.unwrap();
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn real_watch_completion_races_expiry_and_cancellation() {
        use synthetic_peer::{SyntheticPeer, SyntheticRpc, SyntheticScenario};
        for boundary in [
            WatchBoundary::Event,
            WatchBoundary::Eof,
            WatchBoundary::Duration,
            WatchBoundary::FinalWindow,
        ] {
            for cancel_at_boundary in [false, true] {
                let scenario = if boundary == WatchBoundary::Duration {
                    SyntheticScenario::StalledWatch
                } else {
                    SyntheticScenario::Healthy
                };
                let peer = SyntheticPeer::start(scenario).await.unwrap();
                let (lease, expires) = expiring_lease();
                let mut client = bounded(OpenShellReadClient::connect(fake_config(&peer), lease))
                    .await
                    .unwrap();
                let control = WatchControl::default();
                let probe = Arc::new(WatchBoundaryProbe {
                    boundary,
                    expires,
                    cancel: cancel_at_boundary.then(|| control.clone()),
                    entered: std::sync::Mutex::new(None),
                });
                client.watch_boundary_probe = Some(Arc::clone(&probe));
                let read = tokio::spawn(async move {
                    let cursor = (boundary == WatchBoundary::Eof).then_some("empty");
                    client
                        .watch_recent(&control, cursor, Duration::from_millis(100))
                        .await
                });
                let expected = if cancel_at_boundary {
                    ClientError::Cancelled
                } else {
                    ClientError::CredentialLeaseExpired
                };
                assert_eq!(
                    bounded(read).await.unwrap(),
                    Err(expected),
                    "{boundary:?}, cancel={cancel_at_boundary}"
                );
                let entered = probe
                    .entered
                    .lock()
                    .unwrap()
                    .expect("actual watch completion branch reached");
                assert!(entered < expires, "completion was ready before expiry");
                assert!(Instant::now() >= expires);
                assert_eq!(peer.calls().unwrap(), vec![SyntheticRpc::WatchSandbox]);
                peer.shutdown().await.unwrap();
            }
        }
    }

    #[tokio::test]
    async fn expiry_beats_ready_event_eof_duration_and_final_projection() {
        // Exercise the shared wait with each stream completion shape. Blocking
        // only this test thread makes the operation return ready after the real
        // monotonic deadline, without faking an expired-error callback or clock.
        for value in [Some("queued-event"), None, Some("local-duration")] {
            let expires = Instant::now() + Duration::from_millis(20);
            let deadline = LeaseDeadline(expires);
            let control = WatchControl::default();
            let result = deadline
                .wait(Some(&control), async {
                    std::thread::sleep(Duration::from_millis(30));
                    Ok(value)
                })
                .await;
            assert_eq!(result, Err(ClientError::CredentialLeaseExpired));
        }
        let deadline = LeaseDeadline(Instant::now() - Duration::from_millis(1));
        assert_eq!(
            deadline.finish(None, Ok("completed projection")),
            Err(ClientError::CredentialLeaseExpired)
        );
        assert_eq!(
            deadline.wait(None, async { Ok("already queued") }).await,
            Err(ClientError::CredentialLeaseExpired)
        );
    }

    #[tokio::test]
    async fn watch_cancellation_beats_simultaneous_expiry_and_ready_success() {
        let control = WatchControl::default();
        let deadline = LeaseDeadline(Instant::now() - Duration::from_millis(1));
        control.cancel();
        assert_eq!(
            deadline.wait(Some(&control), async { Ok(()) }).await,
            Err(ClientError::Cancelled)
        );
        let control = WatchControl::default();
        let deadline = LeaseDeadline(Instant::now() + Duration::from_millis(20));
        assert_eq!(
            deadline
                .wait(Some(&control), async {
                    std::thread::sleep(Duration::from_millis(30));
                    control.cancel();
                    Ok(())
                })
                .await,
            Err(ClientError::Cancelled)
        );
    }

    fn fake_config(peer: &synthetic_peer::SyntheticPeer) -> ClientConfig {
        synthetic_config(peer, "workspace-1")
    }

    fn fake_config_for_workspace(
        peer: &synthetic_peer::SyntheticPeer,
        workspace: &str,
    ) -> ClientConfig {
        synthetic_config(peer, workspace)
    }

    fn call_names(peer: &synthetic_peer::SyntheticPeer) -> Vec<&'static str> {
        peer.calls()
            .unwrap()
            .into_iter()
            .map(|call| match call {
                synthetic_peer::SyntheticRpc::Health => "Health",
                synthetic_peer::SyntheticRpc::GetSandboxPolicyStatus => "GetSandboxPolicyStatus",
                synthetic_peer::SyntheticRpc::ListSandboxPolicies => "ListSandboxPolicies",
                synthetic_peer::SyntheticRpc::GetSandboxLogs => "GetSandboxLogs",
                synthetic_peer::SyntheticRpc::WatchSandbox => "WatchSandbox",
                synthetic_peer::SyntheticRpc::Unexpected => "Unexpected",
                synthetic_peer::SyntheticRpc::InvalidScheme => "InvalidScheme",
            })
            .collect()
    }

    #[tokio::test]
    async fn fake_grpc_peer_proves_closed_read_allowlist_and_redaction() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client = OpenShellReadClient::connect(fake_config(&peer), lease)
            .await
            .unwrap();

        let connection = client.check_protected_read().await.unwrap();
        assert_eq!(connection.authenticity, Authenticity::Unqualified);
        assert_eq!(connection.installation, "installation-1");
        assert_eq!(connection.workspace, "workspace-1");
        assert_eq!(connection.sandbox, "sandbox-1");
        assert_eq!(connection.health, HealthStatus::Healthy);
        assert_eq!(connection.protected_read.status, PolicyLoadStatus::Loaded);
        let policy = client.read_policy_status(0).await.unwrap();
        assert_eq!(policy.revisions.len(), 1);
        let logs = client.read_recent_logs().await.unwrap();
        assert_eq!(logs.native_sandbox_id.as_deref(), Some("native-sandbox-1"));
        assert_eq!(logs.producers, vec![ProducerKind::Gateway]);
        let control = WatchControl::default();
        let watch = client
            .watch_recent(&control, None, Duration::from_millis(100))
            .await
            .unwrap();
        assert_eq!(watch.continuity, Continuity::Gap);
        let output = format!("{logs:?}{watch:?}{policy:?}");
        for forbidden in [
            "secret-token",
            "secret-target",
            "secret-like",
            "not-for-projection",
        ] {
            assert!(!output.contains(forbidden));
        }
        assert_eq!(
            call_names(&peer).as_slice(),
            [
                "Health",
                "GetSandboxPolicyStatus",
                "GetSandboxPolicyStatus",
                "ListSandboxPolicies",
                "GetSandboxLogs",
                "WatchSandbox",
            ]
        );
        assert!(!call_names(&peer).contains(&"Unexpected"));
        assert!(!call_names(&peer).contains(&"InvalidScheme"));
        drop(peer);
    }

    #[tokio::test]
    async fn health_success_does_not_bypass_protected_denial() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "not-authorized".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client = OpenShellReadClient::connect(fake_config(&peer), lease)
            .await
            .unwrap();
        assert_eq!(
            client.check_protected_read().await.unwrap_err(),
            ClientError::PermissionDenied
        );
        assert_eq!(
            call_names(&peer).as_slice(),
            ["Health", "GetSandboxPolicyStatus"]
        );
        drop(peer);
    }

    #[tokio::test]
    async fn requested_policy_version_mismatch_is_stale() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client = OpenShellReadClient::connect(fake_config(&peer), lease)
            .await
            .unwrap();
        assert_eq!(
            client.read_policy_status(8).await.unwrap_err(),
            ClientError::Stale
        );
        drop(peer);
    }

    #[tokio::test]
    async fn changed_leaf_pin_is_rejected_under_the_same_selected_root() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let replacement_pin = CertificatePin::from_certificate_der(b"different-leaf").unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let result = OpenShellReadClient::connect(
            ClientConfig::new(
                peer.origin().to_owned(),
                peer.selected_peer(),
                "installation-1".to_owned(),
                "workspace-1".to_owned(),
                "sandbox-1".to_owned(),
                OPEN_SHELL_VERSION.to_owned(),
                peer.root_der().to_vec(),
                replacement_pin,
            )
            .unwrap(),
            lease,
        )
        .await;
        assert!(matches!(result, Err(ClientError::Tls)));
        drop(peer);
    }

    #[tokio::test]
    async fn wrong_workspace_is_not_reclassified_as_an_invalid_cursor() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client =
            OpenShellReadClient::connect(fake_config_for_workspace(&peer, "workspace-2"), lease)
                .await
                .unwrap();
        assert_eq!(
            client.check_protected_read().await.unwrap_err(),
            ClientError::Source
        );
        assert_eq!(
            client
                .watch_recent(&WatchControl::default(), None, Duration::from_millis(100))
                .await
                .unwrap_err(),
            ClientError::Source
        );
        assert_eq!(
            call_names(&peer).as_slice(),
            ["Health", "GetSandboxPolicyStatus", "WatchSandbox"]
        );
        drop(peer);
    }

    #[tokio::test]
    async fn watch_failure_states_are_explicit_and_bounded() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client = OpenShellReadClient::connect(fake_config(&peer), lease)
            .await
            .unwrap();
        for (cursor, expected) in [
            ("out-of-range", ClientError::Gap),
            ("invalid-cursor", ClientError::Source),
            ("disconnect", ClientError::Unreachable),
            ("oversized", ClientError::Bounds),
        ] {
            assert_eq!(
                client
                    .watch_recent(
                        &WatchControl::default(),
                        Some(cursor),
                        Duration::from_millis(100)
                    )
                    .await
                    .unwrap_err(),
                expected
            );
        }
        drop(peer);
    }

    #[tokio::test]
    async fn watch_rejects_conflicting_native_sandbox_ids() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client = OpenShellReadClient::connect(fake_config(&peer), lease)
            .await
            .unwrap();
        let result = client
            .watch_recent(
                &WatchControl::default(),
                Some("foreign-sandbox"),
                Duration::from_millis(100),
            )
            .await;
        assert_eq!(result, Err(ClientError::Source));
        assert_eq!(call_names(&peer).as_slice(), ["WatchSandbox"]);
        assert!(!format!("{result:?}").contains("secret-like"));
        drop(peer);
    }

    #[tokio::test]
    async fn cancelled_watch_sends_no_provider_request() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client = OpenShellReadClient::connect(fake_config(&peer), lease)
            .await
            .unwrap();
        let control = WatchControl::default();
        control.cancel();
        assert_eq!(
            client
                .watch_recent(&control, None, Duration::from_millis(100))
                .await
                .unwrap_err(),
            ClientError::Cancelled
        );
        assert!(call_names(&peer).is_empty());
        drop(peer);
    }

    #[tokio::test]
    async fn locally_invalid_cursor_sends_no_provider_request() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client = OpenShellReadClient::connect(fake_config(&peer), lease)
            .await
            .unwrap();
        assert_eq!(
            client
                .watch_recent(
                    &WatchControl::default(),
                    Some("invalid\ncursor"),
                    Duration::from_millis(100),
                )
                .await
                .unwrap_err(),
            ClientError::InvalidCursor
        );
        assert!(call_names(&peer).is_empty());
        drop(peer);
    }

    #[tokio::test]
    async fn midstream_cancellation_stops_a_stalled_watch() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client = OpenShellReadClient::connect(fake_config(&peer), lease)
            .await
            .unwrap();
        let control = WatchControl::default();
        let delayed_cancel = control.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            delayed_cancel.cancel();
        });
        let started = Instant::now();
        assert_eq!(
            client
                .watch_recent(&control, Some("stall"), Duration::from_secs(1))
                .await
                .unwrap_err(),
            ClientError::Cancelled
        );
        assert!(started.elapsed() < Duration::from_millis(250));
        assert_eq!(call_names(&peer).as_slice(), ["WatchSandbox"]);
        drop(peer);
    }

    #[tokio::test]
    async fn cancellation_interrupts_delayed_watch_headers() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client = OpenShellReadClient::connect(fake_config(&peer), lease)
            .await
            .unwrap();
        let control = WatchControl::default();
        let delayed_cancel = control.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            delayed_cancel.cancel();
        });
        let started = Instant::now();
        assert_eq!(
            client
                .watch_recent(&control, Some("delay-headers"), Duration::from_secs(2))
                .await
                .unwrap_err(),
            ClientError::Cancelled
        );
        assert!(started.elapsed() < Duration::from_millis(250));
        assert_eq!(call_names(&peer).as_slice(), ["WatchSandbox"]);
        drop(peer);
    }

    #[tokio::test]
    async fn stalled_watch_honors_its_local_duration_bound() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client = OpenShellReadClient::connect(fake_config(&peer), lease)
            .await
            .unwrap();
        let started = Instant::now();
        let watch = client
            .watch_recent(
                &WatchControl::default(),
                Some("stall"),
                Duration::from_millis(10),
            )
            .await
            .unwrap();
        assert!(started.elapsed() < Duration::from_millis(250));
        assert_eq!(watch.events.len(), 1);
        assert_eq!(watch.continuity, Continuity::Gap);
        assert_eq!(call_names(&peer).as_slice(), ["WatchSandbox"]);
        drop(peer);
    }

    #[tokio::test]
    async fn dropped_transport_during_watch_is_visible() {
        let peer = synthetic_peer::SyntheticPeer::start(synthetic_peer::SyntheticScenario::Healthy)
            .await
            .unwrap();
        let lease = BearerLease::new(
            "test-token".to_owned(),
            Instant::now() + Duration::from_secs(5),
        )
        .unwrap();
        let mut client = OpenShellReadClient::connect(fake_config(&peer), lease)
            .await
            .unwrap();
        let watch = tokio::spawn(async move {
            client
                .watch_recent(
                    &WatchControl::default(),
                    Some("drop-transport"),
                    Duration::from_secs(30),
                )
                .await
        });
        peer.wait_watch_started().await.unwrap();
        peer.sever_transport().unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(5), watch)
            .await
            .expect("severed transport must fail before the watch duration")
            .unwrap();
        assert_eq!(outcome, Err(ClientError::Source));
        assert_eq!(call_names(&peer).as_slice(), ["WatchSandbox"]);
        peer.shutdown().await.unwrap();
    }
}
