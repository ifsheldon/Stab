use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use super::*;

fn remote_asset(name: &str, bytes: &[u8]) -> RemoteAsset {
    RemoteAsset {
        name: name.to_string(),
        state: "uploaded".to_string(),
        size: bytes.len() as u64,
        digest: Some(format!("sha256:{}", crate::archive::sha256_bytes(bytes))),
    }
}

#[test]
fn exact_remote_asset_identity_is_required() {
    let bytes = b"reviewed asset";
    let asset = remote_asset("stab-linux-aarch64", bytes);
    validate_asset(
        &asset,
        "stab-linux-aarch64",
        bytes.len() as u64,
        &crate::archive::sha256_bytes(bytes),
    )
    .expect("exact asset");

    let mut wrong_digest = remote_asset("stab-linux-aarch64", bytes);
    wrong_digest.digest = Some(format!("sha256:{}", "0".repeat(64)));
    assert!(
        validate_asset(
            &wrong_digest,
            "stab-linux-aarch64",
            bytes.len() as u64,
            &crate::archive::sha256_bytes(bytes),
        )
        .is_err()
    );
}

#[test]
fn draft_creation_requires_an_exact_version_confirmation() {
    assert!(matches!(
        create_verified_draft(
            Path::new("."),
            Path::new("target/releases/not-opened"),
            RELEASE_TAG,
            "0.2.1"
        ),
        Err(ReleaseError::PublicationConfirmation { .. })
    ));
}

#[test]
fn remote_tag_must_be_annotated_and_resolve_to_the_reviewed_commit() {
    let commit = "1".repeat(40);
    let reference = RemoteReference {
        object: RemoteGitObject {
            sha: "2".repeat(40),
            kind: "tag".to_string(),
        },
    };
    require_annotated_reference(&reference, RELEASE_TAG).expect("annotated reference");
    let object = RemoteTag {
        tag: RELEASE_TAG.to_string(),
        object: RemoteGitObject {
            sha: commit.clone(),
            kind: "commit".to_string(),
        },
    };
    require_tag_commit(&object, RELEASE_TAG, &commit).expect("reviewed commit");

    let lightweight = RemoteReference {
        object: RemoteGitObject {
            sha: commit.clone(),
            kind: "commit".to_string(),
        },
    };
    assert!(require_annotated_reference(&lightweight, RELEASE_TAG).is_err());
    assert!(require_tag_commit(&object, RELEASE_TAG, &"3".repeat(40)).is_err());
}

struct MovingTagPublisher {
    remote_commit: String,
    events: Vec<&'static str>,
}

impl DraftPublisher for MovingTagPublisher {
    fn require_release_tag_ruleset(&mut self, _token: &GitHubToken) -> Result<(), ReleaseError> {
        self.events.push("ruleset-check");
        Ok(())
    }

    fn require_remote_annotated_tag(
        &mut self,
        _tag: &str,
        commit: &str,
        _token: &GitHubToken,
    ) -> Result<(), ReleaseError> {
        self.events.push("tag-check");
        if self.remote_commit == commit {
            Ok(())
        } else {
            Err(ReleaseError::GitHubRelease(
                "remote tag moved after draft validation".to_string(),
            ))
        }
    }

    fn create_draft(
        &mut self,
        _tag: &str,
        _commit: &str,
        _token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        Err(ReleaseError::GitHubRelease(
            "unexpected draft creation in tag-guard test".to_string(),
        ))
    }

    fn upload_asset(
        &mut self,
        _release_id: u64,
        _name: &str,
        _file: File,
        _token: &GitHubToken,
    ) -> Result<RemoteAsset, ReleaseError> {
        Err(ReleaseError::GitHubRelease(
            "unexpected asset upload in tag-guard test".to_string(),
        ))
    }

    fn published_release_by_tag(
        &mut self,
        _tag: &str,
        _token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        Err(ReleaseError::GitHubRelease(
            "unexpected release query in tag-guard test".to_string(),
        ))
    }

    fn unique_draft_release(
        &mut self,
        _tag: &str,
        _token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        Err(ReleaseError::GitHubRelease(
            "unexpected release query in tag-guard test".to_string(),
        ))
    }
}

