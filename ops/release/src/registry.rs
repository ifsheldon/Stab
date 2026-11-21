use secrecy::{ExposeSecret, SecretString};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;
use std::time::{Duration, Instant};

use cargo_metadata::{DependencyKind, Package};
use crates_io::{HttpClient, NewCrate, NewCrateDependency, Registry, Warnings};
use serde::{Deserialize, Serialize};

use crate::{ReleaseError, archive, cancellation::ReleaseCancellation, safe_fs};

mod index;
mod manifest;
#[cfg(test)]
mod tests;
pub(crate) use index::{CrateName, CrateVersion};
pub(crate) use manifest::require_manifest_dependencies;

const CRATES_IO_HOST: &str = "https://crates.io";
const CRATES_IO_INDEX_HOST: &str = "https://index.crates.io";
const USER_AGENT: &str = "stab-release/0.2.0";
const MAX_REGISTRY_RESPONSE_BYTES: u64 = 1 << 20;
const MAX_INDEX_RESPONSE_BYTES: u64 = 8 << 20;
pub(crate) const MAX_REGISTRY_METADATA_BYTES: u64 = 1 << 20;
const MAX_README_BYTES: u64 = 2 << 20;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CRATES_IO_VISIBILITY: VisibilityPolicy = VisibilityPolicy {
    timeout: Duration::from_secs(10 * 60),
    poll: Duration::from_secs(10),
};

pub(crate) trait RegistryLookup {
    fn version(
        &self,
        package: &str,
        version: &str,
    ) -> Result<Option<RegistryVersion>, ReleaseError>;
}

/// The sparse index Cargo resolves registry dependencies through.
pub(crate) trait SparseIndexLookup {
    fn index_version(
        &self,
        package: &CrateName,
        version: &CrateVersion,
    ) -> Result<Option<RegistryVersion>, ReleaseError>;
}

#[derive(Clone, Copy, Debug)]
struct VisibilityPolicy {
    timeout: Duration,
    poll: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RegistryVersion {
    checksum: String,
    yanked: bool,
}

#[derive(Debug)]
pub(crate) struct CratesIo {
    agent: ureq::Agent,
    host: String,
    index_host: String,
    cancellation: ReleaseCancellation,
}

pub(crate) struct CratesIoToken(SecretString);

impl CratesIoToken {
    pub(crate) fn from_environment() -> Result<Self, ReleaseError> {
        let token = std::env::var_os("CARGO_REGISTRY_TOKEN").ok_or_else(|| {
            ReleaseError::PublicationState(
                "CARGO_REGISTRY_TOKEN is required for the crates.io upload".to_string(),
            )
        })?;
        let token = token.into_string().map_err(|_| {
            ReleaseError::PublicationState(
                "CARGO_REGISTRY_TOKEN must contain valid UTF-8".to_string(),
            )
        })?;
        if token.is_empty() {
            return Err(ReleaseError::PublicationState(
                "CARGO_REGISTRY_TOKEN must not be empty".to_string(),
            ));
        }
        Ok(Self(SecretString::from(token)))
    }

    /// The only exposure point; call it solely where the value is transmitted.
    fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl std::fmt::Debug for CratesIoToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl CratesIo {
    pub(crate) fn new(cancellation: ReleaseCancellation) -> Self {
        Self::with_hosts(
            CRATES_IO_HOST.to_string(),
            CRATES_IO_INDEX_HOST.to_string(),
            cancellation,
        )
    }

    fn with_hosts(host: String, index_host: String, cancellation: ReleaseCancellation) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .http_status_as_error(false)
            .build();
        Self {
            agent: config.into(),
            host,
            index_host,
            cancellation,
        }
    }

