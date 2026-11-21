//! The parity ledger's deserialized schema.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Ledger {
    pub(super) schema_version: u32,
    pub(super) stim: StimIdentity,
    pub(super) command_surfaces: Vec<CommandSurface>,
    pub(super) format_routes: Vec<FormatRoute>,
    pub(super) families: Vec<Family>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CommandSurface {
    pub(super) command: String,
    pub(super) options: Vec<String>,
    pub(super) stim_refs: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StimIdentity {
    pub(super) version: String,
    pub(super) commit: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FormatRoute {
    pub(super) id: String,
    pub(super) command: String,
    pub(super) role: FormatRouteRole,
    pub(super) record_types: Vec<RecordType>,
    pub(super) accepted_formats: Vec<String>,
    #[serde(default)]
    pub(super) rejected_formats: Vec<String>,
    #[serde(default)]
    pub(super) stim_bug_divergences: Vec<String>,
    #[serde(default)]
    pub(super) dets_observable_order: Option<DetsObservableOrder>,
    pub(super) stim_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub(super) enum FormatRouteRole {
    Input,
    Output,
    SideOutput,
    ReplayInput,
}

impl FormatRouteRole {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::Output => "output",
            Self::SideOutput => "side output",
            Self::ReplayInput => "replay input",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum RecordType {
    M,
    D,
    L,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum DetsObservableOrder {
    PrependByDefault,
}

impl DetsObservableOrder {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::PrependByDefault => "prepend by default",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RouteOutcome {
    Accepted,
    Rejected,
    StimBugDivergence,
}

impl RecordType {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::M => "M",
            Self::D => "D",
            Self::L => "L",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct Family {
    pub(super) id: String,
    pub(super) area: Area,
    pub(super) contract: String,
    pub(super) stim_refs: Vec<String>,
    #[serde(default)]
    pub(super) coverage: Vec<String>,
    #[serde(flatten)]
    pub(super) disposition: Disposition,
}

impl Family {
    pub(super) const fn status(&self) -> Status {
        match &self.disposition {
            Disposition::Done { .. } => Status::Done,
            Disposition::Missing { .. } => Status::Missing,
            Disposition::Deferred { .. } => Status::Deferred,
            Disposition::Divergence { .. } => Status::Divergence,
        }
    }

    pub(super) fn owner(&self) -> Option<&str> {
        match &self.disposition {
            Disposition::Done { owner, .. }
            | Disposition::Missing { owner, .. }
            | Disposition::Divergence { owner, .. } => Some(owner),
            Disposition::Deferred { .. } => None,
        }
    }

    pub(super) const fn test(&self) -> Option<&TestOwner> {
        match &self.disposition {
            Disposition::Done {
                evidence: Evidence::Verified { test, .. },
                ..
            }
            | Disposition::Divergence {
                evidence: Evidence::Verified { test, .. },
                ..
            } => Some(test),
            Disposition::Done {
                evidence: Evidence::NeedsOwner { .. },
                ..
            }
            | Disposition::Divergence {
                evidence: Evidence::NeedsOwner { .. },
                ..
            }
            | Disposition::Missing { .. }
            | Disposition::Deferred { .. } => None,
        }
    }

    pub(super) const fn stim_reproduction(&self) -> Option<&TestOwner> {
        match &self.disposition {
            Disposition::Divergence {
                evidence:
                    Evidence::Verified {
                        stim_reproduction: Some(test),
                        ..
                    },
                ..
            } => Some(test),
            _ => None,
        }
    }

    pub(super) const fn evidence_status(&self) -> EvidenceStatus {
        match &self.disposition {
            Disposition::Done {
                evidence: Evidence::Verified { .. },
                ..
            }
            | Disposition::Divergence {
                evidence: Evidence::Verified { .. },
                ..
            } => EvidenceStatus::Verified,
            Disposition::Done {
                evidence: Evidence::NeedsOwner { .. },
                ..
            }
            | Disposition::Divergence {
                evidence: Evidence::NeedsOwner { .. },
                ..
            } => EvidenceStatus::NeedsOwner,
            Disposition::Missing { .. } | Disposition::Deferred { .. } => {
                EvidenceStatus::NotApplicable
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Area {
    Algebra,
    Analysis,
    CircuitModel,
    Cli,
    DemModel,
    Detection,
    GateCatalog,
    Generation,
    ProductSurface,
    ResourceSafety,
    ResultFormats,
    Sampling,
    Search,
    StabExtensions,
    Transforms,
}

impl Area {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Algebra => "algebra",
            Self::Analysis => "analysis",
            Self::CircuitModel => "circuit-model",
            Self::Cli => "cli",
            Self::DemModel => "dem-model",
            Self::Detection => "detection",
            Self::GateCatalog => "gate-catalog",
            Self::Generation => "generation",
            Self::ProductSurface => "product-surface",
            Self::ResourceSafety => "resource-safety",
            Self::ResultFormats => "result-formats",
            Self::Sampling => "sampling",
            Self::Search => "search",
            Self::StabExtensions => "stab-extensions",
            Self::Transforms => "transforms",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Disposition {
    Done {
        owner: String,
        evidence: Evidence,
    },
    Missing {
        owner: String,
        milestone: Milestone,
    },
    Deferred {
        rationale: String,
    },
    Divergence {
        owner: String,
        divergence_kind: DivergenceKind,
        rationale: String,
        evidence: Evidence,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Evidence {
    Verified {
        test: TestOwner,
        #[serde(default)]
        stim_reproduction: Option<TestOwner>,
    },
    NeedsOwner {
        milestone: Milestone,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum EvidenceStatus {
    Verified,
    NeedsOwner,
    NotApplicable,
}

impl EvidenceStatus {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::NeedsOwner => "needs-owner",
            Self::NotApplicable => "not-applicable",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Status {
    Done,
    Missing,
    Deferred,
    Divergence,
}

impl Status {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Done => "done",
            Self::Missing => "missing",
            Self::Deferred => "deferred",
            Self::Divergence => "divergence",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
pub(super) enum Milestone {
    P1,
    P2,
    P3,
    P4,
    P5,
    P6,
}

impl Milestone {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::P1 => "P1",
            Self::P2 => "P2",
            Self::P3 => "P3",
            Self::P4 => "P4",
            Self::P5 => "P5",
            Self::P6 => "P6",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum DivergenceKind {
    StimBug,
    ResourceLimit,
    StabExtension,
}

impl DivergenceKind {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::StimBug => "Stim bug",
            Self::ResourceLimit => "resource limit",
            Self::StabExtension => "Stab extension",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct TestOwner {
    pub(super) package: String,
    #[serde(flatten)]
    pub(super) target: CargoTestTarget,
    pub(super) name: String,
}

impl TestOwner {
    /// The Cargo arguments selecting this owner's test target.
    fn cargo_target_args(&self) -> Vec<&str> {
        match &self.target {
            CargoTestTarget::Lib => vec!["--lib"],
            CargoTestTarget::Bin { target } => vec!["--bin", target.as_str()],
            CargoTestTarget::Test { target } => vec!["--test", target.as_str()],
        }
    }

    pub(super) fn display(&self) -> String {
        format!(
            "cargo test -p {} {} -- {} --exact --include-ignored",
            self.package,
            self.cargo_target_args().join(" "),
            self.name
        )
    }

    pub(super) fn listing_group_key(&self) -> String {
        format!("{}|{}", self.package, self.cargo_target_args().join("|"))
    }

    pub(super) fn listing_all_args(&self) -> Vec<&str> {
        let mut args = vec!["test", "-p", self.package.as_str()];
        args.extend(self.cargo_target_args());
        args.extend(["--quiet", "--", "--list"]);
        args
    }

    pub(super) fn run_args(&self) -> Vec<&str> {
        let mut args = vec!["test", "-p", self.package.as_str()];
        args.extend(self.cargo_target_args());
        args.extend([
            "--quiet",
            "--",
            self.name.as_str(),
            "--exact",
            "--include-ignored",
        ]);
        args
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum CargoTestTarget {
    Lib,
    Bin { target: String },
    Test { target: String },
}