#[test]
fn late_remote_tag_change_is_rejected_after_final_validation() {
    let reviewed_commit = "1".repeat(40);
    let moved_commit = "2".repeat(40);
    let mut publisher = MovingTagPublisher {
        remote_commit: reviewed_commit.clone(),
        events: Vec::new(),
    };
    let token = GitHubToken("reviewed-token".to_string());

    let result = with_stable_remote_tag(
        &mut publisher,
        RELEASE_TAG,
        &reviewed_commit,
        &token,
        |publisher| {
            publisher.events.push("draft-created");
            publisher.events.push("assets-uploaded");
            publisher.events.push("final-release-validated");
            publisher.remote_commit = moved_commit;
            Ok(())
        },
    );

    assert!(matches!(
        result,
        Err(ReleaseError::GitHubRelease(detail))
            if detail == "remote tag moved after draft validation"
    ));
    assert_eq!(
        publisher.events,
        [
            "ruleset-check",
            "tag-check",
            "draft-created",
            "assets-uploaded",
            "final-release-validated",
            "tag-check",
        ]
    );
}

#[test]
fn release_protection_and_tag_identity_bracket_remote_verification() {
    let reviewed_commit = "1".repeat(40);
    let mut publisher = MovingTagPublisher {
        remote_commit: reviewed_commit.clone(),
        events: Vec::new(),
    };
    let token = GitHubToken("reviewed-token".to_string());

    with_stable_remote_tag(
        &mut publisher,
        RELEASE_TAG,
        &reviewed_commit,
        &token,
        |publisher| {
            publisher.events.push("release-validated");
            Ok(())
        },
    )
    .expect("stable protected release tag");

    assert_eq!(
        publisher.events,
        [
            "ruleset-check",
            "tag-check",
            "release-validated",
            "tag-check",
            "ruleset-check",
        ]
    );
}

#[test]
fn complete_private_draft_identity_is_required() {
    let identities = [
        ("stab-linux-aarch64", b"linux".as_slice()),
        ("stab-linux-aarch64.sha256", b"linux checksum".as_slice()),
        ("stab-linux-aarch64.json", b"linux manifest".as_slice()),
        ("stab-macos-aarch64", b"macos".as_slice()),
        ("stab-macos-aarch64.sha256", b"macos checksum".as_slice()),
        ("stab-macos-aarch64.json", b"macos manifest".as_slice()),
    ];
    let expected = identities
        .iter()
        .map(|(name, bytes)| ExpectedAsset {
            name: (*name).to_string(),
            bytes: bytes.len() as u64,
            sha256: crate::archive::sha256_bytes(bytes),
        })
        .collect::<Vec<_>>();
    let mut release = RemoteRelease {
        id: 7,
        tag_name: RELEASE_TAG.to_string(),
        name: Some(RELEASE_TITLE.to_string()),
        body: Some(RELEASE_NOTES.to_string()),
        draft: true,
        prerelease: false,
        published_at: None,
        assets: identities
            .iter()
            .map(|(name, bytes)| remote_asset(name, bytes))
            .collect(),
    };
    validate_release(&release, RELEASE_TAG, &expected, RemoteReleaseState::Draft)
        .expect("complete draft");

    release.assets.pop();
    assert!(validate_release(&release, RELEASE_TAG, &expected, RemoteReleaseState::Draft).is_err());
    release.assets = identities
        .iter()
        .map(|(name, bytes)| remote_asset(name, bytes))
        .collect();
    release.draft = false;
    assert!(
        validate_release(
            &release,
            RELEASE_TAG,
            &expected,
            RemoteReleaseState::Published
        )
        .is_err()
    );
    release.published_at = Some("2026-08-04T00:00:00Z".to_string());
    validate_release(
        &release,
        RELEASE_TAG,
        &expected,
        RemoteReleaseState::Published,
    )
    .expect("complete published release");
}

#[test]
fn draft_request_cannot_publish_the_release() {
    let request = CreateReleaseRequest {
        tag_name: RELEASE_TAG,
        target_commitish: "1",
        name: RELEASE_TITLE,
        body: RELEASE_NOTES,
        draft: true,
        prerelease: false,
        generate_release_notes: false,
        make_latest: "false",
    };
    let value = serde_json::to_value(request).expect("request JSON");
    assert_eq!(
        value,
        serde_json::json!({
            "tag_name": RELEASE_TAG,
            "target_commitish": "1",
            "name": RELEASE_TITLE,
            "body": RELEASE_NOTES,
            "draft": true,
            "prerelease": false,
            "generate_release_notes": false,
            "make_latest": "false"
        })
    );
}

