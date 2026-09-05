use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::{
    CommandSurface, Family, FormatRoute, LEDGER_PATH, LEDGER_SCHEMA_VERSION, Ledger,
    MAX_LEDGER_BYTES, ParityError, StimIdentity, invalid, read_regular_file_bounded,
};
use crate::RepoRoot;
use crate::fixtures::FixtureId;

const FRAGMENT_ROOT: &str = "oracle/stim-v1.16-parity";
const MAX_FRAGMENTS: usize = 16;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    required_fixture_ids: Vec<FixtureId>,
    stim: StimIdentity,
    family_files: Vec<FragmentPath>,
    command_surfaces: Vec<CommandSurface>,
    format_routes: Vec<FormatRoute>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fragment {
    families: Vec<Family>,
}

#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
struct FragmentPath(PathBuf);

impl FragmentPath {
    fn as_path(&self) -> &Path {
        &self.0
    }
}

impl TryFrom<String> for FragmentPath {
    type Error = String;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let path = PathBuf::from(&raw);
        let valid_components = path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
        if raw.is_empty()
            || !valid_components
            || path.parent() != Some(Path::new(FRAGMENT_ROOT))
            || path.extension().and_then(|value| value.to_str()) != Some("toml")
        {
            return Err(format!(
                "family fragment path {raw:?} must be a normalized TOML file directly under {FRAGMENT_ROOT}"
            ));
        }
        Ok(Self(path))
    }
}

impl<'de> Deserialize<'de> for FragmentPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

pub(super) fn read(root: &RepoRoot) -> Result<Ledger, ParityError> {
    let (manifest, mut total_bytes) = read_manifest(root)?;
    if manifest.family_files.is_empty() || manifest.family_files.len() > MAX_FRAGMENTS {
        return Err(invalid(format!(
            "{LEDGER_PATH} must name between 1 and {MAX_FRAGMENTS} family fragments"
        )));
    }

    let mut previous: Option<&Path> = None;
    let mut families = Vec::new();
    for fragment_path in &manifest.family_files {
        if previous.is_some_and(|value| value >= fragment_path.as_path()) {
            return Err(invalid(format!(
                "family fragment {} is duplicated or out of order",
                fragment_path.as_path().display()
            )));
        }
        previous = Some(fragment_path.as_path());
        let absolute_path = root.path.join(fragment_path.as_path());
        let fragment_bytes = read_regular_file_bounded(&absolute_path, MAX_LEDGER_BYTES)?;
        total_bytes = total_bytes
            .checked_add(fragment_bytes.len())
            .filter(|total| *total <= MAX_LEDGER_BYTES)
            .ok_or_else(|| {
                invalid(format!(
                    "parity manifest and family fragments exceed {MAX_LEDGER_BYTES} bytes"
                ))
            })?;
        let fragment = parse::<Fragment>(&absolute_path, &fragment_bytes)?;
        if fragment.families.is_empty() {
            return Err(invalid(format!(
                "family fragment {} is empty",
                fragment_path.as_path().display()
            )));
        }
        families.extend(fragment.families);
    }

    Ok(Ledger {
        schema_version: manifest.schema_version,
        required_fixture_ids: manifest.required_fixture_ids,
        stim: manifest.stim,
        command_surfaces: manifest.command_surfaces,
        format_routes: manifest.format_routes,
        families,
    })
}

pub(crate) fn required_fixture_ids(root: &RepoRoot) -> Result<Vec<FixtureId>, ParityError> {
    read_manifest(root).map(|(manifest, _)| manifest.required_fixture_ids)
}

fn read_manifest(root: &RepoRoot) -> Result<(Manifest, usize), ParityError> {
    let path = root.path.join(LEDGER_PATH);
    let bytes = read_regular_file_bounded(&path, MAX_LEDGER_BYTES)?;
    let manifest = parse::<Manifest>(&path, &bytes)?;
    if manifest.schema_version != LEDGER_SCHEMA_VERSION {
        return Err(invalid(format!(
            "schema_version is {}, expected {LEDGER_SCHEMA_VERSION}",
            manifest.schema_version
        )));
    }
    if manifest.required_fixture_ids.is_empty() {
        return Err(invalid(
            "required_fixture_ids must not be empty".to_string(),
        ));
    }
    Ok((manifest, bytes.len()))
}

fn parse<T>(path: &Path, bytes: &[u8]) -> Result<T, ParityError>
where
    T: for<'de> Deserialize<'de>,
{
    let text = std::str::from_utf8(bytes)
        .map_err(|error| invalid(format!("{} is not UTF-8: {error}", path.display())))?;
    toml::from_str(text).map_err(|source| ParityError::Parse {
        path: path.to_path_buf().into_boxed_path(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::{FragmentPath, LEDGER_PATH, required_fixture_ids};
    use crate::RepoRoot;

    #[test]
    fn fixture_requirements_reject_missing_empty_and_obsolete_root_contracts() {
        let directory = tempfile::tempdir().expect("temporary repository");
        let root = RepoRoot {
            path: directory.path().to_path_buf(),
        };
        std::fs::create_dir(root.path.join("oracle")).expect("oracle directory");
        let source = include_str!("../../../../oracle/stim-v1.16-parity.toml");
        let mut manifest = source.parse::<toml::Table>().expect("root manifest");
        manifest.insert(
            "required_fixture_ids".to_string(),
            toml::Value::Array(vec![toml::Value::String("required".to_string())]),
        );
        let write = |value: &toml::Table| {
            std::fs::write(
                root.path.join(LEDGER_PATH),
                toml::to_string(value).expect("manifest TOML"),
            )
            .expect("write manifest")
        };
        write(&manifest);
        required_fixture_ids(&root).expect("root requirements do not load family fragments");

        for (field, replacement, expected) in [
            (
                "required_fixture_ids",
                None,
                "missing field `required_fixture_ids`",
            ),
            (
                "required_fixture_ids",
                Some(toml::Value::Array(Vec::new())),
                "required_fixture_ids must not be empty",
            ),
            (
                "schema_version",
                Some(toml::Value::Integer(1)),
                "schema_version is 1, expected 2",
            ),
        ] {
            let mut changed = manifest.clone();
            match replacement {
                Some(value) => {
                    changed.insert(field.to_string(), value);
                }
                None => {
                    changed.remove(field);
                }
            }
            write(&changed);
            let error = required_fixture_ids(&root).expect_err("incomplete requirements must fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
    }

    #[test]
    fn fragment_paths_are_normalized_and_confined_to_the_ledger_directory() {
        let valid = "oracle/stim-v1.16-parity/families-a.toml".to_string();
        assert!(FragmentPath::try_from(valid).is_ok());

        for invalid in [
            "",
            "/tmp/families.toml",
            "oracle/stim-v1.16-parity.toml",
            "oracle/stim-v1.16-parity/../families.toml",
            "oracle/stim-v1.16-parity/nested/families.toml",
            "oracle/stim-v1.16-parity/families.json",
        ] {
            assert!(
                FragmentPath::try_from(invalid.to_string()).is_err(),
                "{invalid}"
            );
        }
    }
}
