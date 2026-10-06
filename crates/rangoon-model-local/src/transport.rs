use crate::{Diagnostic, LocalProfile, PreparedRequest, wire};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{Request, StatusCode, client::conn::http1, header};
use hyper_util::rt::TokioIo;
use rangoon_domain::byte_digest;
use rangoon_model_assistance::{ValidatedResponse, validate_response};
use serde::Serialize;
use std::{
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use tokio::{net::TcpStream, sync::watch};

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
pub struct LocalClient {
    timeouts: Timeouts,
}

#[derive(Clone)]
struct Timeouts {
    connect: Duration,
    check: Duration,
    analysis: Duration,
}
impl Default for Timeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(3),
            check: Duration::from_secs(5),
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

/// Unauthenticated source-free protocol observation, not upload permission.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckResult {
    schema_version: &'static str,
    profile_sha256: String,
    server_version: String,
    processing_location: &'static str,
    authority: &'static str,
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
    server_created_at: String,
    usage_source: &'static str,
    usage: wire::Usage,
    proposals: ValidatedResponse,
    processing_location: &'static str,
    authority: &'static str,
}

impl LocalClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// Performs one explicit GET with no model name or source content.
    pub async fn check(
        &self,
        profile: &LocalProfile,
        cancellation: &Cancellation,
    ) -> Result<CheckResult, Diagnostic> {
        self.run(cancellation, self.timeouts.check, async {
            let bytes = self.exchange(profile, "/api/version", None, 1024).await?;
            let version = wire::version(&bytes)?;
            Ok(CheckResult {
                schema_version: "rangoon.local-check.v1",
                profile_sha256: profile.profile_sha256().to_owned(),
                server_version: version,
                processing_location: "unknown",
                authority: "none",
            })
        })
        .await
    }

    /// Sends exact prepared bytes. The application must establish consent first.
    pub async fn analyze(
        &self,
        prepared: &PreparedRequest,
        cancellation: &Cancellation,
    ) -> Result<Completion, Diagnostic> {
        self.run(cancellation, self.timeouts.analysis, async {
            let bytes = self
                .exchange(
                    &prepared.profile,
                    "/api/chat",
                    Some(prepared.body_json()),
                    1_048_576,
                )
                .await?;
            let chat = wire::chat(
                &bytes,
                prepared.profile.model(),
                prepared.profile.max_output_tokens(),
            )?;
            let proposals = validate_response(&prepared.pack, chat.content.as_bytes())
                .map_err(|_| Diagnostic::ProposalInvalid)?;
            Ok(Completion {
                schema_version: "rangoon.local-completion.v1",
                request_id: prepared.request_id().to_owned(),
                profile_sha256: prepared.profile.profile_sha256().to_owned(),
                pack_id: prepared.pack.pack_id().to_owned(),
                response_sha256: byte_digest(&bytes),
                observed_model: chat.model,
                server_created_at: chat.created_at,
                usage_source: "server_reported",
                usage: chat.usage,
                proposals,
                processing_location: "unknown",
                authority: "none",
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

    async fn exchange(
        &self,
        profile: &LocalProfile,
        path: &'static str,
        body: Option<&str>,
        cap: usize,
    ) -> Result<Vec<u8>, Diagnostic> {
        let stream = tokio::time::timeout(
            self.timeouts.connect,
            TcpStream::connect(profile.socket_addr()),
        )
        .await
        .map_err(|_| Diagnostic::TimedOut)?
        .map_err(|_| Diagnostic::ConnectionFailed)?;
        let mut builder = http1::Builder::new();
        builder
            .max_headers(32)
            .max_buf_size(16_384)
            .allow_spaces_after_header_name_in_responses(false)
            .allow_obsolete_multiline_headers_in_responses(false)
            .ignore_invalid_headers_in_responses(false);
        let (mut sender, connection) = builder
            .handshake(TokioIo::new(stream))
            .await
            .map_err(http_error)?;
        let mut request = Request::builder()
            .method(if body.is_some() { "POST" } else { "GET" })
            .uri(path)
            .header(header::HOST, profile.host_header())
            .header(header::ACCEPT, "application/json")
            .header(header::ACCEPT_ENCODING, "identity")
            .header(header::CONNECTION, "close");
        if let Some(content) = body {
            request = request
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::CONTENT_LENGTH, content.len());
        }
        let request = request
            .body(Full::new(Bytes::copy_from_slice(
                body.unwrap_or_default().as_bytes(),
            )))
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::AsyncReadExt, net::TcpListener};

    fn profile(port: u16) -> LocalProfile {
        LocalProfile::parse(format!(r#"{{"schemaVersion":"rangoon.local-profile-request.v1","profileId":"test","host":"127.0.0.1","port":{port},"model":"synthetic:v1","maxOutputTokens":16}}"#).as_bytes()).unwrap()
    }

    #[tokio::test]
    async fn stalled_response_times_out_once_closes_socket_and_releases_global_guard() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let profile = profile(listener.local_addr().unwrap().port());
        let client = LocalClient {
            timeouts: Timeouts {
                connect: Duration::from_millis(100),
                check: Duration::from_millis(100),
                analysis: Duration::from_millis(100),
            },
        };
        let server = async {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            socket.read_to_end(&mut bytes).await.unwrap();
            assert!(bytes.starts_with(b"GET /api/version HTTP/1.1\r\n"));
            assert!(
                tokio::time::timeout(Duration::from_millis(50), listener.accept())
                    .await
                    .is_err(),
                "request must not retry"
            );
        };
        let operation = async {
            let error = client.check(&profile, &Cancellation::new()).await.err();
            assert_eq!(error, Some(Diagnostic::TimedOut));
            let guard = Flight::acquire().expect("timeout must release process guard");
            drop(guard);
        };
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(server, operation);
        })
        .await
        .unwrap();
    }

    #[test]
    fn header_contract_rejects_ambiguous_media_and_compression() {
        for value in [
            "application/json",
            "Application/JSON; Charset=UTF-8",
            "application/json ; charset = utf-8",
        ] {
            let mut headers = hyper::HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, value.parse().unwrap());
            assert_eq!(validate_headers(&headers, 1024), Ok(()));
        }
        for value in [
            "text/json",
            "application/json;",
            "application/json; charset=\"utf-8\"",
            "application/json; foo=bar",
            "application/json; charset=utf-8; charset=utf-8",
        ] {
            let mut headers = hyper::HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, value.parse().unwrap());
            assert_eq!(
                validate_headers(&headers, 1024),
                Err(Diagnostic::ResponseInvalid)
            );
        }
        let mut headers = hyper::HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
        headers.insert(header::CONTENT_ENCODING, "gzip".parse().unwrap());
        assert_eq!(
            validate_headers(&headers, 1024),
            Err(Diagnostic::ResponseInvalid)
        );
        headers.insert(header::CONTENT_ENCODING, "identity".parse().unwrap());
        headers.insert(header::CONTENT_LENGTH, "1024".parse().unwrap());
        assert_eq!(validate_headers(&headers, 1024), Ok(()));
        headers.insert(header::CONTENT_LENGTH, "1025".parse().unwrap());
        assert_eq!(
            validate_headers(&headers, 1024),
            Err(Diagnostic::ResponseTooLarge)
        );
    }
}
