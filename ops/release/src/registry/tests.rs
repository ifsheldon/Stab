use std::assert_matches;
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::TcpListener;
use std::thread;

use super::*;

fn metadata_fixture() -> Vec<u8> {
    serde_json::to_vec(&ReviewedRegistryMetadata {
        name: "stab-core".to_string(),
        vers: "0.2.0".to_string(),
        deps: vec![],
        features: BTreeMap::new(),
        authors: vec![],
        description: Some("fixture".to_string()),
        documentation: None,
        homepage: None,
        readme: Some("fixture readme".to_string()),
        readme_file: Some("README.crates.md".to_string()),
        keywords: vec![],
        categories: vec![],
        license: Some("MIT".to_string()),
        license_file: None,
        repository: None,
        badges: BTreeMap::new(),
        links: None,
        rust_version: None,
    })
    .expect("metadata")
}

#[test]
fn registry_checksum_response_is_identity_checked() {
    let checksum = "a".repeat(64);
    let body = format!(
        "{{\"version\":{{\"crate\":\"stab-core\",\"num\":\"0.2.0\",\"checksum\":\"{checksum}\",\"yanked\":false}}}}"
    );
    assert_eq!(
        parse_version(&body, "stab-core", "0.2.0").expect("version"),
        RegistryVersion {
            checksum,
            yanked: false,
        }
    );
    assert!(parse_version(&body, "stab-cli", "0.2.0").is_err());
}

#[test]
fn registry_version_response_preserves_yanked_state() {
    let checksum = "a".repeat(64);
    let body = format!(
        "{{\"version\":{{\"crate\":\"stab-core\",\"num\":\"0.2.0\",\"checksum\":\"{checksum}\",\"yanked\":true}}}}"
    );
    let version = parse_version(&body, "stab-core", "0.2.0").expect("version");
    assert!(version.yanked);
}

#[test]
fn reviewed_upload_uses_the_exact_metadata_and_archive_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("upload request");
        let request = read_request(&mut stream);
        assert!(request.starts_with(b"PUT /api/v1/crates/new HTTP/1.1\r\n"));
        let header_end = find_subslice(&request, b"\r\n\r\n").expect("header terminator") + 4;
        let headers =
            String::from_utf8_lossy(request.get(..header_end).expect("bounded request headers"))
                .to_ascii_lowercase();
        assert!(headers.contains("authorization: reviewed-token\r\n"));
        let body = request.get(header_end..).expect("request body");
        let metadata_len = read_u32(body, 0);
        let metadata_start = 4;
        let metadata_end = metadata_start + metadata_len;
        assert_eq!(
            body.get(metadata_start..metadata_end)
                .expect("metadata body"),
            metadata_fixture()
        );
        let archive_len = read_u32(body, metadata_end);
        let archive_start = metadata_end + 4;
        assert_eq!(archive_len, b"reviewed-archive".len());
        assert_eq!(
            body.get(archive_start..).expect("archive body"),
            b"reviewed-archive"
        );
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 11\r\nConnection: close\r\n\r\n{{\"ok\":true}}"
        )
        .expect("response");
    });
    let archive = tempfile::tempfile().expect("archive");
    (&archive)
        .write_all(b"reviewed-archive")
        .expect("archive bytes");
    (&archive).seek(SeekFrom::Start(0)).expect("rewind");
    let registry = CratesIo::with_hosts(
        format!("http://{address}"),
        format!("http://{address}/unused-index"),
        ReleaseCancellation::for_test(),
    );
    registry
        .publish_reviewed(
            &metadata_fixture(),
            &archive,
            &CratesIoToken(SecretString::from("reviewed-token")),
        )
        .expect("reviewed upload");
    server.join().expect("server");
}

#[test]
fn visibility_wait_is_interruptible() {
    struct Missing;
    impl RegistryLookup for Missing {
        fn version(
            &self,
            _package: &str,
            _version: &str,
        ) -> Result<Option<RegistryVersion>, ReleaseError> {
            Ok(None)
        }
    }
    impl SparseIndexLookup for Missing {
        fn index_version(
            &self,
            _package: &CrateName,
            _version: &CrateVersion,
        ) -> Result<Option<RegistryVersion>, ReleaseError> {
            Ok(None)
        }
    }
    let cancellation = ReleaseCancellation::for_test();
    cancellation.cancel();
    assert_matches!(
        wait_for_matching_checksum(
            &Missing,
            &cancellation,
            &crate_name(),
            &crate_version(),
            &"a".repeat(64),
        ),
        Err(ReleaseError::OperationInterrupted { .. })
    );
}

struct FixedRegistry(Option<RegistryVersion>);

