use crate::{Diagnostic, PreparedRequest, wire};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, StatusCode, client::conn::http1, header};
use hyper_util::rt::TokioIo;
use rangoon_domain::byte_digest;
use rangoon_model_assistance::{ValidatedResponse, validate_response};
use rustls::{ClientConfig, RootCertStore, pki_types::ServerName};
use serde::Serialize;
use std::{
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tokio::{
    net::{TcpStream, lookup_host},
    sync::watch,
};
use tokio_rustls::{TlsConnector, client::TlsStream};
use zeroize::Zeroizing;

static BUSY: AtomicBool = AtomicBool::new(false);

/// A monotonic cancellation signal. It never resets and contains no request data.
#[derive(Clone)]
pub struct Cancellation(watch::Sender<bool>);

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

impl Cancellation {
    pub fn new() -> Self {
        Self(watch::channel(false).0)
    }
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
    pub fn is_cancelled(&self) -> bool {
        *self.0.borrow()
    }

    async fn cancelled(&self) {
        let mut receiver = self.0.subscribe();
        loop {
            if *receiver.borrow_and_update() {
                return;
            }
            if receiver.changed().await.is_err() {
                return;
            }
        }
    }
}

/// Explicit operations only; all instances share a single process-wide flight guard.
#[derive(Clone, Default)]
pub struct CloudClient {
    timeouts: Timeouts,
    #[cfg(test)]
    fixture: Option<(SocketAddr, Arc<ClientConfig>)>,
}

#[derive(Clone)]
struct Timeouts {
    connect: Duration,
    analysis: Duration,
}
impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            analysis: Duration::from_secs(120),
        }
    }
}

struct Flight;
impl Flight {
    fn acquire() -> Result<Self, Diagnostic> {
        BUSY.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Self)
            .map_err(|_| Diagnostic::Busy)
    }
}
impl Drop for Flight {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::Release);
    }
}

/// Explicitly supplied native material. This type cannot be logged or serialized.
/// It does not prove that the caller read the OS store or collected consent.
pub struct Credential {
    revision: String,
    secret: Zeroizing<Vec<u8>>,
}
impl Credential {
    pub fn new(revision: &str, secret: Zeroizing<Vec<u8>>) -> Result<Self, Diagnostic> {
        if !crate::request::valid_revision(revision)
            || !(1..=2048).contains(&secret.len())
            || !secret.iter().all(|byte| (0x21..=0x7e).contains(byte))
        {
            return Err(Diagnostic::InvalidCredential);
        }
        Ok(Self {
            revision: revision.to_owned(),
            secret,
        })
    }
}

/// Validated advisory proposals with locally bound metadata and untrusted usage.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Completion {
    schema_version: &'static str,
    request_id: String,
    profile_sha256: String,
    pack_id: String,
    response_sha256: String,
    observed_model: String,
    provider_response_id: String,
    usage_source: &'static str,
    usage: Option<wire::Usage>,
    proposals: ValidatedResponse,
    processing_location: &'static str,
    authority: &'static str,
    cost: &'static str,
    retention: &'static str,
}

