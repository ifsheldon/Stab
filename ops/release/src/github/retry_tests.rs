use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Instant;

use super::*;
use crate::RELEASE_TAG;

struct GetResponse {
    status: u16,
    body: String,
    declared_length: Option<usize>,
}

impl GetResponse {
    fn complete(status: u16, body: String) -> Self {
        Self {
            status,
            body,
            declared_length: None,
        }
    }

    fn truncated(body: &str) -> Self {
        Self {
            status: 200,
            body: body.to_string(),
            declared_length: Some(body.len() + 64),
        }
    }
}

#[test]
fn public_get_retries_a_transient_gateway_status() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = serve_gets(
        listener,
        vec![
            GetResponse::complete(504, "{}".to_string()),
            GetResponse::complete(200, rehearsal_repository_response()),
            GetResponse::complete(200, rehearsal_ruleset_response()),
        ],
        false,
    );
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let mut api = GitHubApi::with_hosts(host.clone(), host, target::REHEARSAL, cancellation);

    api.require_release_tag_ruleset(&GitHubToken(SecretString::from("reviewed-token")))
        .expect("transient public identity GET");
    assert_eq!(
        server.join().expect("server").expect("GET server"),
        [
            "/repos/ifsheldon/Stab-release-rehearsal".to_string(),
            "/repos/ifsheldon/Stab-release-rehearsal".to_string(),
            format!(
                "/repos/ifsheldon/Stab-release-rehearsal/rulesets/{}",
                target::REHEARSAL.ruleset.id
            ),
        ]
    );
}

#[test]
fn authenticated_get_retries_a_transient_gateway_status() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = serve_gets(
        listener,
        vec![
            GetResponse::complete(502, "{}".to_string()),
            GetResponse::complete(200, published_release_response()),
        ],
        true,
    );
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let mut api = GitHubApi::with_hosts(host.clone(), host, target::PRODUCTION, cancellation);
    let token = GitHubToken(SecretString::from("reviewed-token"));

    let recorded = api
        .published_release_by_tag(RELEASE_TAG, &token)
        .expect("transient authenticated GET");
    assert_eq!(recorded.id, 42);
    let path = format!("/repos/ifsheldon/Stab/releases/tags/{RELEASE_TAG}");
    assert_eq!(
        server.join().expect("server").expect("GET server"),
        [path.clone(), path]
    );
}

#[test]
fn successful_headers_with_a_truncated_body_are_retried() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = serve_gets(
        listener,
        vec![
            GetResponse::truncated("{"),
            GetResponse::complete(200, "{\"complete\":true}".to_string()),
        ],
        false,
    );
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let api = GitHubApi::with_hosts(host.clone(), host.clone(), target::REHEARSAL, cancellation);

    let value: serde_json::Value = api
        .get_public_json(&format!("{host}/body"), "truncated response test")
        .expect("complete retry body");
    assert_eq!(value, serde_json::json!({"complete": true}));
    assert_eq!(
        server.join().expect("server").expect("GET server"),
        ["/body".to_string(), "/body".to_string()]
    );
}

#[test]
fn get_retry_is_bounded_to_three_attempts() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = serve_gets(
        listener,
        vec![
            GetResponse::complete(504, "{}".to_string()),
            GetResponse::complete(504, "{}".to_string()),
            GetResponse::complete(504, "{}".to_string()),
        ],
        false,
    );
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let api = GitHubApi::with_hosts(host.clone(), host.clone(), target::REHEARSAL, cancellation);

    let error = api
        .get_public_json::<serde_json::Value>(&format!("{host}/bounded"), "bounded retry test")
        .expect_err("three gateway failures must fail closed");
    assert!(error.to_string().contains("504 Gateway Timeout"));
    assert_eq!(server.join().expect("server").expect("GET server").len(), 3);
}

#[test]
fn cancellation_interrupts_get_retry_backoff() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let cancellation = ReleaseCancellation::for_test();
    let server_cancellation = cancellation.clone();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("retry request");
        let request = read_request(&mut stream);
        write_response(&mut stream, &GetResponse::complete(504, "{}".to_string()));
        server_cancellation.cancel();
        request_path(&request)
    });
    let host = format!("http://{address}");
    let api = GitHubApi::with_hosts_and_retry_delay(
        host.clone(),
        host.clone(),
        target::REHEARSAL,
        cancellation,
        Duration::from_secs(5),
    );

    let started = Instant::now();
    let error = api
        .get_public_json::<serde_json::Value>(&format!("{host}/cancel"), "cancel retry test")
        .expect_err("cancellation must interrupt backoff");
    assert!(matches!(error, ReleaseError::OperationInterrupted { .. }));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(server.join().expect("server"), "/cancel");
}

#[test]
fn draft_creation_post_is_not_retried() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = observe_single_post(listener);
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let api = GitHubApi::with_hosts(host.clone(), host.clone(), target::PRODUCTION, cancellation);
    let token = GitHubToken(SecretString::from("reviewed-token"));

    let started = Instant::now();
    let error = api
        .post_json::<serde_json::Value, _>(
            &format!("{host}/draft"),
            &serde_json::json!({"draft": true}),
            &token,
            "draft mutation test",
        )
        .expect_err("gateway failure must not repeat a draft mutation");
    assert!(error.to_string().contains("504 Gateway Timeout"));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(server.join().expect("server").expect("POST observer"), 1);
}