impl RegistryLookup for FixedRegistry {
    fn version(
        &self,
        _package: &str,
        _version: &str,
    ) -> Result<Option<RegistryVersion>, ReleaseError> {
        Ok(self.0.clone())
    }
}

impl SparseIndexLookup for FixedRegistry {
    fn index_version(
        &self,
        _package: &CrateName,
        _version: &CrateVersion,
    ) -> Result<Option<RegistryVersion>, ReleaseError> {
        Ok(self.0.clone())
    }
}

fn crate_name() -> CrateName {
    CrateName::try_new("stab-core").expect("crate name")
}

fn crate_version() -> CrateVersion {
    CrateVersion::try_new("0.2.0").expect("crate version")
}

/// The API already serves the upload; the index serves `index` in order, then repeats the last.
struct LaggingIndex {
    api: RegistryVersion,
    index: Vec<Option<RegistryVersion>>,
    index_queries: std::cell::Cell<usize>,
}

impl RegistryLookup for LaggingIndex {
    fn version(
        &self,
        _package: &str,
        _version: &str,
    ) -> Result<Option<RegistryVersion>, ReleaseError> {
        Ok(Some(self.api.clone()))
    }
}

impl SparseIndexLookup for LaggingIndex {
    fn index_version(
        &self,
        _package: &CrateName,
        _version: &CrateVersion,
    ) -> Result<Option<RegistryVersion>, ReleaseError> {
        let query = self.index_queries.get();
        self.index_queries.set(query + 1);
        Ok(self
            .index
            .get(query)
            .or(self.index.last())
            .cloned()
            .flatten())
    }
}

const FAST_VISIBILITY: VisibilityPolicy = VisibilityPolicy {
    timeout: Duration::from_millis(200),
    poll: Duration::from_millis(1),
};

#[test]
fn visibility_wait_requires_the_sparse_index_cargo_reads() {
    let expected = "c".repeat(64);
    let visible = registry_version(expected.clone(), false);
    let lagging = LaggingIndex {
        api: visible.clone(),
        index: vec![None, None, Some(visible.clone())],
        index_queries: std::cell::Cell::new(0),
    };
    wait_for_visibility(
        &lagging,
        &ReleaseCancellation::for_test(),
        &crate_name(),
        &crate_version(),
        &expected,
        FAST_VISIBILITY,
    )
    .expect("index catches up");
    assert_eq!(lagging.index_queries.get(), 3);

    let never_indexed = LaggingIndex {
        api: visible,
        index: vec![None],
        index_queries: std::cell::Cell::new(0),
    };
    assert_matches!(
        wait_for_visibility(
            &never_indexed,
            &ReleaseCancellation::for_test(),
            &crate_name(),
            &crate_version(),
            &expected,
            FAST_VISIBILITY,
        ),
        Err(ReleaseError::RegistryVisibility { .. })
    );
    assert!(never_indexed.index_queries.get() > 1);
}

#[test]
fn visibility_wait_rejects_a_divergent_or_yanked_index_entry() {
    let expected = "c".repeat(64);
    for (index, rejected_as_yanked) in [
        (registry_version("d".repeat(64), false), false),
        (registry_version(expected.clone(), true), true),
    ] {
        let registry = LaggingIndex {
            api: registry_version(expected.clone(), false),
            index: vec![Some(index)],
            index_queries: std::cell::Cell::new(0),
        };
        let result = wait_for_visibility(
            &registry,
            &ReleaseCancellation::for_test(),
            &crate_name(),
            &crate_version(),
            &expected,
            FAST_VISIBILITY,
        );
        if rejected_as_yanked {
            assert_matches!(result, Err(ReleaseError::RegistryYanked { .. }));
        } else {
            assert_matches!(result, Err(ReleaseError::RegistryChecksum { .. }));
        }
    }
}

#[test]
fn sparse_index_query_reads_the_cargo_index_file() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
    let address = listener.local_addr().expect("address");
    let checksum = "e".repeat(64);
    let index_body = format!(
        "{{\"name\":\"stab-core\",\"vers\":\"0.2.0\",\"deps\":[],\"cksum\":\"{checksum}\",\"features\":{{}},\"yanked\":false}}\n"
    );
    let server = thread::spawn(move || {
        for status in ["200 OK", "404 Not Found"] {
            let (mut stream, _) = listener.accept().expect("index request");
            let headers = read_headers(&mut stream).to_ascii_lowercase();
            assert!(
                headers.starts_with("get /index/st/ab/stab-core http/1.1\r\n"),
                "{headers}"
            );
            assert!(headers.contains("user-agent: stab-release/0.2.0\r\n"));
            assert!(!headers.contains("authorization"));
            let body = if status == "200 OK" {
                index_body.as_str()
            } else {
                ""
            };
            write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .expect("response");
        }
    });
    let registry = CratesIo::with_hosts(
        format!("http://{address}/unused-api"),
        format!("http://{address}/index"),
        ReleaseCancellation::for_test(),
    );
    assert_eq!(
        registry
            .index_version(&crate_name(), &crate_version())
            .expect("indexed version"),
        Some(registry_version(checksum, false))
    );
    assert_eq!(
        registry
            .index_version(&crate_name(), &crate_version())
            .expect("missing index file"),
        None
    );
    server.join().expect("server");
}

