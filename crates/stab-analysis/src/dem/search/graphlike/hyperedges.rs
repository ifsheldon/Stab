//! The graphlike graph of the hypergraph search's edges of at most two detectors.
//!
//! With both of its maxima at 2, the hypergraph search only ever crosses edges of at most two
//! detectors and holds at most two detection events, so its search space is a graph. This module
//! builds that graph in the graphlike representation, with the hypergraph's edge semantics: each
//! error's targets fold into one edge across `^` separators, and the folded edges of more than two
//! detectors are left out.

use stab_model::{DemTarget, DetectorErrorModel};

use super::{Edge, Graph};
use crate::dem::search::budget::LogicalErrorSearchLimits;
use crate::dem::search::error_traversal::SearchGraphTargetPolicy;
use crate::dem::search::values::folded_error_targets;
use crate::resources::LogicalErrorSearchResource;
use crate::{AnalysisError, AnalysisOperation, AnalysisResult, ResourceLimitError};

/// The search the caller asked for, named in this graph's resource-limit errors.
const HYPERGRAPH_SEARCH: &str = "hypergraph search";

/// The largest folded edge the graph keeps, the hypergraph search's `max_edge_degree`.
const MAX_EDGE_DEGREE: usize = 2;

impl Graph {
    /// Builds the graph the hypergraph search at maxima 2 explores.
    ///
    /// The detector nodes, the expanded-traversal limits, `max_hyperedge_degree`, and
    /// `max_hyperedge_incidences` apply exactly as in the hypergraph search's graph; the retained
    /// graph limits count this representation, which stores an edge at each of its detectors.
    pub(in crate::dem::search) fn from_dem_as_two_detector_hyperedges(
        model: &DetectorErrorModel,
        limits: &LogicalErrorSearchLimits,
    ) -> AnalysisResult<Self> {
        let mut incidences = 0;
        Self::from_dem_errors(
            model,
            HYPERGRAPH_SEARCH,
            SearchGraphTargetPolicy::Hypergraph {
                max_weight: MAX_EDGE_DEGREE.min(limits.max_hyperedge_degree),
            },
            limits,
            |graph, targets| graph.add_two_detector_hyperedge(targets, &mut incidences),
        )
    }

    /// Adds an error's folded edge if it has at most two detectors, as the hypergraph graph
    /// does, counting its detector incidences against `max_hyperedge_incidences`.
    ///
    /// A folded edge with no detectors and some observable is a distance-1 logical error. The
    /// first one is kept, as Stim's graphlike search keeps it, where the hypergraph search keeps
    /// the last; either is a shortest witness.
    fn add_two_detector_hyperedge(
        &mut self,
        targets: &[DemTarget],
        incidences: &mut usize,
    ) -> AnalysisResult<()> {
        let limits = self.construction_budget.limits();
        let (detectors, observables) = folded_error_targets(targets, &limits.cancellation)?;
        if detectors.len() > MAX_EDGE_DEGREE {
            return Ok(());
        }
        let mut detectors = detectors.into_iter();
        let Some(first) = detectors.next() else {
            if self.distance_1_error_mask.is_empty() && !observables.is_empty() {
                self.distance_1_error_mask = observables;
            }
            return Ok(());
        };
        let second = detectors.next();
        let degree = 1 + usize::from(second.is_some());
        let degree_limit = limits.max_hyperedge_degree;
        if degree > degree_limit {
            return Err(ResourceLimitError::logical_error_search(
                HYPERGRAPH_SEARCH,
                LogicalErrorSearchResource::HyperedgeDegree,
                degree as u64,
                degree_limit as u64,
            )
            .into());
        }

        // An edge is new exactly when its first detector does not hold it yet, since every
        // folded edge is stored at each of its detectors.
        let first_index = self.node_index_for_detector(first)?;
        let first_node = self.nodes.get(first_index).ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::LogicalErrorSearch,
                format!(
                    "hypergraph edge detector D{} is outside the graph",
                    first.get()
                ),
            )
        })?;
        if first_node.contains(&Edge::new(second, observables.clone())) {
            return Ok(());
        }
        let projected_incidences = incidences.checked_add(degree).ok_or_else(|| {
            AnalysisError::count_overflow(
                AnalysisOperation::LogicalErrorSearch,
                "hypergraph edge incidence count overflowed",
            )
        })?;
        let incidence_limit = limits.max_hyperedge_incidences;
        if projected_incidences > incidence_limit {
            return Err(ResourceLimitError::logical_error_search(
                HYPERGRAPH_SEARCH,
                LogicalErrorSearchResource::HyperedgeIncidences,
                projected_incidences as u64,
                incidence_limit as u64,
            )
            .into());
        }

        match second {
            Some(second) => {
                self.add_outward_edge(first, Some(second), observables.clone())?;
                self.add_outward_edge(second, Some(first), observables)?;
            }
            None => self.add_outward_edge(first, None, observables)?,
        }
        *incidences = projected_incidences;
        Ok(())
    }
}
