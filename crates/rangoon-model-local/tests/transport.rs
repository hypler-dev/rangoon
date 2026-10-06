//! Synthetic loopback transport contract tests. No real model process is used.

use rangoon_model_assistance::prepare_pack;
use rangoon_model_local::{Cancellation, Diagnostic, LocalClient, LocalProfile, PreparedRequest};
use serde_json::{Value, json};
use std::{net::SocketAddr, sync::OnceLock, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Mutex,
    time::timeout,
};

const IO_LIMIT: Duration = Duration::from_secs(2);
const TEST_LOCK_LIMIT: Duration = Duration::from_secs(30);

// `LocalClient` intentionally has one process-wide flight guard. Keep these tests
// serialized even when the Rust harness runs integration tests concurrently.
static TRANSPORT_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn lock() -> &'static Mutex<()> {
    TRANSPORT_TEST_LOCK.get_or_init(|| Mutex::new(()))
}

fn local_profile(address: SocketAddr) -> LocalProfile {
    let host = match address {
        SocketAddr::V4(_) => "127.0.0.1",
        SocketAddr::V6(_) => "::1",
    };
    let raw = json!({
        "schemaVersion": "rangoon.local-profile-request.v1",
        "profileId": "transport-test",
        "host": host,
        "port": address.port(),
        "model": "fixture-model:v1",
        "maxOutputTokens": 512,
    });
    LocalProfile::parse(&serde_json::to_vec(&raw).unwrap()).unwrap()
}

fn prepared(profile: &LocalProfile) -> PreparedRequest {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/model-assistance/local-v1.json"
    ))
    .unwrap();
    let mut request = fixture["vectors"][0]["contextRequest"].clone();
    request["target"] = json!({
        "profileId": profile.profile_id(),
        "profileSha256": profile.profile_sha256(),
        "model": profile.model(),
        "maxOutputTokens": profile.max_output_tokens(),
    });
    PreparedRequest::new(
        profile,
        prepare_pack(&serde_json::to_vec(&request).unwrap()).unwrap(),
    )
    .unwrap()
}

fn valid_proposal() -> String {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../fixtures/model-assistance/context-v1.json"
    ))
    .unwrap();
    fixture["vectors"][0]["validResponseJson"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn response(status: &str, headers: &str, body: &[u8]) -> Vec<u8> {
    let mut raw = format!("HTTP/1.1 {status}\r\n{headers}\r\n\r\n").into_bytes();
    raw.extend_from_slice(body);
    raw
}

fn json_response(body: &[u8]) -> Vec<u8> {
    response(
        "200 OK",
        &format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close",
            body.len()
        ),
        body,
    )
}

async fn accept(listener: &TcpListener) -> TcpStream {
    timeout(IO_LIMIT, listener.accept())
        .await
        .expect("server accept timed out")
        .expect("synthetic accept failed")
        .0
}

async fn write_raw(stream: &mut TcpStream, raw: &[u8]) {
    match timeout(IO_LIMIT, stream.write_all(raw)).await {
        Ok(Ok(())) => {}
        Ok(Err(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
            ) => {}
        Ok(Err(error)) => panic!("synthetic write failed: {error}"),
        Err(_) => panic!("server write timed out"),
    }
    match timeout(IO_LIMIT, stream.shutdown()).await {
        Ok(Ok(())) => {}
        Ok(Err(error))
            if matches!(
                error.kind(),
                std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::NotConnected
            ) => {}
        Ok(Err(error)) => panic!("synthetic shutdown failed: {error}"),
        Err(_) => panic!("server shutdown timed out"),
    }
}

async fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut received = Vec::new();
    let mut buffer = [0_u8; 4096];
    let body_end;
    loop {
        let read = timeout(IO_LIMIT, stream.read(&mut buffer))
            .await
            .expect("request read timed out")
            .expect("synthetic read failed");
        assert_ne!(read, 0, "request ended before headers");
        received.extend_from_slice(&buffer[..read]);
        if let Some(index) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            body_end = index + 4;
            break;
        }
        assert!(
            received.len() < 16 * 1024,
            "request headers unexpectedly large"
        );
    }
    let head = std::str::from_utf8(&received[..body_end]).unwrap();
    let length = head
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        })
        .map(|(_, value)| value.trim().parse::<usize>().unwrap())
        .unwrap_or(0);
    while received.len() < body_end + length {
        let read = timeout(IO_LIMIT, stream.read(&mut buffer))
            .await
            .expect("request body read timed out")
            .expect("synthetic body read failed");
        assert_ne!(read, 0, "request ended before declared body");
        received.extend_from_slice(&buffer[..read]);
    }
    received
}