fn read_headers(stream: &mut std::net::TcpStream) -> String {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    while find_subslice(&request, b"\r\n\r\n").is_none() {
        let read = stream.read(&mut buffer).expect("request bytes");
        assert_ne!(read, 0, "request ended before its headers");
        request.extend_from_slice(buffer.get(..read).expect("read buffer prefix"));
    }
    String::from_utf8(request).expect("UTF-8 request headers")
}

fn registry_version(checksum: String, yanked: bool) -> RegistryVersion {
    RegistryVersion { checksum, yanked }
}

#[test]
fn existing_registry_versions_must_match_reviewed_bytes() {
    let expected = "c".repeat(64);
    assert!(
        !require_absent_or_matching(&FixedRegistry(None), "stab-core", "0.2.0", &expected)
            .expect("missing")
    );
    assert!(
        require_absent_or_matching(
            &FixedRegistry(Some(registry_version(expected.clone(), false))),
            "stab-core",
            "0.2.0",
            &expected
        )
        .expect("matching")
    );
    assert_matches!(
        require_absent_or_matching(
            &FixedRegistry(Some(registry_version("d".repeat(64), false))),
            "stab-core",
            "0.2.0",
            &expected
        ),
        Err(ReleaseError::RegistryChecksum { .. })
    );
    assert_matches!(
        require_absent_or_matching(
            &FixedRegistry(Some(registry_version(expected.clone(), true))),
            "stab-core",
            "0.2.0",
            &expected
        ),
        Err(ReleaseError::RegistryYanked { .. })
    );
    assert_matches!(
        wait_for_matching_checksum(
            &FixedRegistry(Some(registry_version(expected.clone(), true))),
            &ReleaseCancellation::for_test(),
            &crate_name(),
            &crate_version(),
            &expected,
        ),
        Err(ReleaseError::RegistryYanked { .. })
    );
}

fn read_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    let mut expected = None;
    loop {
        let read = stream.read(&mut buffer).expect("request bytes");
        assert_ne!(read, 0, "request ended before its declared body");
        request.extend_from_slice(buffer.get(..read).expect("read buffer prefix"));
        if expected.is_none()
            && let Some(header_end) = find_subslice(&request, b"\r\n\r\n")
        {
            let headers = String::from_utf8_lossy(
                request.get(..header_end).expect("bounded request headers"),
            );
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .map(str::to_string)
                })
                .expect("content length")
                .parse::<usize>()
                .expect("numeric content length");
            expected = Some(header_end + 4 + content_length);
        }
        if expected.is_some_and(|length| request.len() >= length) {
            return request;
        }
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn read_u32(bytes: &[u8], offset: usize) -> usize {
    let end = offset.checked_add(4).expect("u32 offset");
    let raw: [u8; 4] = bytes
        .get(offset..end)
        .expect("u32 bytes")
        .try_into()
        .expect("u32 width");
    usize::try_from(u32::from_le_bytes(raw)).expect("u32 fits usize")
}

const REGISTRY_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// `stab-records` as `cargo metadata` reports it: a path dependency with a
/// version, a crates.io dependency, and a path-only dev-dependency.
fn package_fixture(extra_dependencies: &[serde_json::Value]) -> Package {
    let mut dependencies = vec![
        serde_json::json!({
            "name": "stab-foundation", "source": null, "req": "=0.2.0", "kind": null,
            "rename": null, "optional": false, "uses_default_features": true,
            "features": [], "target": null, "path": "/stab/crates/stab-foundation",
            "registry": null,
        }),
        serde_json::json!({
            "name": "serde", "source": REGISTRY_SOURCE, "req": "^1.0.228", "kind": null,
            "rename": null, "optional": false, "uses_default_features": false,
            "features": ["derive", "alloc"], "target": "cfg(unix)", "path": null,
            "registry": null,
        }),
        serde_json::json!({
            "name": "stab-compat-corpus", "source": null, "req": "*", "kind": "dev",
            "rename": null, "optional": false, "uses_default_features": true,
            "features": [], "target": null, "path": "/stab/test-support/compat-corpus",
            "registry": null,
        }),
    ];
    dependencies.extend_from_slice(extra_dependencies);
    serde_json::from_value(serde_json::json!({
        "name": "stab-records",
        "version": "0.2.0",
        "id": "path+file:///stab/crates/stab-records#0.2.0",
        "license": "MIT",
        "license_file": null,
        "description": "fixture",
        "source": null,
        "dependencies": dependencies,
        "targets": [],
        "features": {},
        "manifest_path": "/stab/crates/stab-records/Cargo.toml",
        "readme": null,
        "repository": null,
        "homepage": null,
        "documentation": null,
        "links": null,
        "publish": null,
        "rust_version": null,
    }))
    .expect("cargo metadata package fixture")
}

