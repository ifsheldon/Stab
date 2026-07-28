#![allow(
    clippy::expect_used,
    reason = "folded traversal contract tests use direct fixture assertions"
)]

use std::ops::ControlFlow;

use stab_model::advanced::{
    DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem, FoldedDemTraversal,
    FoldedDemVisitor, MAX_DEM_REPEAT_NESTING, shifted_coordinates, shifted_detector,
    shifted_targets,
};
use stab_model::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemRepeatBlock, DemTarget,
    DetectorErrorModel, ModelError, RepeatNestingLimit, ValidationError,
};

#[derive(Debug)]
struct RecordingVisitor {
    selection: DemRepeatSelection,
    errors: Vec<(u64, Vec<f64>, usize, u64)>,
    stop_after_errors: Option<usize>,
}

impl RecordingVisitor {
    fn new(selection: DemRepeatSelection) -> Self {
        Self {
            selection,
            errors: Vec::new(),
            stop_after_errors: None,
        }
    }
}

impl FoldedDemVisitor for RecordingVisitor {
    type Error = ModelError;

    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> Result<ControlFlow<()>, Self::Error> {
        if instruction.kind() == DemInstructionKind::Error {
            self.errors.push((
                state.detector_offset(),
                state.coordinate_shift()?.to_vec(),
                state.folded_repeat_depth(),
                state.folded_repeat_multiplicity(),
            ));
            if self
                .stop_after_errors
                .is_some_and(|limit| self.errors.len() == limit)
            {
                return Ok(ControlFlow::Break(()));
            }
        }
        Ok(ControlFlow::Continue(()))
    }

    fn enter_repeat(
        &mut self,
        _repeat: &DemRepeatBlock,
        _body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> Result<DemRepeatSelection, Self::Error> {
        Ok(self.selection.clone())
    }
}

fn assert_compact_structure_and_semantic_state() {
    let model = DetectorErrorModel::from_dem_str(
        "shift_detectors(1, 2) 2\n\
         repeat 3 {\n\
             error(0.25) D0 L1\n\
             shift_detectors(0.5, 1) 3\n\
         }\n\
         error(0.5) D0\n",
    )
    .expect("valid folded DEM");
    let traversal = FoldedDemTraversal::new(&model).expect("folded traversal");
    let root = traversal.root();

    assert_eq!(root.compact_id(), 0);
    assert_eq!(root.summary().detector_shift().expect("detector shift"), 11);
    assert_eq!(root.summary().detector_count().expect("detector count"), 12);
    assert_eq!(root.summary().observable_count(), 2);
    assert_eq!(root.summary().error_count().expect("error count"), 4);
    assert_eq!(root.summary().max_repeat_depth(), 1);
    let repeat_body = root
        .items()
        .iter()
        .find_map(|item| match item {
            FoldedDemItem::Repeat { body, .. } => Some(body.as_ref()),
            FoldedDemItem::Instruction(_) => None,
        })
        .expect("compact repeat body");
    assert_eq!(repeat_body.compact_id(), 1);
    assert_eq!(repeat_body.items().len(), 2);

    let mut visitor = RecordingVisitor::new(DemRepeatSelection::Expand {
        max_total_iterations: 3,
        context: "model contract",
    });
    assert_eq!(
        traversal
            .try_visit_with_coordinates(&mut visitor)
            .expect("expanded traversal"),
        ControlFlow::Continue(())
    );
    assert_eq!(
        visitor.errors,
        vec![
            (2, vec![1.0, 2.0], 0, 1),
            (5, vec![1.5, 3.0], 0, 1),
            (8, vec![2.0, 4.0], 0, 1),
            (11, vec![2.5, 5.0], 0, 1),
        ]
    );

    let detector = shifted_detector(DemDetectorId::try_new(7).expect("detector id"), 5)
        .expect("shifted detector");
    assert_eq!(detector.get(), 12);
    assert_eq!(
        shifted_targets(
            &[
                DemTarget::relative_detector(2).expect("detector target"),
                DemTarget::logical_observable(3).expect("observable target"),
                DemTarget::separator(),
            ],
            10,
        )
        .expect("shifted targets"),
        vec![
            DemTarget::relative_detector(12).expect("shifted detector target"),
            DemTarget::logical_observable(3).expect("preserved observable target"),
            DemTarget::separator(),
        ]
    );
    assert_eq!(
        shifted_coordinates(&[1.0, 2.0, 3.0], &[0.5, -1.0]).expect("shifted coordinates"),
        vec![1.5, 1.0, 3.0]
    );
}

fn assert_repeat_policies_and_cancellation() {
    let model = DetectorErrorModel::from_dem_str(
        "repeat 4 {\n\
             error(0.25) D0\n\
         }\n\
         error(0.5) D1\n",
    )
    .expect("valid zero-shift repeat DEM");
    let traversal = FoldedDemTraversal::new(&model).expect("folded traversal");

    let mut skipped = RecordingVisitor::new(DemRepeatSelection::Skip);
    assert_eq!(
        traversal
            .try_visit_with_coordinates(&mut skipped)
            .expect("skipped traversal"),
        ControlFlow::Continue(())
    );
    assert_eq!(skipped.errors, vec![(0, Vec::new(), 0, 1)]);

    let mut structural = RecordingVisitor::new(DemRepeatSelection::StructuralOnce);
    assert_eq!(
        traversal
            .try_visit_with_coordinates(&mut structural)
            .expect("structural traversal"),
        ControlFlow::Continue(())
    );
    assert_eq!(
        structural.errors,
        vec![(0, Vec::new(), 0, 1), (0, Vec::new(), 0, 1)]
    );

    let mut folded = RecordingVisitor::new(DemRepeatSelection::FoldOnce);
    assert_eq!(
        traversal
            .try_visit_with_coordinates(&mut folded)
            .expect("folded traversal"),
        ControlFlow::Continue(())
    );
    assert_eq!(
        folded.errors,
        vec![(0, Vec::new(), 1, 4), (0, Vec::new(), 0, 1)]
    );

    let mut selected = RecordingVisitor::new(DemRepeatSelection::Selected(vec![1, 3]));
    assert_eq!(
        traversal
            .try_visit_with_coordinates(&mut selected)
            .expect("selected traversal"),
        ControlFlow::Continue(())
    );
    assert_eq!(
        selected.errors,
        vec![
            (0, Vec::new(), 0, 1),
            (0, Vec::new(), 0, 1),
            (0, Vec::new(), 0, 1),
        ]
    );

    let mut cancelled = RecordingVisitor::new(DemRepeatSelection::Expand {
        max_total_iterations: 4,
        context: "cancellation contract",
    });
    cancelled.stop_after_errors = Some(2);
    assert_eq!(
        traversal
            .try_visit_with_coordinates(&mut cancelled)
            .expect("cancelled traversal"),
        ControlFlow::Break(())
    );
    assert_eq!(cancelled.errors.len(), 2);

    let mut invalid = RecordingVisitor::new(DemRepeatSelection::Selected(vec![2, 2]));
    let error = traversal
        .try_visit_with_coordinates(&mut invalid)
        .expect_err("duplicate selected iteration");
    assert!(
        error
            .to_string()
            .contains("strictly increasing and in range")
    );
}

#[test]
fn folded_dem_traversal_contract_is_compact_semantic_and_cancellable() {
    assert_compact_structure_and_semantic_state();
    assert_repeat_policies_and_cancellation();
}

#[derive(Debug)]
struct ExpansionLimitVisitor {
    limit: u64,
    error_calls: usize,
}

impl FoldedDemVisitor for ExpansionLimitVisitor {
    type Error = ModelError;