async fn assert_check_error(case: &str, raw_response: Vec<u8>, expected: Diagnostic) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let profile = local_profile(listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut stream = accept(&listener).await;
        let _request = read_request(&mut stream).await;
        write_raw(&mut stream, &raw_response).await;
    });
    let result = LocalClient::new()
        .check(&profile, &Cancellation::new())
        .await;
    assert_eq!(
        result.err(),
        Some(expected),
        "unexpected diagnostic for {case}"
    );
    timeout(IO_LIMIT, server)
        .await
        .expect("server task timed out")
        .unwrap();
}

async fn assert_analyze_error(case: &str, raw_response: Vec<u8>, expected: Diagnostic) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let profile = local_profile(listener.local_addr().unwrap());
    let request = prepared(&profile);
    let server = tokio::spawn(async move {
        let mut stream = accept(&listener).await;
        let _request = read_request(&mut stream).await;
        write_raw(&mut stream, &raw_response).await;
    });
    let result = LocalClient::new()
        .analyze(&request, &Cancellation::new())
        .await;
    assert_eq!(
        result.err(),
        Some(expected),
        "unexpected diagnostic for {case}"
    );
    timeout(IO_LIMIT, server)
        .await
        .expect("server task timed out")
        .unwrap();
}

#[tokio::test]
async fn source_free_get_and_exact_post_use_numeric_ipv4_loopback() {
    let _guard = timeout(TEST_LOCK_LIMIT, lock().lock())
        .await
        .expect("test lock timed out");
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let profile = local_profile(listener.local_addr().unwrap());
    let request = prepared(&profile);
    let expected_host = format!("127.0.0.1:{}", listener.local_addr().unwrap().port());
    let expected_body = request.body_json().as_bytes().to_vec();
    let proposal = valid_proposal();
    let chat = serde_json::to_vec(&json!({
        "model": profile.model(),
        "created_at": "synthetic-local-test",
        "message": {"role": "assistant", "content": proposal},
        "done": true,
        "done_reason": "stop",
        "eval_count": 1,
    }))
    .unwrap();
    let server = tokio::spawn(async move {
        let mut first = accept(&listener).await;
        let get = read_request(&mut first).await;
        write_raw(&mut first, &json_response(br#"{"version":"synthetic-v1"}"#)).await;
        let mut second = accept(&listener).await;
        let post = read_request(&mut second).await;
        write_raw(&mut second, &json_response(&chat)).await;
        (get, post)
    });

    let client = LocalClient::new();
    let check = client.check(&profile, &Cancellation::new()).await.unwrap();
    let completion = client
        .analyze(&request, &Cancellation::new())
        .await
        .unwrap();
    let (get, post) = timeout(IO_LIMIT, server)
        .await
        .expect("server task timed out")
        .unwrap();
    let get = String::from_utf8(get).unwrap();
    let post_head_end = post
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap()
        + 4;
    let post_head = std::str::from_utf8(&post[..post_head_end]).unwrap();

    assert_eq!(
        get,
        format!(
            "GET /api/version HTTP/1.1\r\nhost: {expected_host}\r\naccept: application/json\r\naccept-encoding: identity\r\nconnection: close\r\n\r\n"
        )
    );
    assert_eq!(
        post_head,
        format!(
            "POST /api/chat HTTP/1.1\r\nhost: {expected_host}\r\naccept: application/json\r\naccept-encoding: identity\r\nconnection: close\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n",
            expected_body.len()
        )
    );
    assert_eq!(&post[post_head_end..], expected_body.as_slice());
    assert_eq!(
        serde_json::to_value(check).unwrap()["serverVersion"],
        "synthetic-v1"
    );
    let completion = serde_json::to_value(completion).unwrap();
    assert_eq!(completion["requestId"], request.request_id());
    assert_eq!(completion["profileSha256"], profile.profile_sha256());
    assert_eq!(completion["authority"], "none");
}

#[tokio::test]
async fn numeric_ipv6_loopback_uses_bracketed_host_header() {
    let _guard = timeout(TEST_LOCK_LIMIT, lock().lock())
        .await
        .expect("test lock timed out");
    let listener = TcpListener::bind(("::1", 0))
        .await
        .expect("IPv6 loopback listener unavailable");
    let profile = local_profile(listener.local_addr().unwrap());
    let expected_host = format!("[::1]:{}", listener.local_addr().unwrap().port());
    let server = tokio::spawn(async move {
        let mut stream = accept(&listener).await;
        let request = read_request(&mut stream).await;
        write_raw(&mut stream, &json_response(br#"{"version":"ipv6"}"#)).await;
        request
    });
    LocalClient::new()
        .check(&profile, &Cancellation::new())
        .await
        .unwrap();
    let request = String::from_utf8(
        timeout(IO_LIMIT, server)
            .await
            .expect("server task timed out")
            .unwrap(),
    )
    .unwrap();
    assert!(request.starts_with(&format!(
        "GET /api/version HTTP/1.1\r\nhost: {expected_host}\r\n"
    )));
}

#[tokio::test]
async fn refused_and_pre_cancelled_checks_make_exactly_zero_server_attempts() {
    let _guard = timeout(TEST_LOCK_LIMIT, lock().lock())
        .await
        .expect("test lock timed out");
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let refused_profile = local_profile(listener.local_addr().unwrap());
    drop(listener);
    assert!(matches!(
        LocalClient::new()
            .check(&refused_profile, &Cancellation::new())
            .await,
        Err(Diagnostic::ConnectionFailed)
    ));

    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let profile = local_profile(listener.local_addr().unwrap());
    let cancellation = Cancellation::new();
    cancellation.cancel();
    assert!(matches!(
        LocalClient::new().check(&profile, &cancellation).await,
        Err(Diagnostic::Cancelled)
    ));
    assert!(
        timeout(Duration::from_millis(250), listener.accept())
            .await
            .is_err(),
        "pre-cancel unexpectedly connected"
    );

    // Bind before every adapter value exists. Preparation and cancellation must
    // remain local operations, with no automatic analysis attempt or retry.
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let profile = local_profile(listener.local_addr().unwrap());
    let request = prepared(&profile);
    let client = LocalClient::new();
    let cancellation = Cancellation::new();
    cancellation.cancel();
    assert!(matches!(
        client.analyze(&request, &cancellation).await,
        Err(Diagnostic::Cancelled)
    ));
    assert!(
        timeout(Duration::from_millis(250), listener.accept())
            .await
            .is_err(),
        "pre-cancelled analysis unexpectedly connected"
    );
}

#[tokio::test]
async fn busy_is_process_wide_then_cancellation_and_drop_close_connections_and_release_guard() {
    let _guard = timeout(TEST_LOCK_LIMIT, lock().lock())
        .await
        .expect("test lock timed out");
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let profile = local_profile(listener.local_addr().unwrap());
    let cancellation = Cancellation::new();
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut stream = accept(&listener).await;
        let _request = read_request(&mut stream).await;
        let _ = accepted_tx.send(());
        let mut byte = [0_u8; 1];
        let closed = timeout(IO_LIMIT, stream.read(&mut byte))
            .await
            .expect("cancelled stream did not close")
            .expect("server read failed");
        assert_eq!(closed, 0, "cancelled request left socket open");
    });
    let first_client = LocalClient::new();
    let first_profile = profile.clone();
    let first_cancel = cancellation.clone();
    let first =
        tokio::spawn(async move { first_client.check(&first_profile, &first_cancel).await });
    timeout(IO_LIMIT, accepted_rx)
        .await
        .expect("first request not accepted")
        .unwrap();
    assert!(matches!(
        LocalClient::new()
            .check(&profile, &Cancellation::new())
            .await,
        Err(Diagnostic::Busy)
    ));
    cancellation.cancel();
    assert!(matches!(
        timeout(IO_LIMIT, first)
            .await
            .expect("cancelled request task timed out")
            .unwrap(),
        Err(Diagnostic::Cancelled)
    ));
    timeout(IO_LIMIT, server)
        .await
        .expect("cancel server task timed out")
        .unwrap();

    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let profile = local_profile(listener.local_addr().unwrap());
    let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let mut stream = accept(&listener).await;
        let _request = read_request(&mut stream).await;
        let _ = accepted_tx.send(());
        let mut byte = [0_u8; 1];
        assert_eq!(
            timeout(IO_LIMIT, stream.read(&mut byte))
                .await
                .expect("dropped stream did not close")
                .expect("server read failed"),
            0
        );
    });
    let drop_client = LocalClient::new();
    let drop_profile = profile.clone();
    let dropped =
        tokio::spawn(async move { drop_client.check(&drop_profile, &Cancellation::new()).await });
    timeout(IO_LIMIT, accepted_rx)
        .await
        .expect("dropped request not accepted")
        .unwrap();
    dropped.abort();
    let _ = dropped.await;
    timeout(IO_LIMIT, server)
        .await
        .expect("drop server task timed out")
        .unwrap();

    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let profile = local_profile(listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut stream = accept(&listener).await;
        let _request = read_request(&mut stream).await;
        write_raw(&mut stream, &json_response(br#"{"version":"released"}"#)).await;
    });
    LocalClient::new()
        .check(&profile, &Cancellation::new())
        .await
        .unwrap();
    timeout(IO_LIMIT, server)
        .await
        .expect("released server task timed out")
        .unwrap();
}

#[tokio::test]
async fn closed_http_failures_reject_redirect_status_headers_and_framing() {
    let _guard = timeout(TEST_LOCK_LIMIT, lock().lock())
        .await
        .expect("test lock timed out");
    assert_check_error(
        "redirect",
        response(
            "302 Found",
            "Location: /api/version\r\nContent-Length: 0\r\nConnection: close",
            b"",
        ),
        Diagnostic::RedirectRejected,
    )
    .await;
    assert_check_error(
        "non-200",
        response(
            "503 Service Unavailable",
            "Content-Length: 0\r\nConnection: close",
            b"",
        ),
        Diagnostic::RemoteRejected,
    )
    .await;
    assert_check_error(
        "wrong content type",
        response(
            "200 OK",
            "Content-Type: text/plain\r\nContent-Length: 2\r\nConnection: close",
            b"{}",
        ),
        Diagnostic::ResponseInvalid,
    )
    .await;
    assert_check_error("duplicate content type", response("200 OK", "Content-Type: application/json\r\nContent-Type: application/json\r\nContent-Length: 2\r\nConnection: close", b"{}"), Diagnostic::ResponseInvalid).await;
    assert_check_error("quoted charset", response("200 OK", "Content-Type: application/json; charset=\"utf-8\"\r\nContent-Length: 2\r\nConnection: close", b"{}"), Diagnostic::ResponseInvalid).await;
    assert_check_error("compressed encoding", response("200 OK", "Content-Type: application/json\r\nContent-Encoding: gzip\r\nContent-Length: 2\r\nConnection: close", b"{}"), Diagnostic::ResponseInvalid).await;
    assert_check_error("duplicate encoding", response("200 OK", "Content-Type: application/json\r\nContent-Encoding: identity\r\nContent-Encoding: identity\r\nContent-Length: 2\r\nConnection: close", b"{}"), Diagnostic::ResponseInvalid).await;

    let many_headers = (0..33)
        .map(|index| format!("X-Test-{index}: x\r\n"))
        .collect::<String>();
    let raw = response(
        "200 OK",
        &format!(
            "Content-Type: application/json\r\n{many_headers}Content-Length: 2\r\nConnection: close"
        ),
        b"{}",
    );
    let result = check_result(raw).await;
    assert!(matches!(
        result,
        Err(Diagnostic::ResponseInvalid | Diagnostic::ResponseTooLarge)
    ));
    let too_wide = response(
        "200 OK",
        &format!(
            "Content-Type: application/json\r\nX-Pad: {}\r\nContent-Length: 2\r\nConnection: close",
            "x".repeat(17_000)
        ),
        b"{}",
    );
    let result = check_result(too_wide).await;
    assert!(matches!(
        result,
        Err(Diagnostic::ResponseInvalid | Diagnostic::ResponseTooLarge)
    ));

    assert_check_error(
        "declared body too large",
        response(
            "200 OK",
            "Content-Type: application/json\r\nContent-Length: 1025\r\nConnection: close",
            b"",
        ),
        Diagnostic::ResponseTooLarge,
    )
    .await;
    let chunk = vec![b'x'; 1025];
    let chunked = response(
        "200 OK",
        "Content-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close",
        format!("{:X}\r\n", chunk.len()).as_bytes(),
    );
    let mut chunked = chunked;
    chunked.extend_from_slice(&chunk);
    chunked.extend_from_slice(b"\r\n0\r\n\r\n");
    assert_check_error(
        "chunked body too large",
        chunked,
        Diagnostic::ResponseTooLarge,
    )
    .await;
    let with_trailer = response(
        "200 OK",
        "Content-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close",
        b"15\r\n{\"version\":\"chunked\"}\r\n0\r\nX-Trailer: reject\r\n\r\n",
    );
    assert_check_error("chunked trailer", with_trailer, Diagnostic::ResponseInvalid).await;
    assert_check_error(
        "truncated framing",
        response(
            "200 OK",
            "Content-Type: application/json\r\nContent-Length: 20\r\nConnection: close",
            br#"{"version":"cut"}"#,
        ),
        Diagnostic::ResponseIncomplete,
    )
    .await;
    assert_check_error(
        "partial JSON",
        json_response(br#"{"version": "#),
        Diagnostic::ResponseInvalid,
    )
    .await;

    assert_analyze_error(
        "chat declared body too large",
        response(
            "200 OK",
            "Content-Type: application/json\r\nContent-Length: 1048577\r\nConnection: close",
            b"",
        ),
        Diagnostic::ResponseTooLarge,
    )
    .await;
    let chat_chunk = vec![b'x'; 1_048_577];
    let mut chunked_chat = response(
        "200 OK",
        "Content-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close",
        format!("{:X}\r\n", chat_chunk.len()).as_bytes(),
    );
    chunked_chat.extend_from_slice(&chat_chunk);
    chunked_chat.extend_from_slice(b"\r\n0\r\n\r\n");
    assert_analyze_error(
        "chat chunked body too large",
        chunked_chat,
        Diagnostic::ResponseTooLarge,
    )
    .await;
}

async fn check_result(raw_response: Vec<u8>) -> Result<(), Diagnostic> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let profile = local_profile(listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        let mut stream = accept(&listener).await;
        let _request = read_request(&mut stream).await;
        write_raw(&mut stream, &raw_response).await;
    });
    let result = LocalClient::new()
        .check(&profile, &Cancellation::new())
        .await
        .map(|_| ());
    timeout(IO_LIMIT, server)
        .await
        .expect("server task timed out")
        .unwrap();
    result
}