fn metadata_with(deps: Vec<ReviewedRegistryDependency>) -> Vec<u8> {
    let mut metadata: ReviewedRegistryMetadata =
        serde_json::from_slice(&metadata_fixture()).expect("metadata fixture");
    metadata.name = "stab-records".to_string();
    metadata.deps = deps;
    serde_json::to_vec(&metadata).expect("metadata")
}

const NORMALIZED_MANIFEST: &str = r#"[package]
name = "stab-records"
version = "0.2.0"

[dependencies.stab-foundation]
version = "=0.2.0"

[target."cfg(unix)".dependencies.serde]
version = "1.0.228"
features = ["derive", "alloc"]
default-features = false
"#;

/// Regression: the reviewed upload metadata carried a path-only
/// dev-dependency with requirement `*`, which `cargo package` strips from the
/// normalized manifest and crates.io rejects.
#[test]
fn reviewed_dependencies_exclude_path_only_dev_dependencies() {
    let deps = manifest::published_dependencies(&package_fixture(&[])).expect("dependencies");
    assert_eq!(
        deps.iter()
            .map(|dependency| (dependency.name.as_str(), dependency.kind.as_str()))
            .collect::<Vec<_>>(),
        [("stab-foundation", "normal"), ("serde", "normal")]
    );
    assert_eq!(
        deps.first()
            .map(|dependency| dependency.version_req.as_str()),
        Some("=0.2.0")
    );
}

#[test]
fn published_dependencies_reject_wildcard_or_versionless_requirements() {
    for (name, kind, source) in [
        (
            "stab-model",
            serde_json::Value::Null,
            serde_json::Value::Null,
        ),
        (
            "libc",
            serde_json::Value::from("dev"),
            serde_json::Value::from(REGISTRY_SOURCE),
        ),
    ] {
        let extra = serde_json::json!({
            "name": name, "source": source, "req": "*", "kind": kind,
            "rename": null, "optional": false, "uses_default_features": true,
            "features": [], "target": null, "path": null, "registry": null,
        });
        let error = manifest::published_dependencies(&package_fixture(&[extra]))
            .expect_err("a published wildcard requirement must fail preflight");
        assert_matches!(error, ReleaseError::PackageContract(detail) if detail.contains(name));
    }
}

#[test]
fn reviewed_dependencies_match_the_normalized_manifest_in_the_packaged_archive() {
    let commit = "1".repeat(40);
    let path = Path::new("stab-records-0.2.0.crate");
    let archive = archive::validate_bytes(
        path,
        archive::fixture_crate("stab-records", "0.2.0", &commit, NORMALIZED_MANIFEST),
        "stab-records",
        "0.2.0",
        &commit,
    )
    .expect("packaged archive");
    let deps = manifest::published_dependencies(&package_fixture(&[])).expect("dependencies");
    require_manifest_dependencies(
        &metadata_with(deps.clone()),
        &archive.normalized_manifest,
        path,
    )
    .expect("reviewed dependencies equal the packaged manifest");

    // The pre-fix metadata still listed the stripped dev-dependency.
    let mut stale = deps.clone();
    stale.push(ReviewedRegistryDependency {
        optional: false,
        default_features: true,
        name: "stab-compat-corpus".to_string(),
        features: Vec::new(),
        version_req: "=0.2.0".to_string(),
        target: None,
        kind: "dev".to_string(),
        registry: None,
        explicit_name_in_toml: None,
        artifact: None,
        bindep_target: None,
        lib: false,
    });
    let error =
        require_manifest_dependencies(&metadata_with(stale), &archive.normalized_manifest, path)
            .expect_err("an extra reviewed dependency must fail");
    assert_matches!(
        error,
        ReleaseError::ArchiveContract { detail, .. } if detail.contains("stab-compat-corpus")
    );

    let mut drifted = deps;
    if let Some(serde) = drifted
        .iter_mut()
        .find(|dependency| dependency.name == "serde")
    {
        serde.version_req = "^1.0.229".to_string();
    }
    assert_matches!(
        require_manifest_dependencies(&metadata_with(drifted), &archive.normalized_manifest, path),
        Err(ReleaseError::ArchiveContract { .. })
    );
}
