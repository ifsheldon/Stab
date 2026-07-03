//! Fixture milestone identifiers.

use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Milestone {
    #[serde(rename = "M0")]
    M0,
    #[serde(rename = "M4")]
    M4,
    #[serde(rename = "M5")]
    M5,
    #[serde(rename = "M6")]
    M6,
    #[serde(rename = "M7")]
    M7,
    #[serde(rename = "M8")]
    M8,
    #[serde(rename = "M9")]
    M9,
    #[serde(rename = "M10")]
    M10,
    #[serde(rename = "M11")]
    M11,
    #[serde(rename = "M12")]
    M12,
    #[serde(rename = "PF1")]
    Pf1,
    #[serde(rename = "PF2")]
    Pf2,
    #[serde(rename = "PF3")]
    Pf3,
    #[serde(rename = "PF4")]
    Pf4,
    #[serde(rename = "PF5")]
    Pf5,
    #[serde(rename = "PF6")]
    Pf6,
    #[serde(rename = "PF7")]
    Pf7,
}

impl Milestone {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "M0" => Ok(Self::M0),
            "M4" => Ok(Self::M4),
            "M5" => Ok(Self::M5),
            "M6" => Ok(Self::M6),
            "M7" => Ok(Self::M7),
            "M8" => Ok(Self::M8),
            "M9" => Ok(Self::M9),
            "M10" => Ok(Self::M10),
            "M11" => Ok(Self::M11),
            "M12" => Ok(Self::M12),
            "PF1" => Ok(Self::Pf1),
            "PF2" => Ok(Self::Pf2),
            "PF3" => Ok(Self::Pf3),
            "PF4" => Ok(Self::Pf4),
            "PF5" => Ok(Self::Pf5),
            "PF6" => Ok(Self::Pf6),
            "PF7" => Ok(Self::Pf7),
            _ => Err(value.to_string()),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::M0 => "M0",
            Self::M4 => "M4",
            Self::M5 => "M5",
            Self::M6 => "M6",
            Self::M7 => "M7",
            Self::M8 => "M8",
            Self::M9 => "M9",
            Self::M10 => "M10",
            Self::M11 => "M11",
            Self::M12 => "M12",
            Self::Pf1 => "PF1",
            Self::Pf2 => "PF2",
            Self::Pf3 => "PF3",
            Self::Pf4 => "PF4",
            Self::Pf5 => "PF5",
            Self::Pf6 => "PF6",
            Self::Pf7 => "PF7",
        }
    }
}