impl CloudClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sends exact prepared bytes. The application must establish consent first.
    pub async fn analyze(
        &self,
        prepared: &PreparedRequest,
        credential: &Credential,
        cancellation: &Cancellation,
    ) -> Result<Completion, Diagnostic> {
        if prepared.credential_revision() != credential.revision {
            return Err(Diagnostic::CredentialChanged);
        }
        self.run(cancellation, self.timeouts.analysis, async {
            let bytes = self
                .exchange(prepared.body_json(), credential, 1_048_576)
                .await?;
            let chat = wire::response(&bytes, prepared.profile.max_output_tokens())?;
            let proposals = validate_response(&prepared.pack, chat.content.as_bytes())
                .map_err(|_| Diagnostic::ProposalInvalid)?;
            Ok(Completion {
                schema_version: "rangoon.cloud-completion.v1",
                request_id: prepared.request_id().to_owned(),
                profile_sha256: prepared.profile.profile_sha256().to_owned(),
                pack_id: prepared.pack.pack_id().to_owned(),
                response_sha256: byte_digest(&bytes),
                observed_model: chat.model,
                provider_response_id: chat.id,
                usage_source: "provider_reported",
                usage: chat.usage,
                proposals,
                processing_location: "unknown",
                authority: "none",
                cost: "unknown",
                retention: "unknown",
            })
        })
        .await
    }

    async fn run<T>(
        &self,
        cancellation: &Cancellation,
        deadline: Duration,
        operation: impl Future<Output = Result<T, Diagnostic>>,
    ) -> Result<T, Diagnostic> {
        if cancellation.is_cancelled() {
            return Err(Diagnostic::Cancelled);
        }
        let _flight = Flight::acquire()?;
        let result = tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(Diagnostic::Cancelled),
            result = tokio::time::timeout(deadline, operation) => result.unwrap_or(Err(Diagnostic::TimedOut)),
        };
        if cancellation.is_cancelled() {
            Err(Diagnostic::Cancelled)
        } else {
            result
        }
    }

    async fn connect(&self) -> Result<TlsStream<TcpStream>, Diagnostic> {
        #[cfg(test)]
        if let Some((address, config)) = &self.fixture {
            let stream = TcpStream::connect(address)
                .await
                .map_err(|_| Diagnostic::ConnectionFailed)?;
            return TlsConnector::from(config.clone())
                .connect(
                    ServerName::try_from("api.openai.com").map_err(|_| Diagnostic::TlsRejected)?,
                    stream,
                )
                .await
                .map_err(|_| Diagnostic::TlsRejected);
        }
        let addresses: Vec<_> = lookup_host(("api.openai.com", 443))
            .await
            .map_err(|_| Diagnostic::ConnectionFailed)?
            .take(17)
            .collect();
        validate_addresses(&addresses)?;
        let stream = TcpStream::connect(addresses[0])
            .await
            .map_err(|_| Diagnostic::ConnectionFailed)?;
        let config = tls_config()?;
        TlsConnector::from(Arc::new(config))
            .connect(
                ServerName::try_from("api.openai.com").map_err(|_| Diagnostic::TlsRejected)?,
                stream,
            )
            .await
            .map_err(|_| Diagnostic::TlsRejected)
    }

    async fn exchange(
        &self,
        body: &str,
        credential: &Credential,
        cap: usize,
    ) -> Result<Vec<u8>, Diagnostic> {
        let stream = tokio::time::timeout(self.timeouts.connect, self.connect())
            .await
            .map_err(|_| Diagnostic::TimedOut)??;
        let mut builder = http1::Builder::new();
        builder
            .max_headers(64)
            .max_buf_size(32_768)
            .allow_spaces_after_header_name_in_responses(false)
            .allow_obsolete_multiline_headers_in_responses(false)
            .ignore_invalid_headers_in_responses(false);
        let (mut sender, connection) = builder
            .handshake(TokioIo::new(stream))
            .await
            .map_err(http_error)?;
        // Construct authorization only after the fixed-name TLS handshake succeeds.
        let mut bearer = Zeroizing::new(Vec::with_capacity(7 + credential.secret.len()));
        bearer.extend_from_slice(b"Bearer ");
        bearer.extend_from_slice(&credential.secret);
        let mut authorization =
            header::HeaderValue::from_bytes(&bearer).map_err(|_| Diagnostic::InvalidCredential)?;
        authorization.set_sensitive(true);
        let request = Request::builder()
            .method("POST")
            .uri("/v1/responses")
            .header(header::HOST, "api.openai.com")
            .header(header::AUTHORIZATION, authorization)
            .header(header::ACCEPT, "application/json")
            .header(header::ACCEPT_ENCODING, "identity")
            .header(header::CONNECTION, "close")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CONTENT_LENGTH, body.len())
            .body(Full::new(Bytes::copy_from_slice(body.as_bytes())))
            .map_err(|_| Diagnostic::ResponseInvalid)?;
        let operation = async {
            let response = sender.send_request(request).await.map_err(http_error)?;
            if response.status().is_redirection() {
                return Err(Diagnostic::RedirectRejected);
            }
            if response.status() != StatusCode::OK {
                return Err(Diagnostic::RemoteRejected);
            }
            validate_headers(response.headers(), cap)?;
            let mut incoming = response.into_body();
            let mut bytes = Vec::new();
            while let Some(frame) = incoming.frame().await {
                let frame = frame.map_err(http_error)?;
                let data = frame.into_data().map_err(|_| Diagnostic::ResponseInvalid)?;
                if data.len() > cap.saturating_sub(bytes.len()) {
                    return Err(Diagnostic::ResponseTooLarge);
                }
                bytes.extend_from_slice(&data);
            }
            Ok(bytes)
        };
        tokio::pin!(operation);
        tokio::pin!(connection);
        // Both futures live in this stack frame; cancellation drops the socket and
        // its HTTP driver together. No detached task can outlive the request.
        tokio::select! {
            biased;
            result = &mut operation => result,
            result = &mut connection => {
                result.map_err(http_error)?;
                operation.await
            }
        }
    }
}