#[test]
fn draft_creation_sends_the_private_release_contract() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let commit = "1".repeat(40);
    let expected_commit = commit.clone();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("create request");
        let request = read_request(&mut stream);
        let header_end = find_subslice(&request, b"\r\n\r\n").expect("headers") + 4;
        let headers =
            String::from_utf8_lossy(request.get(..header_end).expect("bounded request headers"));
        assert!(headers.starts_with("POST /repos/ifsheldon/Stab/releases HTTP/1.1\r\n"));
        let lowercase_headers = headers.to_ascii_lowercase();
        assert!(lowercase_headers.contains("authorization: bearer reviewed-token\r\n"));
        let request_body: serde_json::Value =
            serde_json::from_slice(request.get(header_end..).expect("bounded request body"))
                .expect("request JSON");
        assert_eq!(
            request_body,
            serde_json::json!({
                "tag_name": RELEASE_TAG,
                "target_commitish": expected_commit,
                "name": RELEASE_TITLE,
                "body": RELEASE_NOTES,
                "draft": true,
                "prerelease": false,
                "generate_release_notes": false,
                "make_latest": "false"
            })
        );
        let response = format!(
            "{{\"id\":42,\"tag_name\":\"{RELEASE_TAG}\",\"name\":\"{RELEASE_TITLE}\",\"body\":\"{RELEASE_NOTES}\",\"draft\":true,\"prerelease\":false,\"assets\":[]}}"
        );
        write!(
            stream,
            "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
            response.len()
        )
        .expect("response");
    });
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let mut api = GitHubApi::with_hosts(host.clone(), host, cancellation);
    let release = api
        .create_draft(
            RELEASE_TAG,
            &commit,
            &GitHubToken("reviewed-token".to_string()),
        )
        .expect("create draft");
    validate_release(&release, RELEASE_TAG, &[], RemoteReleaseState::Draft).expect("private draft");
    server.join().expect("server");
}

#[test]
fn asset_upload_sends_exact_file_bytes_and_scopes_the_token_to_the_header() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let payload = b"reviewed asset bytes".to_vec();
    let expected_payload = payload.clone();
    let digest = crate::archive::sha256_bytes(&payload);
    let expected_digest = digest.clone();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("upload request");
        let request = read_request(&mut stream);
        let header_end = find_subslice(&request, b"\r\n\r\n").expect("headers") + 4;
        let headers =
            String::from_utf8_lossy(request.get(..header_end).expect("bounded request headers"));
        assert!(headers.starts_with(
            "POST /repos/ifsheldon/Stab/releases/42/assets?name=stab-linux-aarch64 HTTP/1.1\r\n"
        ));
        let lowercase_headers = headers.to_ascii_lowercase();
        assert!(lowercase_headers.contains("authorization: bearer reviewed-token\r\n"));
        assert_eq!(headers.matches("reviewed-token").count(), 1);
        assert_eq!(
            request.get(header_end..).expect("bounded request body"),
            expected_payload
        );
        let response = format!(
            "{{\"name\":\"stab-linux-aarch64\",\"state\":\"uploaded\",\"size\":{},\"digest\":\"sha256:{expected_digest}\"}}",
            expected_payload.len()
        );
        write!(
            stream,
            "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
            response.len()
        )
        .expect("response");
    });
    let file_root = tempfile::tempdir().expect("file root");
    let path = file_root.path().join("asset");
    std::fs::write(&path, &payload).expect("asset");
    let file = std::fs::File::open(&path).expect("open asset");
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let mut api = GitHubApi::with_hosts(host.clone(), host, cancellation);
    let asset = api
        .upload_asset(
            42,
            "stab-linux-aarch64",
            file,
            &GitHubToken("reviewed-token".to_string()),
        )
        .expect("upload");
    validate_asset(&asset, "stab-linux-aarch64", payload.len() as u64, &digest)
        .expect("recorded asset");
    server.join().expect("server");
}

fn read_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected = None;
    loop {
        let read = stream.read(&mut buffer).expect("request read");
        if read == 0 {
            break;
        }
        request.extend_from_slice(buffer.get(..read).expect("bounded read"));
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
                .expect("content length");
            expected = Some(header_end + content_length);
        }
        if expected.is_some_and(|length| request.len() >= length) {
            break;
        }
    }
    request
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[test]
fn draft_verification_survives_the_published_only_by_tag_endpoint() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let page = serde_json::to_string(&vec![release_entry(42, RELEASE_TAG, true)]).expect("page");
    let server = serve_json_get_requests(
        listener,
        vec![
            (404, "{\"message\":\"Not Found\"}".to_string()),
            (200, page),
        ],
    );
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let mut api = GitHubApi::with_hosts(host.clone(), host, cancellation);
    let token = GitHubToken("reviewed-token".to_string());

    let by_tag = api.published_release_by_tag(RELEASE_TAG, &token);
    assert!(
        matches!(by_tag, Err(ReleaseError::GitHubRelease(detail)) if detail.contains("404")),
        "the by-tag endpoint must be pinned as unable to see drafts"
    );
    let release = api
        .unique_draft_release(RELEASE_TAG, &token)
        .expect("draft lookup must survive the published-only by-tag endpoint");
    validate_release(&release, RELEASE_TAG, &[], RemoteReleaseState::Draft).expect("private draft");
    assert_eq!(
        server.join().expect("server"),
        [
            format!("/repos/ifsheldon/Stab/releases/tags/{RELEASE_TAG}"),
            format!("/repos/ifsheldon/Stab/releases?per_page={RELEASE_LIST_PAGE_SIZE}&page=1"),
        ]
    );
}