    fn visit_instruction(
        &mut self,
        _instruction: &DemInstruction,
        _state: &DemTraversalState,
    ) -> Result<ControlFlow<()>, Self::Error> {
        Ok(ControlFlow::Continue(()))
    }

    fn enter_repeat(
        &mut self,
        _repeat: &DemRepeatBlock,
        _body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> Result<DemRepeatSelection, Self::Error> {
        Ok(DemRepeatSelection::Expand {
            max_total_iterations: self.limit,
            context: "custom expansion",
        })
    }

    fn repeat_expansion_limit_error(
        &mut self,
        context: &'static str,
        actual: u64,
        limit: u64,
    ) -> Self::Error {
        self.error_calls += 1;
        ValidationError::InvalidDetectorErrorModel {
            message: format!("{context}: custom limit {limit}, actual {actual}"),
        }
        .into()
    }
}

#[test]
fn folded_dem_traversal_enforces_exact_expansion_limit_before_callbacks() {
    assert_eq!(
        MAX_DEM_REPEAT_NESTING,
        RepeatNestingLimit::HARD_MAX,
        "the advanced traversal limit must stay aligned with the model parser limit"
    );
    let model = DetectorErrorModel::from_dem_str(
        "repeat 3 {\n\
             error(0.25) D0\n\
         }\n",
    )
    .expect("valid repeated DEM");
    let traversal = FoldedDemTraversal::new(&model).expect("folded traversal");

    let mut exact = ExpansionLimitVisitor {
        limit: 3,
        error_calls: 0,
    };
    assert_eq!(
        traversal.try_visit(&mut exact).expect("exact expansion"),
        ControlFlow::Continue(())
    );
    assert_eq!(exact.error_calls, 0);

    let mut excess = ExpansionLimitVisitor {
        limit: 2,
        error_calls: 0,
    };
    let error = traversal
        .try_visit(&mut excess)
        .expect_err("first expansion excess");
    assert_eq!(excess.error_calls, 1);
    assert_eq!(
        error.to_string(),
        "invalid detector error model: custom expansion: custom limit 2, actual 3"
    );
}