    pub(crate) fn publish_reviewed(
        &self,
        metadata_bytes: &[u8],
        archive: &File,
        token: &CratesIoToken,
    ) -> Result<(), ReleaseError> {
        self.cancellation.check("crates.io upload")?;
        let metadata = parse_reviewed_metadata(metadata_bytes, None, None)?;
        let crate_request = metadata.into_new_crate();
        let serialized = serde_json::to_vec(&crate_request)?;
        if serialized != metadata_bytes {
            return Err(ReleaseError::PackageContract(
                "reviewed registry metadata changed during upload preparation".to_string(),
            ));
        }
        let client = UreqClient {
            agent: self.agent.clone(),
            cancellation: self.cancellation.clone(),
        };
        let mut registry = Registry::new_handle(
            self.host.clone(),
            Some(token.expose().to_string()),
            client,
            false,
        );
        let warnings = registry
            .publish(&crate_request, archive)
            .map_err(|error| ReleaseError::Registry(error.to_string()))?;
        self.cancellation.check("crates.io upload")?;
        require_no_warnings(warnings)
    }
}

impl RegistryLookup for CratesIo {
    fn version(
        &self,
        package: &str,
        version: &str,
    ) -> Result<Option<RegistryVersion>, ReleaseError> {
        self.cancellation.check("crates.io checksum query")?;
        validate_identifier(package)?;
        validate_identifier(version)?;
        let url = format!("{}/api/v1/crates/{package}/{version}", self.host);
        let mut response = self
            .agent
            .get(&url)
            .header("Accept", "application/json")
            .header("User-Agent", USER_AGENT)
            .call()
            .map_err(|error| {
                ReleaseError::Registry(format!("failed to query {package} {version}: {error}"))
            })?;
        self.cancellation.check("crates.io checksum query")?;
        if response.status() == ureq::http::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(ReleaseError::Registry(format!(
                "failed to query {package} {version}: HTTP {}",
                response.status()
            )));
        }
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_REGISTRY_RESPONSE_BYTES)
            .read_to_string()
            .map_err(|error| {
                ReleaseError::Registry(format!(
                    "failed to read {package} {version} response: {error}"
                ))
            })?;
        self.cancellation.check("crates.io checksum query")?;
        parse_version(&body, package, version).map(Some)
    }
}

impl SparseIndexLookup for CratesIo {
    fn index_version(
        &self,
        package: &CrateName,
        version: &CrateVersion,
    ) -> Result<Option<RegistryVersion>, ReleaseError> {
        self.cancellation.check("crates.io sparse index query")?;
        let url = format!("{}/{}", self.index_host, package.sparse_index_path());
        let mut response = self
            .agent
            .get(&url)
            .header("User-Agent", USER_AGENT)
            .call()
            .map_err(|error| {
                ReleaseError::Registry(format!(
                    "failed to query the sparse index for {package} {version}: {error}"
                ))
            })?;
        self.cancellation.check("crates.io sparse index query")?;
        // Cargo treats these statuses as "no such crate yet" for sparse registries.
        if matches!(
            response.status(),
            ureq::http::StatusCode::NOT_FOUND
                | ureq::http::StatusCode::GONE
                | ureq::http::StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS
        ) {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(ReleaseError::Registry(format!(
                "failed to query the sparse index for {package} {version}: HTTP {}",
                response.status()
            )));
        }
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_INDEX_RESPONSE_BYTES)
            .read_to_string()
            .map_err(|error| {
                ReleaseError::Registry(format!(
                    "failed to read the sparse index for {package} {version}: {error}"
                ))
            })?;
        self.cancellation.check("crates.io sparse index query")?;
        index::parse_index_file(&body, package, version)
    }
}