struct StateRoutingPublisher {
    events: Vec<&'static str>,
}

impl DraftPublisher for StateRoutingPublisher {
    fn require_release_tag_ruleset(&mut self, _token: &GitHubToken) -> Result<(), ReleaseError> {
        Ok(())
    }

    fn require_remote_annotated_tag(
        &mut self,
        _tag: &str,
        _commit: &str,
        _token: &GitHubToken,
    ) -> Result<(), ReleaseError> {
        Ok(())
    }

    fn create_draft(
        &mut self,
        _tag: &str,
        _commit: &str,
        _token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        Err(ReleaseError::GitHubRelease(
            "unexpected draft creation in state-routing test".to_string(),
        ))
    }

    fn upload_asset(
        &mut self,
        _release_id: u64,
        _name: &str,
        _file: File,
        _token: &GitHubToken,
    ) -> Result<RemoteAsset, ReleaseError> {
        Err(ReleaseError::GitHubRelease(
            "unexpected asset upload in state-routing test".to_string(),
        ))
    }

    fn published_release_by_tag(
        &mut self,
        _tag: &str,
        _token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        self.events.push("published-by-tag");
        Err(ReleaseError::GitHubRelease(
            "draft releases are invisible to the by-tag endpoint".to_string(),
        ))
    }

    fn unique_draft_release(
        &mut self,
        _tag: &str,
        _token: &GitHubToken,
    ) -> Result<RemoteRelease, ReleaseError> {
        self.events.push("unique-draft-lookup");
        Ok(remote_release(42, true))
    }
}

#[test]
fn draft_state_routes_to_the_list_lookup_and_published_state_to_by_tag() {
    let token = GitHubToken("reviewed-token".to_string());
    let mut publisher = StateRoutingPublisher { events: Vec::new() };

    let draft = release_in_state(
        &mut publisher,
        RELEASE_TAG,
        RemoteReleaseState::Draft,
        &token,
    )
    .expect("draft verification must not use the published-only by-tag endpoint");
    assert_eq!(draft.id, 42);
    assert!(draft.draft);

    let published = release_in_state(
        &mut publisher,
        RELEASE_TAG,
        RemoteReleaseState::Published,
        &token,
    );
    assert!(matches!(
        published,
        Err(ReleaseError::GitHubRelease(detail)) if detail.contains("invisible")
    ));
    assert_eq!(
        publisher.events,
        ["unique-draft-lookup", "published-by-tag"]
    );
}

#[test]
fn unique_draft_lookup_scans_full_pages_and_filters_on_tag_and_draft_state() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let mut first_page = vec![
        release_entry(1, RELEASE_TAG, false),
        release_entry(2, "v0.1.9", true),
    ];
    while first_page.len() < RELEASE_LIST_PAGE_SIZE {
        let id = first_page.len() as u64 + 100;
        first_page.push(release_entry(id, &format!("other-{id}"), false));
    }
    let second_page = vec![release_entry(77, RELEASE_TAG, true)];
    let server = serve_json_get_requests(
        listener,
        vec![
            (200, serde_json::to_string(&first_page).expect("first page")),
            (
                200,
                serde_json::to_string(&second_page).expect("second page"),
            ),
        ],
    );
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let mut api = GitHubApi::with_hosts(host.clone(), host, cancellation);

    let release = api
        .unique_draft_release(RELEASE_TAG, &GitHubToken("reviewed-token".to_string()))
        .expect("draft on the second page");
    assert_eq!(release.id, 77);
    assert_eq!(
        server.join().expect("server"),
        [
            format!("/repos/ifsheldon/Stab/releases?per_page={RELEASE_LIST_PAGE_SIZE}&page=1"),
            format!("/repos/ifsheldon/Stab/releases?per_page={RELEASE_LIST_PAGE_SIZE}&page=2"),
        ]
    );
}