fn http_error(error: hyper::Error) -> Diagnostic {
    if error.is_parse() {
        Diagnostic::ResponseInvalid
    } else {
        // Once TCP connects, a failed HTTP exchange (including a body decoder
        // EOF wrapped by Hyper) cannot establish a complete response.
        Diagnostic::ResponseIncomplete
    }
}

fn validate_headers(headers: &hyper::HeaderMap, cap: usize) -> Result<(), Diagnostic> {
    let mut types = headers.get_all(header::CONTENT_TYPE).iter();
    let value = types
        .next()
        .and_then(|v| v.to_str().ok())
        .ok_or(Diagnostic::ResponseInvalid)?;
    if types.next().is_some() {
        return Err(Diagnostic::ResponseInvalid);
    }
    let mut parts = value.split(';').map(str::trim);
    if !parts
        .next()
        .is_some_and(|v| v.eq_ignore_ascii_case("application/json"))
    {
        return Err(Diagnostic::ResponseInvalid);
    }
    if let Some(parameter) = parts.next() {
        let Some((key, value)) = parameter.split_once('=') else {
            return Err(Diagnostic::ResponseInvalid);
        };
        if !key.trim().eq_ignore_ascii_case("charset")
            || !value.trim().eq_ignore_ascii_case("utf-8")
            || parts.next().is_some()
        {
            return Err(Diagnostic::ResponseInvalid);
        }
    }
    let mut encodings = headers.get_all(header::CONTENT_ENCODING).iter();
    if let Some(value) = encodings.next() {
        if !value.as_bytes().eq_ignore_ascii_case(b"identity") || encodings.next().is_some() {
            return Err(Diagnostic::ResponseInvalid);
        }
    }
    if let Some(value) = headers.get(header::CONTENT_LENGTH) {
        let length: u64 = value
            .to_str()
            .ok()
            .and_then(|s| s.parse().ok())
            .ok_or(Diagnostic::ResponseInvalid)?;
        if length > cap as u64 {
            return Err(Diagnostic::ResponseTooLarge);
        }
    }
    Ok(())
}

fn tls_config() -> Result<ClientConfig, Diagnostic> {
    let roots = RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let mut config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|_| Diagnostic::TlsRejected)?
            .with_root_certificates(roots)
            .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

fn validate_addresses(addresses: &[SocketAddr]) -> Result<(), Diagnostic> {
    if addresses.is_empty()
        || addresses.len() > 16
        || addresses
            .iter()
            .any(|address| address.port() != 443 || !public_address(address.ip()))
    {
        Err(Diagnostic::AddressRejected)
    } else {
        Ok(())
    }
}

fn public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(matches!(a, 0 | 10 | 127 | 224..=255)
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192
                    && (b == 168 || (b == 0 && matches!(c, 0 | 2)) || (b == 88 && c == 99)))
                || (a == 198 && (matches!(b, 18 | 19) || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            // Accept only global-unicast 2000::/3, excluding protocol-assignment,
            // transition and documentation ranges. IPv4-mapped/NAT64 fail this test.
            s[0] & 0xe000 == 0x2000
                && !(s[0] == 0x2001 && (s[1] < 0x200 || s[1] == 0xdb8))
                && s[0] != 0x2002
                && !(s[0] == 0x3fff && s[1] < 0x1000)
        }
    }
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
