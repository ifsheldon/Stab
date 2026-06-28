//! Benchmark comparability classes used by M12 gate evidence.

use serde::{Deserialize, Serialize};

use crate::manifest::Runner;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum ComparabilityClass {
    #[default]
    #[serde(rename = "unspecified")]
    Unspecified,
    #[serde(rename = "direct-match")]
    DirectMatch,
    #[serde(rename = "cli-baseline")]
    CliBaseline,
    #[serde(rename = "contract-representative")]
    ContractRepresentative,
    #[serde(rename = "contract-proxy")]
    ContractProxy,
    #[serde(rename = "contract-smoke")]
    ContractSmoke,
    #[serde(rename = "partial-match")]
    PartialMatch,
    #[serde(rename = "report-only")]
    ReportOnly,
    #[serde(rename = "contract-only")]
    ContractOnly,
}

impl ComparabilityClass {
    pub(crate) fn from_note_and_runner(note: Option<&str>, runner: Runner) -> Self {
        if let Some(prefix) = note.and_then(note_prefix) {
            return match prefix {
                "direct-match" => Self::DirectMatch,
                "cli-baseline" => Self::CliBaseline,
                "contract-representative" => Self::ContractRepresentative,
                "contract-proxy" => Self::ContractProxy,
                "contract-smoke" => Self::ContractSmoke,
                "partial-match" => Self::PartialMatch,
                "report-only" => Self::ReportOnly,
                "contract-only" | "contract-only baseline" => Self::ContractOnly,
                _ => Self::Unspecified,
            };
        }
        if runner == Runner::ContractOnly {
            Self::ContractOnly
        } else {
            Self::Unspecified
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::DirectMatch => "direct-match",
            Self::CliBaseline => "cli-baseline",
            Self::ContractRepresentative => "contract-representative",
            Self::ContractProxy => "contract-proxy",
            Self::ContractSmoke => "contract-smoke",
            Self::PartialMatch => "partial-match",
            Self::ReportOnly => "report-only",
            Self::ContractOnly => "contract-only",
        }
    }

    pub(crate) fn allows_positional_measurement_pairs(self) -> bool {
        self == Self::DirectMatch
    }
}

fn note_prefix(note: &str) -> Option<&str> {
    note.split_once(':')
        .map(|(prefix, _)| prefix.trim())
        .filter(|prefix| !prefix.is_empty())
}

#[cfg(test)]
mod tests {
    use super::ComparabilityClass;
    use crate::manifest::Runner;

    #[test]
    fn comparability_class_is_parsed_from_note_prefix() {
        for (prefix, expected) in [
            ("direct-match", ComparabilityClass::DirectMatch),
            ("cli-baseline", ComparabilityClass::CliBaseline),
            (
                "contract-representative",
                ComparabilityClass::ContractRepresentative,
            ),
            ("contract-proxy", ComparabilityClass::ContractProxy),
            ("contract-smoke", ComparabilityClass::ContractSmoke),
            ("partial-match", ComparabilityClass::PartialMatch),
            ("report-only", ComparabilityClass::ReportOnly),
            ("contract-only", ComparabilityClass::ContractOnly),
        ] {
            assert_eq!(
                ComparabilityClass::from_note_and_runner(
                    Some(&format!("{prefix}: explanation")),
                    Runner::StimPerf,
                ),
                expected
            );
        }
    }

    #[test]
    fn contract_only_runner_defaults_to_contract_only_class() {
        assert_eq!(
            ComparabilityClass::from_note_and_runner(None, Runner::ContractOnly),
            ComparabilityClass::ContractOnly
        );
        assert_eq!(
            ComparabilityClass::from_note_and_runner(None, Runner::StimPerf),
            ComparabilityClass::Unspecified
        );
    }
}