#[test]
fn unique_draft_lookup_requires_exactly_one_matching_draft() {
    let no_draft_page =
        serde_json::to_string(&vec![release_entry(1, RELEASE_TAG, false)]).expect("page");
    let error = unique_draft_error(vec![(200, no_draft_page)]);
    assert!(error.contains("no draft release"), "{error}");

    let duplicate_page = serde_json::to_string(&vec![
        release_entry(1, RELEASE_TAG, true),
        release_entry(2, RELEASE_TAG, true),
    ])
    .expect("page");
    let error = unique_draft_error(vec![(200, duplicate_page)]);
    assert!(error.contains("expected exactly one"), "{error}");
}

#[test]
fn unique_draft_lookup_fails_closed_when_the_release_list_never_ends() {
    let full_page = (0..RELEASE_LIST_PAGE_SIZE)
        .map(|index| release_entry(index as u64 + 1, &format!("other-{index}"), false))
        .collect::<Vec<_>>();
    let body = serde_json::to_string(&full_page).expect("page");
    let responses = std::iter::repeat_n((200, body), RELEASE_LIST_PAGE_BOUND).collect::<Vec<_>>();

    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = serve_json_get_requests(listener, responses);
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let mut api = GitHubApi::with_hosts(host.clone(), host, cancellation);

    let error = api
        .unique_draft_release(RELEASE_TAG, &GitHubToken("reviewed-token".to_string()))
        .expect_err("an unbounded release list must fail closed");
    assert!(error.to_string().contains("cannot be verified"), "{error}");
    assert_eq!(
        server.join().expect("server").len(),
        RELEASE_LIST_PAGE_BOUND
    );
}

#[test]
fn published_release_by_tag_still_verifies_published_releases() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let body = serde_json::to_string(&release_entry(7, RELEASE_TAG, false)).expect("release");
    let server = serve_json_get_requests(listener, vec![(200, body)]);
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let mut api = GitHubApi::with_hosts(host.clone(), host, cancellation);

    let release = api
        .published_release_by_tag(RELEASE_TAG, &GitHubToken("reviewed-token".to_string()))
        .expect("published release");
    validate_release(&release, RELEASE_TAG, &[], RemoteReleaseState::Published)
        .expect("published state");
    assert_eq!(
        server.join().expect("server"),
        [format!("/repos/ifsheldon/Stab/releases/tags/{RELEASE_TAG}")]
    );
}

fn unique_draft_error(responses: Vec<(u16, String)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = serve_json_get_requests(listener, responses);
    let cancellation = ReleaseCancellation::for_test();
    let host = format!("http://{address}");
    let mut api = GitHubApi::with_hosts(host.clone(), host, cancellation);
    let error = api
        .unique_draft_release(RELEASE_TAG, &GitHubToken("reviewed-token".to_string()))
        .expect_err("draft lookup must reject this release list");
    server.join().expect("server");
    error.to_string()
}

fn release_entry(id: u64, tag: &str, draft: bool) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "tag_name": tag,
        "name": RELEASE_TITLE,
        "body": RELEASE_NOTES,
        "draft": draft,
        "prerelease": false,
        "published_at": if draft {
            serde_json::Value::Null
        } else {
            serde_json::Value::from("2026-08-04T00:00:00Z")
        },
        "assets": [],
    })
}

fn remote_release(id: u64, draft: bool) -> RemoteRelease {
    RemoteRelease {
        id,
        tag_name: RELEASE_TAG.to_string(),
        name: Some(RELEASE_TITLE.to_string()),
        body: Some(RELEASE_NOTES.to_string()),
        draft,
        prerelease: false,
        published_at: (!draft).then(|| "2026-08-04T00:00:00Z".to_string()),
        assets: Vec::new(),
    }
}

fn serve_json_get_requests(
    listener: TcpListener,
    responses: Vec<(u16, String)>,
) -> thread::JoinHandle<Vec<String>> {
    thread::spawn(move || {
        let mut paths = Vec::new();
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().expect("release request");
            let request = read_request_headers(&mut stream);
            assert!(request.starts_with("GET "), "unexpected request: {request}");
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer reviewed-token\r\n"),
                "release request must carry the bearer token"
            );
            paths.push(
                request
                    .lines()
                    .next()
                    .and_then(|line| line.split(' ').nth(1))
                    .expect("request path")
                    .to_string(),
            );
            let reason = if status == 200 { "OK" } else { "Not Found" };
            write!(
                stream,
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .expect("response");
        }
        paths
    })
}

fn read_request_headers(stream: &mut std::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    while find_subslice(&request, b"\r\n\r\n").is_none() {
        let read = stream.read(&mut buffer).expect("headers read");
        if read == 0 {
            break;
        }
        request.extend_from_slice(buffer.get(..read).expect("bounded read"));
    }
    String::from_utf8_lossy(&request).into_owned()
}