#[test]
fn asset_upload_post_is_not_retried() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = observe_single_post(listener);
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let api = GitHubApi::with_hosts(host.clone(), host.clone(), target::PRODUCTION, cancellation);
    let token = GitHubToken(SecretString::from("reviewed-token"));
    let fixture = tempfile::tempfile().expect("asset file");

    let started = Instant::now();
    let error = api
        .post_file::<serde_json::Value>(
            &format!("{host}/asset"),
            fixture,
            &token,
            "asset mutation test",
        )
        .expect_err("gateway failure must not repeat an asset mutation");
    assert!(error.to_string().contains("504 Gateway Timeout"));
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(server.join().expect("server").expect("POST observer"), 1);
}

fn rehearsal_repository_response() -> String {
    serde_json::json!({
        "id": target::REHEARSAL.repository_id,
        "full_name": target::REHEARSAL.repository,
        "private": false,
        "archived": false
    })
    .to_string()
}

fn rehearsal_ruleset_response() -> String {
    serde_json::json!({
        "id": target::REHEARSAL.ruleset.id,
        "name": target::REHEARSAL.ruleset.name,
        "node_id": target::REHEARSAL.ruleset.node_id,
        "created_at": target::REHEARSAL.ruleset.created_at,
        "updated_at": target::REHEARSAL.ruleset.updated_at,
        "target": "tag",
        "source_type": "Repository",
        "source": target::REHEARSAL.repository,
        "enforcement": "active",
        "conditions": {"ref_name": {"include": [target::REHEARSAL.ruleset.ref_include], "exclude": []}},
        "rules": [{"type": "update"}, {"type": "deletion"}],
    })
    .to_string()
}

fn published_release_response() -> String {
    serde_json::json!({
        "id": 42,
        "tag_name": RELEASE_TAG,
        "name": target::PRODUCTION.title,
        "body": target::PRODUCTION.notes,
        "draft": false,
        "prerelease": false,
        "published_at": "2026-08-04T00:00:00Z",
        "assets": [],
    })
    .to_string()
}

fn serve_gets(
    listener: TcpListener,
    responses: Vec<GetResponse>,
    expect_authorization: bool,
) -> thread::JoinHandle<std::io::Result<Vec<String>>> {
    thread::spawn(move || {
        listener.set_nonblocking(true)?;
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut paths = Vec::new();
        for response in responses {
            let stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break Some(stream),
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break None,
                    Err(error) => return Err(error),
                }
            };
            let Some(mut stream) = stream else {
                break;
            };
            stream.set_nonblocking(false)?;
            let request = read_request(&mut stream);
            assert!(request.starts_with("GET "), "unexpected request: {request}");
            assert_eq!(has_authorization(&request), expect_authorization);
            paths.push(request_path(&request));
            write_response(&mut stream, &response);
        }
        Ok(paths)
    })
}

fn observe_single_post(listener: TcpListener) -> thread::JoinHandle<std::io::Result<usize>> {
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("POST request");
        let request = read_request(&mut stream);
        assert!(
            request.starts_with("POST "),
            "unexpected request: {request}"
        );
        assert!(has_authorization(&request));
        write_response(&mut stream, &GetResponse::complete(504, "{}".to_string()));
        drop(stream);

        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let deadline = Instant::now() + Duration::from_millis(250);
        let mut requests = 1;
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut retry, _)) => {
                    requests += 1;
                    drop(read_request(&mut retry));
                    write_response(&mut retry, &GetResponse::complete(504, "{}".to_string()));
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(requests)
    })
}

fn read_request(stream: &mut TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected = None;
    loop {
        let read = stream.read(&mut buffer).expect("request read");
        if read == 0 {
            break;
        }
        request.extend_from_slice(buffer.get(..read).expect("bounded request read"));
        if expected.is_none()
            && let Some(header_start) = find_subslice(&request, b"\r\n\r\n")
        {
            let header_end = header_start + 4;
            let headers = String::from_utf8_lossy(
                request.get(..header_end).expect("bounded request headers"),
            );
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("Content-Length: ")
                        .or_else(|| line.strip_prefix("content-length: "))
                })
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            expected = Some(header_end + content_length);
        }
        if expected.is_some_and(|length| request.len() >= length) {
            break;
        }
    }
    String::from_utf8_lossy(&request).into_owned()
}

fn write_response(stream: &mut TcpStream, response: &GetResponse) {
    let reason = match response.status {
        200 => "OK",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Request Failed",
    };
    let content_length = response.declared_length.unwrap_or(response.body.len());
    write!(
        stream,
        "HTTP/1.1 {} {reason}\r\nContent-Type: application/json\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n{}",
        response.status, response.body
    )
    .expect("response");
    stream.flush().expect("response flush");
}

fn has_authorization(request: &str) -> bool {
    request
        .lines()
        .skip(1)
        .take_while(|line| !line.trim_end_matches('\r').is_empty())
        .filter_map(|line| line.split_once(':').map(|(name, _)| name))
        .any(|name| name.eq_ignore_ascii_case("authorization"))
}

fn request_path(request: &str) -> String {
    request
        .lines()
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .expect("request path")
        .to_string()
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