pub(crate) fn canonical_metadata(root: &Path, package: &Package) -> Result<Vec<u8>, ReleaseError> {
    if package.license_file.is_some() {
        return Err(ReleaseError::PackageContract(format!(
            "{} uses an unsupported license-file publication field",
            package.name
        )));
    }
    let readme_path = root.join("README.crates.md");
    let readme_file = safe_fs::open_regular_file(&readme_path)?;
    let readme = safe_fs::read_bounded_file(readme_file, &readme_path, MAX_README_BYTES)?;
    let readme = String::from_utf8(readme).map_err(ReleaseError::from)?;
    let deps = manifest::published_dependencies(package)?;
    let metadata = ReviewedRegistryMetadata {
        name: package.name.to_string(),
        vers: package.version.to_string(),
        deps,
        features: package.features.clone(),
        authors: package.authors.clone(),
        description: package.description.clone(),
        documentation: package.documentation.clone(),
        homepage: package.homepage.clone(),
        readme: Some(readme),
        readme_file: Some("README.crates.md".to_string()),
        keywords: package.keywords.clone(),
        categories: package.categories.clone(),
        license: package.license.clone(),
        license_file: None,
        repository: package.repository.clone(),
        badges: BTreeMap::new(),
        links: package.links.clone(),
        rust_version: package.rust_version.as_ref().map(ToString::to_string),
    };
    let bytes = serde_json::to_vec(&metadata)?;
    if u64::try_from(bytes.len())
        .ok()
        .is_none_or(|size| size > MAX_REGISTRY_METADATA_BYTES)
        || u32::try_from(bytes.len()).is_err()
    {
        return Err(ReleaseError::PackageContract(format!(
            "{} registry metadata exceeds its publication bound",
            package.name
        )));
    }
    parse_reviewed_metadata(
        &bytes,
        Some(package.name.as_str()),
        Some(&package.version.to_string()),
    )?;
    Ok(bytes)
}

pub(crate) fn validate_reviewed_metadata(
    bytes: &[u8],
    package: &str,
    version: &str,
) -> Result<(), ReleaseError> {
    parse_reviewed_metadata(bytes, Some(package), Some(version)).map(drop)
}

pub(crate) fn metadata_sha256(bytes: &[u8]) -> String {
    archive::sha256_bytes(bytes)
}

pub(crate) fn require_absent_or_matching(
    registry: &impl RegistryLookup,
    package: &str,
    version: &str,
    expected: &str,
) -> Result<bool, ReleaseError> {
    require_absent_or_matching_version(
        registry.version(package, version)?,
        package,
        version,
        expected,
    )
}

fn require_absent_or_matching_version(
    found: Option<RegistryVersion>,
    package: &str,
    version: &str,
    expected: &str,
) -> Result<bool, ReleaseError> {
    match found {
        None => Ok(false),
        Some(actual) if actual.yanked => Err(ReleaseError::RegistryYanked {
            package: package.to_string(),
            version: version.to_string(),
        }),
        Some(actual) if actual.checksum == expected => Ok(true),
        Some(actual) => Err(ReleaseError::RegistryChecksum {
            package: package.to_string(),
            version: version.to_string(),
            expected: expected.to_string(),
            actual: actual.checksum,
        }),
    }
}

/// Waits until both the crates.io API and the sparse index Cargo resolves
/// through expose the reviewed checksum, so the next crate can be packaged.
pub(crate) fn wait_for_matching_checksum(
    registry: &(impl RegistryLookup + SparseIndexLookup),
    cancellation: &ReleaseCancellation,
    package: &CrateName,
    version: &CrateVersion,
    expected: &str,
) -> Result<(), ReleaseError> {
    wait_for_visibility(
        registry,
        cancellation,
        package,
        version,
        expected,
        CRATES_IO_VISIBILITY,
    )
}

fn wait_for_visibility(
    registry: &(impl RegistryLookup + SparseIndexLookup),
    cancellation: &ReleaseCancellation,
    package: &CrateName,
    version: &CrateVersion,
    expected: &str,
    policy: VisibilityPolicy,
) -> Result<(), ReleaseError> {
    let (name, number) = (package.as_str(), version.as_str());
    let started = Instant::now();
    while started.elapsed() < policy.timeout {
        cancellation.check("crates.io visibility polling")?;
        if require_absent_or_matching(registry, name, number, expected)?
            && require_absent_or_matching_version(
                registry.index_version(package, version)?,
                name,
                number,
                expected,
            )?
        {
            return Ok(());
        }
        cancellation.sleep(policy.poll, "crates.io visibility polling")?;
    }
    Err(ReleaseError::RegistryVisibility {
        package: name.to_string(),
        version: number.to_string(),
        checksum: expected.to_string(),
    })
}

