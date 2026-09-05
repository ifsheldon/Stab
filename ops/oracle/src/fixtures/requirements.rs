use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;

use super::{FixtureError, FixtureManifest, FixtureStatus};
use crate::RepoRoot;

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(try_from = "String")]
pub(crate) struct FixtureId(String);

impl FixtureId {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for FixtureId {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(format!("invalid fixture id {value:?}"));
        }
        Ok(Self(value))
    }
}

impl fmt::Display for FixtureId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub(crate) fn check_required_fixtures(
    root: &RepoRoot,
    required: &[FixtureId],
) -> Result<(), FixtureError> {
    let manifest = FixtureManifest::read(root)?;
    let mut violations = Vec::new();
    manifest.check_required_ids(required, &mut violations);
    if violations.is_empty() {
        Ok(())
    } else {
        Err(FixtureError::Validation(
            violations.join("\n").into_boxed_str(),
        ))
    }
}

impl FixtureManifest {
    pub(super) fn check_required_ids(&self, required: &[FixtureId], violations: &mut Vec<String>) {
        if required.is_empty() {
            violations.push("required_fixture_ids must not be empty".to_string());
        }
        let mut rows = BTreeMap::new();
        for row in &self.rows {
            if rows.insert(&row.id, row).is_some() {
                violations.push(format!("duplicate fixture id {}", row.id));
            }
        }
        let mut seen = BTreeSet::new();
        for id in required {
            if !seen.insert(id) {
                violations.push(format!("duplicate required fixture id {id}"));
            }
            match rows.get(id) {
                Some(row) if row.status == FixtureStatus::Implemented => {}
                Some(row) => violations.push(format!(
                    "required fixture {id} is {}, expected implemented",
                    row.status.as_str()
                )),
                None => violations.push(format!("required fixture {id} has no manifest row")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FixtureId, FixtureManifest, FixtureStatus};

    const HEADER: &str = "id,milestone,upstream_source,parity_mode,comparator,command_shape,argv,stdin_path,expected_stdout_path,expected_status,expected_stderr_class,status,statistical_plan,source_license_note\n";

    fn manifest() -> FixtureManifest {
        FixtureManifest::from_csv(&format!(
            "{HEADER}required,M4,src/stim/circuit/circuit.test.cc,structural,structural,parse circuit,core-circuit-parse-print,,,0,any,implemented,round trip,hand-authored\noptional,M4,src/stim/circuit/circuit.test.cc,structural,structural,parse circuit,core-circuit-parse-print,,,0,any,manifest-only,round trip,hand-authored\n"
        ))
        .expect("fixture manifest")
    }

    fn required() -> Vec<FixtureId> {
        vec![FixtureId::try_from("required".to_string()).expect("fixture id")]
    }

    #[test]
    fn required_fixtures_resolve_once_and_must_remain_implemented() {
        let manifest = manifest();
        let mut errors = Vec::new();
        manifest.check_required_ids(&required(), &mut errors);
        assert!(errors.is_empty(), "{errors:?}");

        for status in [
            FixtureStatus::ManifestOnly,
            FixtureStatus::Red,
            FixtureStatus::Ignored,
        ] {
            let mut changed = manifest.clone();
            changed.rows.first_mut().expect("required row").status = status;
            let mut errors = Vec::new();
            changed.check_required_ids(&required(), &mut errors);
            assert!(
                errors
                    .iter()
                    .any(|error| error.contains("required fixture required is")),
                "{errors:?}"
            );
        }

        let mut missing = manifest.clone();
        missing.rows.retain(|row| row.id.as_str() != "required");
        let mut errors = Vec::new();
        missing.check_required_ids(&required(), &mut errors);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("required fixture required has no manifest row")),
            "{errors:?}"
        );

        let mut duplicated = manifest.clone();
        duplicated
            .rows
            .push(manifest.rows.first().expect("required row").clone());
        let mut errors = Vec::new();
        duplicated.check_required_ids(&required(), &mut errors);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("duplicate fixture id required")),
            "{errors:?}"
        );
    }

    #[test]
    fn fixture_requirements_reject_empty_duplicate_and_unknown_references() {
        let manifest = manifest();
        let mut duplicate = required();
        duplicate.extend(required());
        for (references, expected) in [
            (Vec::new(), "required_fixture_ids must not be empty"),
            (duplicate, "duplicate required fixture id required"),
            (
                vec![FixtureId::try_from("unknown".to_string()).expect("fixture id")],
                "required fixture unknown has no manifest row",
            ),
        ] {
            let mut errors = Vec::new();
            manifest.check_required_ids(&references, &mut errors);
            assert!(
                errors.iter().any(|error| error.contains(expected)),
                "{errors:?}"
            );
        }
    }
}
