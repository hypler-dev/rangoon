use super::*;
use crate::{CheckRequest, CloudProfile};
use rangoon_model_assistance::prepare_pack;
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer},
};
use serde_json::{Value, json};
use std::fmt::Write as _;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    time::timeout,
};
use tokio_rustls::TlsAcceptor;

const CA: &[u8] = include_bytes!("../../../fixtures/model-assistance/cloud-tls/ca.der");
const CERT: &[u8] = include_bytes!("../../../fixtures/model-assistance/cloud-tls/valid.der");
const KEY: &[u8] = include_bytes!("../../../fixtures/model-assistance/cloud-tls/valid-key.der");
const WRONG: &[u8] = include_bytes!("../../../fixtures/model-assistance/cloud-tls/wrong.der");
const WRONG_KEY: &[u8] =
    include_bytes!("../../../fixtures/model-assistance/cloud-tls/wrong-key.der");
const IO: Duration = Duration::from_secs(5);

fn client_config(trusted: bool) -> Arc<ClientConfig> {
    let mut roots = RootCertStore::empty();
    if trusted {
        roots.add(CertificateDer::from(CA)).unwrap();
    }
    Arc::new(
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    )
}
fn server_config(wrong_name: bool) -> Arc<ServerConfig> {
    let (cert, key) = if wrong_name {
        (WRONG, WRONG_KEY)
    } else {
        (CERT, KEY)
    };
    Arc::new(
        ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(
                vec![CertificateDer::from(cert)],
                PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.to_vec())),
            )
            .unwrap(),
    )
}
fn prepared() -> PreparedRequest {
    let p=CloudProfile::parse(br#"{"schemaVersion":"rangoon.cloud-profile-request.v1","profileId":"cloud-test","model":"synthetic-model-v1","maxOutputTokens":512}"#).unwrap();
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/model-assistance/local-v1.json"
    ))
    .unwrap();
    let mut r = fixture["vectors"][0]["contextRequest"].clone();
    r["target"] = json!({"profileId":p.profile_id(),"profileSha256":p.profile_sha256(),"model":p.model(),"maxOutputTokens":p.max_output_tokens()});
    PreparedRequest::new(
        &p,
        prepare_pack(&serde_json::to_vec(&r).unwrap()).unwrap(),
        &"a".repeat(64),
    )
    .unwrap()
}
fn credential() -> Credential {
    Credential::new(
        &"a".repeat(64),
        Zeroizing::new(b"synthetic-cloud-test-key".to_vec()),
    )
    .unwrap()
}
fn check_request() -> CheckRequest {
    let profile = CloudProfile::parse(
        br#"{"schemaVersion":"rangoon.cloud-profile-request.v1","profileId":"cloud-test","model":"synthetic-model-v1","maxOutputTokens":512}"#,
    )
    .unwrap();
    CheckRequest::new(&profile, &"a".repeat(64)).unwrap()
}
fn response_body() -> Vec<u8> {
    let v: Value = serde_json::from_str(include_str!(
        "../../../fixtures/model-assistance/context-v1.json"
    ))
    .unwrap();
    let proposal: Value =
        serde_json::from_str(v["vectors"][0]["validResponseJson"].as_str().unwrap()).unwrap();
    serde_json::to_vec(&json!({"id":"resp_synthetic","object":"response","model":"synthetic-observed-model-v1","status":"completed","error":null,"incomplete_details":null,"output":[{"id":"msg_synthetic","type":"message","status":"completed","role":"assistant","content":[{"type":"output_text","text":serde_json::to_string(&proposal).unwrap(),"annotations":[]}]}],"usage":{"input_tokens":100,"output_tokens":20,"total_tokens":120,"input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}})).unwrap()
}
fn raw(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
    let mut r = format!("HTTP/1.1 {status}\r\n{headers}\r\n\r\n").into_bytes();
    r.extend_from_slice(body);
    r
}
fn ok(body: &[u8]) -> Vec<u8> {
    raw(
        "200 OK",
        &format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close",
            body.len()
        ),
        body,
    )
}
async fn read_request<S: tokio::io::AsyncRead + Unpin>(stream: &mut S) -> Vec<u8> {
    let mut r = Vec::new();
    let mut buf = [0; 4096];
    loop {
        let n = timeout(IO, stream.read(&mut buf)).await.unwrap().unwrap();
        assert!(n > 0);
        r.extend_from_slice(&buf[..n]);
        assert!(r.len() < 300_000);
        if let Some(i) = r.windows(4).position(|b| b == b"\r\n\r\n") {
            let h = std::str::from_utf8(&r[..i]).unwrap();
            let length = h
                .lines()
                .find_map(|l| {
                    l.split_once(':')
                        .filter(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                        .map(|(_, v)| v.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            if r.len() == i + 4 + length {
                return r;
            }
        }
    }
}
fn assert_request(r: &[u8], p: &PreparedRequest) {
    let i = r.windows(4).position(|b| b == b"\r\n\r\n").unwrap();
    let h = std::str::from_utf8(&r[..i]).unwrap().to_ascii_lowercase();
    assert!(h.starts_with("post /v1/responses http/1.1\r\n"));
    assert!(h.contains("host: api.openai.com\r\n"));
    assert!(h.contains("authorization: bearer synthetic-cloud-test-key\r\n"));
    assert!(!h.contains("proxy-authorization"));
    assert_eq!(&r[i + 4..], p.body_json().as_bytes());
}
fn assert_check_request(r: &[u8], check: &CheckRequest) {
    let i = r.windows(4).position(|b| b == b"\r\n\r\n").unwrap();
    let headers = std::str::from_utf8(&r[..i]).unwrap().to_ascii_lowercase();
    assert!(headers.starts_with("get /v1/models/synthetic-model-v1 http/1.1\r\n"));
    assert!(headers.contains("host: api.openai.com\r\n"));
    assert!(headers.contains("authorization: bearer synthetic-cloud-test-key\r\n"));
    assert!(!headers.contains("proxy-authorization"));
    assert!(!headers.contains(check.credential_revision()));
    assert!(!headers.contains("source"));
    assert_eq!(&r[i + 4..], b"");
}
async fn roundtrip(
    p: &PreparedRequest,
    bytes: Vec<u8>,
    trusted: bool,
    wrong: bool,
) -> Result<Completion, Diagnostic> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = CloudClient {
        fixture: Some((listener.local_addr().unwrap(), client_config(trusted))),
        ..Default::default()
    };
    let server = async {
        let (s, _) = timeout(IO, listener.accept()).await.unwrap().unwrap();
        let handshake = TlsAcceptor::from(server_config(wrong)).accept(s).await;
        if !trusted || wrong {
            assert!(handshake.is_err());
            return;
        }
        let mut s = handshake.unwrap();
        let request = read_request(&mut s).await;
        assert_request(&request, p);
        let _ = s.write_all(&bytes).await;
        let _ = s.shutdown().await;
        drop(s);
        assert!(
            timeout(Duration::from_millis(20), listener.accept())
                .await
                .is_err(),
            "must not retry"
        );
    };
    let key = credential();
    let cancel = Cancellation::new();
    let (_, result) = tokio::join!(server, client.analyze(p, &key, &cancel));
    result
}
async fn check_roundtrip(
    check: &CheckRequest,
    bytes: Vec<u8>,
    trusted: bool,
    wrong: bool,
) -> Result<CheckResult, Diagnostic> {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = CloudClient {
        fixture: Some((listener.local_addr().unwrap(), client_config(trusted))),
        ..Default::default()
    };
    let server = async {
        let (socket, _) = timeout(IO, listener.accept()).await.unwrap().unwrap();
        let handshake = TlsAcceptor::from(server_config(wrong)).accept(socket).await;
        if !trusted || wrong {
            assert!(handshake.is_err());
            return;
        }
        let mut socket = handshake.unwrap();
        let request = read_request(&mut socket).await;
        assert_check_request(&request, check);
        let _ = socket.write_all(&bytes).await;
        let _ = socket.shutdown().await;
    };
    let key = credential();
    let cancellation = Cancellation::new();
    let (_, result) = tokio::join!(server, client.check(check, &key, &cancellation));
    result
}

// A child process permits hostile proxy environments without unsafe process-wide
// environment mutation or races with the Rust test harness.
#[test]
fn tls_contract_with_hostile_proxy_environment() {
    if std::env::var_os("RANGOON_SYNTHETIC_TLS_CHILD").is_some() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                timeout(Duration::from_secs(30), tls_cases()).await.unwrap();
            });
        return;
    }
    let proxy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    proxy.set_nonblocking(true).unwrap();
    let url = format!("http://{}", proxy.local_addr().unwrap());
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child
        .args([
            "--exact",
            "transport::tests::tls_contract_with_hostile_proxy_environment",
            "--nocapture",
        ])
        .env("RANGOON_SYNTHETIC_TLS_CHILD", "1")
        .env("NO_PROXY", "")
        .env("no_proxy", "");
    for name in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        child.env(name, &url);
    }
    assert!(child.status().unwrap().success());
    assert!(
        matches!(proxy.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock),
        "proxy must receive zero connections"
    );
}
async fn tls_cases() {
    let p = prepared();
    let body = response_body();
    check_cases().await;
    let completion = roundtrip(&p, ok(&body), true, false).await.unwrap();
    let receipt = serde_json::to_value(completion).unwrap();
    assert_eq!(receipt["responseSha256"], byte_digest(&body));
    assert_eq!(receipt["requestId"], p.request_id());
    assert_eq!(receipt["authority"], "none");
    assert_eq!(receipt["cost"], "unknown");
    assert!(!receipt.to_string().contains("synthetic-cloud-test-key"));
    assert_eq!(
        roundtrip(&p, ok(&body), false, false).await.err(),
        Some(Diagnostic::TlsRejected)
    );
    assert_eq!(
        roundtrip(&p, ok(&body), true, true).await.err(),
        Some(Diagnostic::TlsRejected)
    );
    for (bytes, error) in [
        (
            raw(
                "302 Found",
                "Location: https://wrong.example\r\nContent-Length: 0",
                b"",
            ),
            Diagnostic::RedirectRejected,
        ),
        (
            raw(
                "429 Too Many Requests",
                "Retry-After: 0\r\nContent-Length: 0",
                b"",
            ),
            Diagnostic::RemoteRejected,
        ),
        (
            raw(
                "200 OK",
                "Content-Type: application/json\r\nContent-Encoding: gzip\r\nContent-Length: 0",
                b"",
            ),
            Diagnostic::ResponseInvalid,
        ),
        (
            raw(
                "200 OK",
                "Content-Type: application/json\r\nContent-Length: 1048577",
                b"",
            ),
            Diagnostic::ResponseTooLarge,
        ),
        (
            raw(
                "200 OK",
                "Content-Type: application/json\r\nContent-Length: 20",
                b"{}",
            ),
            Diagnostic::ResponseIncomplete,
        ),
        (
            raw(
                "200 OK",
                "Content-Type: application/json\r\nTransfer-Encoding: chunked",
                b"2\r\n{}\r\n0\r\nX-Extra: unexpected\r\n\r\n",
            ),
            Diagnostic::ResponseInvalid,
        ),
        (
            ok(b"{\"status\":\"completed\",\"status\":\"completed\"}"),
            Diagnostic::ResponseInvalid,
        ),
    ] {
        assert_eq!(roundtrip(&p, bytes, true, false).await.err(), Some(error));
    }
    for (count, expected) in [(61, None), (62, Some(Diagnostic::ResponseInvalid))] {
        let mut extra = String::new();
        for i in 0..count {
            write!(&mut extra, "X-Test-{i}: x\r\n").unwrap();
        }
        let bytes = raw(
            "200 OK",
            &format!(
                "{extra}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close",
                body.len()
            ),
            &body,
        );
        assert_eq!(roundtrip(&p, bytes, true, false).await.err(), expected);
    }
    let huge_header = raw(
        "200 OK",
        &format!(
            "X-Test: {}\r\nContent-Type: application/json\r\nContent-Length: 0",
            "x".repeat(32_768)
        ),
        b"",
    );
    assert_eq!(
        roundtrip(&p, huge_header, true, false).await.err(),
        Some(Diagnostic::ResponseInvalid)
    );
    let oversized = raw(
        "200 OK",
        "Content-Type: application/json\r\nConnection: close",
        &vec![b' '; 1_048_577],
    );
    assert_eq!(
        roundtrip(&p, oversized, true, false).await.err(),
        Some(Diagnostic::ResponseTooLarge)
    );
    let mut forged: Value = serde_json::from_slice(&body).unwrap();
    forged["output"][0]["content"][0]["text"] = json!("{\"authority\":\"execute\"}");
    assert_eq!(
        roundtrip(&p, ok(&serde_json::to_vec(&forged).unwrap()), true, false)
            .await
            .err(),
        Some(Diagnostic::ProposalInvalid)
    );
    let key = credential();
    let c = Cancellation::new();
    c.cancel();
    assert_eq!(
        CloudClient::new().analyze(&p, &key, &c).await.err(),
        Some(Diagnostic::Cancelled)
    );
    let changed =
        Credential::new(&"b".repeat(64), Zeroizing::new(b"synthetic-other".to_vec())).unwrap();
    assert_eq!(
        CloudClient::new()
            .analyze(&p, &changed, &Cancellation::new())
            .await
            .err(),
        Some(Diagnostic::CredentialChanged)
    );
    for cancel in [false, true] {
        stalled(&p, cancel).await;
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = CloudClient {
        fixture: Some((listener.local_addr().unwrap(), client_config(true))),
        timeouts: Timeouts {
            check: Duration::from_millis(100),
            connect: Duration::from_millis(100),
            analysis: IO,
        },
    };
    let server = async {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut data = Vec::new();
        let _ = timeout(IO, socket.read_to_end(&mut data)).await.unwrap();
        assert!(
            !data
                .windows(b"synthetic-cloud-test-key".len())
                .any(|w| w == b"synthetic-cloud-test-key")
        );
        assert!(!data.windows(4).any(|w| w == b"POST"));
    };
    let cancel = Cancellation::new();
    let (_, result) = tokio::join!(server, client.analyze(&p, &key, &cancel));
    assert_eq!(result.err(), Some(Diagnostic::TimedOut));
    assert!(Flight::acquire().is_ok());
}

async fn check_cases() {
    let check = check_request();
    let metadata = serde_json::to_vec(&json!({
        "object": "model",
        "id": "synthetic-model-v1",
        "owned_by": "synthetic-owner",
        "created": 1_700_000_000u64,
        "shutdown_date": null,
    }))
    .unwrap();
    let result = check_roundtrip(&check, ok(&metadata), true, false)
        .await
        .unwrap();
    let receipt = serde_json::to_value(result).unwrap();
    assert_eq!(receipt["schemaVersion"], "rangoon.cloud-check.v1");
    assert_eq!(receipt["requestId"], check.request_id());
    assert_eq!(receipt["responseSha256"], byte_digest(&metadata));
    assert_eq!(receipt["observedModel"], "synthetic-model-v1");
    assert_eq!(receipt["ownedBy"], "synthetic-owner");
    assert_eq!(receipt["observation"], "model_visibility_only");
    assert_eq!(receipt["authority"], "none");
    assert!(!receipt.to_string().contains("synthetic-cloud-test-key"));

    let wrong =
        Credential::new(&"b".repeat(64), Zeroizing::new(b"synthetic-other".to_vec())).unwrap();
    assert_eq!(
        CloudClient::new()
            .check(&check, &wrong, &Cancellation::new())
            .await
            .err(),
        Some(Diagnostic::CredentialChanged)
    );
    let cancelled = Cancellation::new();
    cancelled.cancel();
    assert_eq!(
        CloudClient::new()
            .check(&check, &credential(), &cancelled)
            .await
            .err(),
        Some(Diagnostic::Cancelled)
    );

    let mismatched = serde_json::to_vec(&json!({
        "object": "model",
        "id": "other-model",
        "owned_by": "synthetic-owner",
        "created": 1,
    }))
    .unwrap();
    assert_eq!(
        check_roundtrip(&check, ok(&mismatched), true, false)
            .await
            .err(),
        Some(Diagnostic::ResponseInvalid)
    );
    for (bytes, expected) in [
        (
            raw(
                "302 Found",
                "Location: https://wrong.example\r\nContent-Length: 0",
                b"",
            ),
            Diagnostic::RedirectRejected,
        ),
        (
            raw(
                "404 Not Found",
                "Content-Type: application/json\r\nContent-Length: 2",
                b"{}",
            ),
            Diagnostic::RemoteRejected,
        ),
    ] {
        assert_eq!(
            check_roundtrip(&check, bytes, true, false).await.err(),
            Some(expected)
        );
    }
    stalled_check(&check).await;
}

async fn stalled_check(check: &CheckRequest) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = CloudClient {
        fixture: Some((listener.local_addr().unwrap(), client_config(true))),
        timeouts: Timeouts {
            check: Duration::from_millis(200),
            connect: IO,
            analysis: IO,
        },
    };
    let key = credential();
    let server = async {
        let (socket, _) = listener.accept().await.unwrap();
        let mut socket = TlsAcceptor::from(server_config(false))
            .accept(socket)
            .await
            .unwrap();
        assert_check_request(&read_request(&mut socket).await, check);
        assert_eq!(
            CloudClient::new()
                .check(check, &key, &Cancellation::new())
                .await
                .err(),
            Some(Diagnostic::Busy)
        );
        let mut byte = [0; 1];
        match timeout(IO, socket.read(&mut byte)).await.unwrap() {
            Ok(read) => assert_eq!(read, 0),
            Err(error) => assert!(matches!(
                error.kind(),
                std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset
            )),
        }
    };
    let cancellation = Cancellation::new();
    let (_, result) = tokio::join!(server, client.check(check, &key, &cancellation));
    assert_eq!(result.err(), Some(Diagnostic::TimedOut));
    assert!(Flight::acquire().is_ok());
}

async fn stalled(p: &PreparedRequest, cancel: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = CloudClient {
        fixture: Some((listener.local_addr().unwrap(), client_config(true))),
        timeouts: Timeouts {
            check: Duration::from_millis(100),
            connect: IO,
            analysis: Duration::from_millis(200),
        },
    };
    let c = Cancellation::new();
    let key = credential();
    let server = async {
        let (s, _) = listener.accept().await.unwrap();
        let mut s = TlsAcceptor::from(server_config(false))
            .accept(s)
            .await
            .unwrap();
        assert_request(&read_request(&mut s).await, p);
        assert_eq!(
            CloudClient::new()
                .analyze(p, &key, &Cancellation::new())
                .await
                .err(),
            Some(Diagnostic::Busy)
        );
        if cancel {
            c.cancel();
        }
        let mut buffer = [0; 1];
        match timeout(IO, s.read(&mut buffer)).await.unwrap() {
            Ok(n) => assert_eq!(n, 0),
            Err(e) => assert!(matches!(
                e.kind(),
                std::io::ErrorKind::UnexpectedEof | std::io::ErrorKind::ConnectionReset
            )),
        };
    };
    let (_, result) = tokio::join!(server, client.analyze(p, &key, &c));
    assert_eq!(
        result.err(),
        Some(if cancel {
            Diagnostic::Cancelled
        } else {
            Diagnostic::TimedOut
        })
    );
    assert!(Flight::acquire().is_ok());
}

#[test]
fn production_dns_answer_table_and_boundaries() {
    for ip in [
        "0.0.0.0",
        "10.1.2.3",
        "100.64.0.0",
        "100.127.255.255",
        "127.0.0.1",
        "169.254.2.2",
        "172.16.0.1",
        "172.31.255.255",
        "192.0.0.255",
        "192.0.2.1",
        "192.88.99.1",
        "192.168.1.1",
        "198.18.0.0",
        "198.19.255.255",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.0",
        "255.255.255.255",
        "::",
        "::1",
        "::ffff:8.8.8.8",
        "64:ff9b::808:808",
        "100::1",
        "2001::1",
        "2001:1ff::1",
        "2001:db8::1",
        "2002::1",
        "3fff::1",
        "3fff:fff::1",
        "fc00::1",
        "fe80::1",
        "ff00::1",
    ] {
        assert!(!public_address(ip.parse().unwrap()), "{ip}");
    }
    for ip in [
        "8.8.8.8",
        "100.63.255.255",
        "100.128.0.0",
        "172.15.255.255",
        "172.32.0.0",
        "192.0.1.1",
        "198.17.255.255",
        "198.20.0.0",
        "223.255.255.255",
        "2001:200::1",
        "2606:4700::1",
        "3fff:1000::1",
    ] {
        assert!(public_address(ip.parse().unwrap()), "{ip}");
    }
    let good: SocketAddr = "8.8.8.8:443".parse().unwrap();
    let bad: SocketAddr = "127.0.0.1:443".parse().unwrap();
    assert_eq!(validate_addresses(&[]), Err(Diagnostic::AddressRejected));
    assert!(validate_addresses(&[good; 16]).is_ok());
    assert_eq!(
        validate_addresses(&[good; 17]),
        Err(Diagnostic::AddressRejected)
    );
    assert_eq!(
        validate_addresses(&[good, bad]),
        Err(Diagnostic::AddressRejected)
    );
    assert_eq!(
        validate_addresses(&["8.8.8.8:80".parse().unwrap()]),
        Err(Diagnostic::AddressRejected)
    );
}
#[test]
fn credential_bounds_and_diagnostic_receipts() {
    for bytes in [
        vec![],
        vec![b'a'; 2049],
        b"contains space".to_vec(),
        vec![0],
        vec![0xff],
    ] {
        assert!(Credential::new(&"a".repeat(64), Zeroizing::new(bytes)).is_err());
    }
    for n in [1, 2048] {
        assert!(Credential::new(&"a".repeat(64), Zeroizing::new(vec![b'x'; n])).is_ok());
    }
    for revision in ["A".repeat(64), "a".repeat(63), "g".repeat(64)] {
        assert!(Credential::new(&revision, Zeroizing::new(vec![b'x'])).is_err());
    }
    let r = serde_json::to_value(Diagnostic::TlsRejected.receipt()).unwrap();
    assert_eq!(
        r,
        json!({"schemaVersion":"rangoon.cloud-diagnostic.v1","adapter":"openai-responses.v1","code":"tls_rejected","authority":"none"})
    );
}