#[tokio::test]
async fn malformed_and_untrusted_model_responses_fail_closed() {
    let _guard = timeout(TEST_LOCK_LIMIT, lock().lock())
        .await
        .expect("test lock timed out");
    let profile_model_mismatch = serde_json::to_vec(&json!({
        "model": "different:v1", "created_at": "test", "message": {"role": "assistant", "content": "{}"}, "done": true, "done_reason": "stop"
    })).unwrap();
    assert_analyze_error(
        "model mismatch",
        json_response(&profile_model_mismatch),
        Diagnostic::ResponseInvalid,
    )
    .await;
    let partial = serde_json::to_vec(&json!({
        "model": "fixture-model:v1", "created_at": "test", "message": {"role": "assistant", "content": "{}"}, "done": false, "done_reason": "length"
    })).unwrap();
    assert_analyze_error(
        "partial model response",
        json_response(&partial),
        Diagnostic::ResponseIncomplete,
    )
    .await;
    let untrusted = serde_json::to_vec(&json!({
        "model": "fixture-model:v1", "created_at": "test", "message": {"role": "assistant", "content": "{\"schemaVersion\":\"rangoon.analysis-proposals.v1\",\"task\":\"wrong\",\"proposals\":[],\"uncertainties\":[]}"}, "done": true, "done_reason": "stop"
    })).unwrap();
    assert_analyze_error(
        "untrusted model proposal",
        json_response(&untrusted),
        Diagnostic::ProposalInvalid,
    )
    .await;
}
