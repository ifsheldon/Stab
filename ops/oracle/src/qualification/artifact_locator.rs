use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct ReportRootRelativePath(PathBuf);

impl ReportRootRelativePath {
    pub(super) fn try_new(path: PathBuf) -> Result<Self, String> {
        if path.as_os_str().is_empty()
            || path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!(
                "report artifact path {path:?} is not report-root-relative"
            ));
        }
        Ok(Self(path))
    }

    pub(super) fn as_path(&self) -> &Path {
        &self.0
    }
}

impl Serialize for ReportRootRelativePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ReportRootRelativePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path = PathBuf::deserialize(deserializer)?;
        Self::try_new(path).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_root_relative_paths_reject_external_locations() {
        assert!(ReportRootRelativePath::try_new(PathBuf::new()).is_err());
        assert!(ReportRootRelativePath::try_new(PathBuf::from("/tmp/failure")).is_err());
        assert!(ReportRootRelativePath::try_new(PathBuf::from("cases/../failure")).is_err());
        assert!(ReportRootRelativePath::try_new(PathBuf::from("cases/case-a/failure.txt")).is_ok());
    }

    #[test]
    fn deserialization_rejects_traversal_before_report_validation() {
        let error = serde_json::from_str::<ReportRootRelativePath>(r#""cases/../failure""#)
            .expect_err("reject traversal");
        assert!(error.to_string().contains("not report-root-relative"));
    }
}