fn reviewed_dependency(
    package: &str,
    dependency: &cargo_metadata::Dependency,
) -> Result<ReviewedRegistryDependency, ReleaseError> {
    if dependency.req == cargo_metadata::semver::VersionReq::STAR {
        return Err(ReleaseError::PackageContract(format!(
            "{package} would publish dependency {} with a wildcard or missing version requirement, which crates.io rejects",
            dependency.name
        )));
    }
    if dependency.registry.is_some()
        || dependency
            .source
            .as_ref()
            .is_some_and(|source| !source.is_crates_io())
    {
        return Err(ReleaseError::PackageContract(format!(
            "dependency {} uses an unsupported alternate registry",
            dependency.name
        )));
    }
    let kind = match dependency.kind {
        DependencyKind::Normal => "normal",
        DependencyKind::Development => "dev",
        DependencyKind::Build => "build",
        _ => {
            return Err(ReleaseError::PackageContract(format!(
                "dependency {} has an unknown dependency kind",
                dependency.name
            )));
        }
    };
    Ok(ReviewedRegistryDependency {
        optional: dependency.optional,
        default_features: dependency.uses_default_features,
        name: dependency.name.clone(),
        features: dependency.features.clone(),
        version_req: dependency.req.to_string(),
        target: dependency.target.as_ref().map(ToString::to_string),
        kind: kind.to_string(),
        registry: None,
        explicit_name_in_toml: dependency.rename.clone(),
        artifact: None,
        bindep_target: None,
        lib: false,
    })
}

fn parse_reviewed_metadata(
    bytes: &[u8],
    expected_package: Option<&str>,
    expected_version: Option<&str>,
) -> Result<ReviewedRegistryMetadata, ReleaseError> {
    if u64::try_from(bytes.len())
        .ok()
        .is_none_or(|size| size > MAX_REGISTRY_METADATA_BYTES)
        || u32::try_from(bytes.len()).is_err()
    {
        return Err(ReleaseError::PackageContract(
            "reviewed registry metadata exceeds its publication bound".to_string(),
        ));
    }
    let metadata: ReviewedRegistryMetadata = serde_json::from_slice(bytes)?;
    if expected_package.is_some_and(|expected| metadata.name != expected)
        || expected_version.is_some_and(|expected| metadata.vers != expected)
    {
        return Err(ReleaseError::PackageContract(
            "reviewed registry metadata names the wrong package or version".to_string(),
        ));
    }
    if serde_json::to_vec(&metadata)? != bytes {
        return Err(ReleaseError::PackageContract(
            "reviewed registry metadata is not in canonical form".to_string(),
        ));
    }
    Ok(metadata)
}

fn require_no_warnings(warnings: Warnings) -> Result<(), ReleaseError> {
    if warnings.invalid_categories.is_empty()
        && warnings.invalid_badges.is_empty()
        && warnings.other.is_empty()
    {
        return Ok(());
    }
    Err(ReleaseError::Registry(format!(
        "crates.io accepted the upload with unexpected warnings: invalid categories {:?}, invalid badges {:?}, other {:?}",
        warnings.invalid_categories, warnings.invalid_badges, warnings.other
    )))
}

