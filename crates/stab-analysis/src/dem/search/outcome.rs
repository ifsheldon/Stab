//! The options and the outcome of the graphlike and hypergraph logical-error searches.

use std::fmt::{Display, Formatter};

use stab_model::DetectorErrorModel;

use super::budget::LogicalErrorSearchLimits;

/// Whether the hypergraph search crosses an edge that grows the set of detection events.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SymptomGrowth {
    /// Cross every edge that keeps the detection-event set within the size limit.
    Explore,
    /// Skip an edge when crossing it would leave more detection events than the current state
    /// has. The search is then much faster but may miss errors the full search would find.
    Skip,
}

/// What the graphlike search does with errors that are not single graphlike edges.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum UngraphlikeErrors {
    /// Leave out of the graph every error that has a `^` separator and every error with more
    /// than two detection events, as Stim does.
    #[default]
    Ignore,
    /// Split each error at its `^` separators into graphlike components, and fail with
    /// [`crate::AnalysisError::NonGraphlikeError`] on a component with more than two detection
    /// events.
    Reject,
}

impl UngraphlikeErrors {
    pub(super) const fn ignored(self) -> bool {
        matches!(self, Self::Ignore)
    }
}

/// Options of [`super::find_undetectable_logical_error`].
///
/// There is no default: the two size limits decide how much of the model the heuristic sees, and
/// Stim's Python API also requires all three search arguments.
///
/// When both `max_detection_event_set_size` and `max_edge_degree` are 2, the call runs the
/// graphlike search with [`UngraphlikeErrors::Ignore`] instead, as Stim does. That search leaves
/// out every error with a `^` separator, where the hypergraph search toggles all of an error's
/// targets into one edge, so the two can find different witnesses or one can find none; a
/// [`LogicalErrorSearch::NotFound`] report then names the graphlike search. `symptom_growth` has
/// no effect at these maxima either way, since crossing an edge of at most two detectors at the
/// state's first detector never grows the detection-event set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HypergraphSearchOptions {
    /// Search states with more detection events than this are not explored.
    pub max_detection_event_set_size: usize,
    /// Errors with more detection events than this are left out of the search graph.
    pub max_edge_degree: usize,
    pub symptom_growth: SymptomGrowth,
    pub limits: LogicalErrorSearchLimits,
}

/// Options of [`super::shortest_graphlike_undetectable_logical_error`].
///
/// The default ignores ungraphlike error components, as Stim's Python API does, and uses the
/// default search limits.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GraphlikeSearchOptions {
    pub ungraphlike_errors: UngraphlikeErrors,
    pub limits: LogicalErrorSearchLimits,
}

/// The outcome of a logical-error search that ran to completion.
///
/// A search stopped by a [`LogicalErrorSearchLimits`] budget has not shown that no error exists,
/// so it is an [`crate::AnalysisError::ResourceLimit`] and never `NotFound`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
#[must_use]
pub enum LogicalErrorSearch {
    /// An undetectable logical error, as error mechanisms of probability 1.
    Found(DetectorErrorModel),
    /// The search explored every admissible state without finding one.
    NotFound(NoLogicalErrorReport),
}

impl LogicalErrorSearch {
    /// The error found, if any.
    pub const fn found(&self) -> Option<&DetectorErrorModel> {
        match self {
            Self::Found(model) => Some(model),
            Self::NotFound(_) => None,
        }
    }

    /// The error found, if any.
    pub fn into_found(self) -> Option<DetectorErrorModel> {
        match self {
            Self::Found(model) => Some(model),
            Self::NotFound(_) => None,
        }
    }

    /// The report of a search that found nothing.
    pub const fn not_found(&self) -> Option<NoLogicalErrorReport> {
        match self {
            Self::Found(_) => None,
            Self::NotFound(report) => Some(*report),
        }
    }
}

/// Which logical-error search found nothing.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LogicalErrorSearchKind {
    Hypergraph,
    Graphlike,
}

/// A logical-error search that explored every admissible state without finding an undetectable
/// logical error, with the facts that make the result vacuous.
///
/// `Display` renders Stim's message for the same outcome.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct NoLogicalErrorReport {
    search: LogicalErrorSearchKind,
    no_observables: bool,
    no_detectors: bool,
    no_errors: bool,
    no_graphlike_errors: bool,
}

impl NoLogicalErrorReport {
    pub(super) const fn hypergraph(
        no_observables: bool,
        no_detectors: bool,
        no_errors: bool,
    ) -> Self {
        Self {
            search: LogicalErrorSearchKind::Hypergraph,
            no_observables,
            no_detectors,
            no_errors,
            no_graphlike_errors: false,
        }
    }

    /// `no_graphlike_errors` is only meaningful when the model has errors.
    pub(super) const fn graphlike(
        no_observables: bool,
        no_detectors: bool,
        no_errors: bool,
        no_graphlike_errors: bool,
    ) -> Self {
        Self {
            search: LogicalErrorSearchKind::Graphlike,
            no_observables,
            no_detectors,
            no_errors,
            no_graphlike_errors: !no_errors && no_graphlike_errors,
        }
    }

    pub const fn search(self) -> LogicalErrorSearchKind {
        self.search
    }

    pub const fn no_observables(self) -> bool {
        self.no_observables
    }

    pub const fn no_detectors(self) -> bool {
        self.no_detectors
    }

    pub const fn no_errors(self) -> bool {
        self.no_errors
    }

    /// The model has errors but none with at most two detection events.
    pub const fn no_graphlike_errors(self) -> bool {
        self.no_graphlike_errors
    }
}

impl Display for NoLogicalErrorReport {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.search {
            LogicalErrorSearchKind::Hypergraph => "Failed to find any logical errors.",
            LogicalErrorSearchKind::Graphlike => "Failed to find any graphlike logical errors.",
        })?;
        if self.no_observables {
            formatter.write_str(
                "\n    WARNING: NO OBSERVABLES. The circuit or detector error model didn't define any observables, making it vacuously impossible to find a logical error.",
            )?;
        }
        if self.no_detectors {
            formatter.write_str(
                "\n    WARNING: NO DETECTORS. The circuit or detector error model didn't define any detectors.",
            )?;
        }
        if self.no_errors {
            formatter.write_str(
                "\n    WARNING: NO ERRORS. The circuit or detector error model didn't include any errors, making it vacuously impossible to find a logical error.",
            )?;
        } else if self.no_graphlike_errors {
            formatter.write_str(
                "\n    WARNING: NO GRAPHLIKE ERRORS. Although the circuit or detector error model does define some errors, none of them are graphlike (i.e. have at most two detection events), making it vacuously impossible to find a graphlike logical error.",
            )?;
        }
        Ok(())
    }
}
