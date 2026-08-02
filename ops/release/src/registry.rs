use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::ReleaseError;

const CRATES_IO_API: &str = "https://crates.io/api/v1/crates";
const MAX_REGISTRY_RESPONSE_BYTES: u64 = 1 << 20;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const VISIBILITY_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const VISIBILITY_POLL: Duration = Duration::from_secs(10);

pub(crate) trait RegistryLookup {
    fn checksum(&self, package: &str, version: &str) -> Result<Option<String>, ReleaseError>;
}

#[derive(Debug)]
pub(crate) struct CratesIo {
    agent: ureq::Agent,
    base_url: String,
}

impl CratesIo {
    pub(crate) fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build();
        Self {
            agent: config.into(),
            base_url: CRATES_IO_API.to_string(),
        }
    }

    #[cfg(test)]
    fn with_base_url(base_url: String) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build();
        Self {
            agent: config.into(),
            base_url,
        }
    }
}

impl RegistryLookup for CratesIo {
    fn checksum(&self, package: &str, version: &str) -> Result<Option<String>, ReleaseError> {
        validate_identifier(package)?;
        validate_identifier(version)?;
        let url = format!("{}/{package}/{version}", self.base_url);
        let response = self
            .agent
            .get(&url)
            .header("Accept", "application/json")
            .header("User-Agent", "stab-release/0.2.0")
            .call();
        let mut response = match response {
            Ok(response) => response,
            Err(ureq::Error::StatusCode(404)) => return Ok(None),
            Err(error) => {
                return Err(ReleaseError::Registry(format!(
                    "failed to query {package} {version}: {error}"
                )));
            }
        };
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
        parse_checksum(&body, package, version).map(Some)
    }
}

pub(crate) fn require_absent_or_matching(
    registry: &impl RegistryLookup,
    package: &str,
    version: &str,
    expected: &str,
) -> Result<bool, ReleaseError> {
    match registry.checksum(package, version)? {
        None => Ok(false),
        Some(actual) if actual == expected => Ok(true),
        Some(actual) => Err(ReleaseError::RegistryChecksum {
            package: package.to_string(),
            version: version.to_string(),
            expected: expected.to_string(),
            actual,
        }),
    }
}

pub(crate) fn wait_for_matching_checksum(
    registry: &impl RegistryLookup,
    package: &str,
    version: &str,
    expected: &str,
) -> Result<(), ReleaseError> {
    let started = Instant::now();
    while started.elapsed() < VISIBILITY_TIMEOUT {
        match registry.checksum(package, version)? {
            Some(actual) if actual == expected => return Ok(()),
            Some(actual) => {
                return Err(ReleaseError::RegistryChecksum {
                    package: package.to_string(),
                    version: version.to_string(),
                    expected: expected.to_string(),
                    actual,
                });
            }
            None => std::thread::sleep(VISIBILITY_POLL),
        }
    }
    Err(ReleaseError::RegistryVisibility {
        package: package.to_string(),
        version: version.to_string(),
        checksum: expected.to_string(),
    })
}

fn parse_checksum(body: &str, package: &str, version: &str) -> Result<String, ReleaseError> {
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
    if response.version.checksum.len() != 64
        || !response
            .version
            .checksum
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ReleaseError::Registry(format!(
            "crates.io returned an invalid checksum for {package} {version}"
        )));
    }
    Ok(response.version.checksum)
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
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    #[test]
    fn registry_checksum_response_is_identity_checked() {
        let checksum = "a".repeat(64);
        let body = format!(
            "{{\"version\":{{\"crate\":\"stab-core\",\"num\":\"0.2.0\",\"checksum\":\"{checksum}\"}}}}"
        );
        assert_eq!(
            parse_checksum(&body, "stab-core", "0.2.0").expect("checksum"),
            checksum
        );
        assert!(parse_checksum(&body, "stab-cli", "0.2.0").is_err());
    }

    #[test]
    fn registry_lookup_distinguishes_missing_and_matching_versions() {
        let checksum = "b".repeat(64);
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let body = format!(
            "{{\"version\":{{\"crate\":\"stab-core\",\"num\":\"0.2.0\",\"checksum\":\"{checksum}\"}}}}"
        );
        let server = thread::spawn(move || {
            for (index, stream) in listener.incoming().take(2).enumerate() {
                let mut stream = stream.expect("request");
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request).expect("read request");
                if index == 0 {
                    write!(
                        stream,
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .expect("404");
                } else {
                    write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    )
                    .expect("200");
                }
            }
        });
        let registry = CratesIo::with_base_url(format!("http://{address}"));
        assert_eq!(
            registry.checksum("stab-core", "0.2.0").expect("missing"),
            None
        );
        assert_eq!(
            registry.checksum("stab-core", "0.2.0").expect("present"),
            Some(checksum)
        );
        server.join().expect("server");
    }

    struct FixedRegistry(Option<String>);

    impl RegistryLookup for FixedRegistry {
        fn checksum(&self, _package: &str, _version: &str) -> Result<Option<String>, ReleaseError> {
            Ok(self.0.clone())
        }
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
                &FixedRegistry(Some(expected.clone())),
                "stab-core",
                "0.2.0",
                &expected
            )
            .expect("matching")
        );
        assert!(matches!(
            require_absent_or_matching(
                &FixedRegistry(Some("d".repeat(64))),
                "stab-core",
                "0.2.0",
                &expected
            ),
            Err(ReleaseError::RegistryChecksum { .. })
        ));
    }
}