fn parse_version(
    body: &str,
    package: &str,
    version: &str,
) -> Result<RegistryVersion, ReleaseError> {
    let response: VersionResponse = serde_json::from_str(body).map_err(|error| {
        ReleaseError::Registry(format!(
            "invalid crates.io response for {package} {version}: {error}"
        ))
    })?;
    if response.version.crate_name != package || response.version.number != version {
        return Err(ReleaseError::Registry(format!(
            "crates.io returned {} {} for requested {package} {version}",
            response.version.crate_name, response.version.number
        )));
    }
    if !is_sha256_hex(&response.version.checksum) {
        return Err(ReleaseError::Registry(format!(
            "crates.io returned an invalid checksum for {package} {version}"
        )));
    }
    Ok(RegistryVersion {
        checksum: response.version.checksum,
        yanked: response.version.yanked,
    })
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_identifier(value: &str) -> Result<(), ReleaseError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
    {
        return Err(ReleaseError::Registry(format!(
            "invalid registry identifier {value:?}"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewedRegistryMetadata {
    name: String,
    vers: String,
    deps: Vec<ReviewedRegistryDependency>,
    features: BTreeMap<String, Vec<String>>,
    authors: Vec<String>,
    description: Option<String>,
    documentation: Option<String>,
    homepage: Option<String>,
    readme: Option<String>,
    readme_file: Option<String>,
    keywords: Vec<String>,
    categories: Vec<String>,
    license: Option<String>,
    license_file: Option<String>,
    repository: Option<String>,
    badges: BTreeMap<String, BTreeMap<String, String>>,
    links: Option<String>,
    rust_version: Option<String>,
}

impl ReviewedRegistryMetadata {
    fn into_new_crate(self) -> NewCrate {
        NewCrate {
            name: self.name,
            vers: self.vers,
            deps: self
                .deps
                .into_iter()
                .map(ReviewedRegistryDependency::into_new_dependency)
                .collect(),
            features: self.features,
            authors: self.authors,
            description: self.description,
            documentation: self.documentation,
            homepage: self.homepage,
            readme: self.readme,
            readme_file: self.readme_file,
            keywords: self.keywords,
            categories: self.categories,
            license: self.license,
            license_file: self.license_file,
            repository: self.repository,
            badges: self.badges,
            links: self.links,
            rust_version: self.rust_version,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReviewedRegistryDependency {
    optional: bool,
    default_features: bool,
    name: String,
    features: Vec<String>,
    version_req: String,
    target: Option<String>,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    registry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    explicit_name_in_toml: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bindep_target: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    lib: bool,
}

impl ReviewedRegistryDependency {
    fn into_new_dependency(self) -> NewCrateDependency {
        NewCrateDependency {
            optional: self.optional,
            default_features: self.default_features,
            name: self.name,
            features: self.features,
            version_req: self.version_req,
            target: self.target,
            kind: self.kind,
            registry: self.registry,
            explicit_name_in_toml: self.explicit_name_in_toml,
            artifact: self.artifact,
            bindep_target: self.bindep_target,
            lib: self.lib,
        }
    }
}

#[derive(Debug, Deserialize)]
struct VersionResponse {
    version: VersionRecord,
}

#[derive(Debug, Deserialize)]
struct VersionRecord {
    #[serde(rename = "crate")]
    crate_name: String,
    #[serde(rename = "num")]
    number: String,
    checksum: String,
    yanked: bool,
}

#[derive(Clone)]
struct UreqClient {
    agent: ureq::Agent,
    cancellation: ReleaseCancellation,
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
struct UreqClientError {
    message: String,
}

impl HttpClient for UreqClient {
    type Error = UreqClientError;

    fn request(
        &self,
        request: ureq::http::Request<Vec<u8>>,
    ) -> Result<ureq::http::Response<Vec<u8>>, Self::Error> {
        self.cancellation
            .check("crates.io upload")
            .map_err(transport_error)?;
        let mut response = self.agent.run(request).map_err(|error| UreqClientError {
            message: format!("registry transport failed: {error}"),
        })?;
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_REGISTRY_RESPONSE_BYTES)
            .read_to_vec()
            .map_err(|error| UreqClientError {
                message: format!(
                    "registry response exceeded its bound or could not be read: {error}"
                ),
            })?;
        self.cancellation
            .check("crates.io upload")
            .map_err(transport_error)?;
        let (parts, _) = response.into_parts();
        Ok(ureq::http::Response::from_parts(parts, body))
    }
}

fn transport_error(error: ReleaseError) -> UreqClientError {
    UreqClientError {
        message: error.to_string(),
    }
}
