#![allow(
    clippy::expect_used,
    reason = "mechanism traversal tests use compact exact DEM fixtures"
)]

use std::convert::Infallible;
use std::ops::ControlFlow;

use stab_model::{
    DemDetectorId, DemErrorMechanismTraversalLimits, DemErrorMechanismView,
    DemErrorMechanismVisitError, DemErrorMechanismVisitor, DemErrorTarget, DemObservableId,
    DetectorErrorModel,
};

#[derive(Default)]
struct RecordingVisitor {
    mechanisms: Vec<(f64, Vec<DemErrorTarget>)>,
    tags: Vec<Option<Vec<u8>>>,
    stop_after: Option<usize>,
}

#[derive(Default)]
struct CountingVisitor {
    mechanisms: usize,
    targets: usize,
}

impl DemErrorMechanismVisitor for CountingVisitor {
    type Error = Infallible;

    fn visit_error_mechanism(
        &mut self,
        mechanism: DemErrorMechanismView<'_>,
    ) -> Result<ControlFlow<()>, Self::Error> {
        self.mechanisms += 1;
        for target in mechanism.targets() {
            std::hint::black_box(target.expect("valid absolute target"));
            self.targets += 1;
        }
        Ok(ControlFlow::Continue(()))
    }
}

impl DemErrorMechanismVisitor for RecordingVisitor {
    type Error = Infallible;

    fn visit_error_mechanism(
        &mut self,
        mechanism: DemErrorMechanismView<'_>,
    ) -> Result<ControlFlow<()>, Self::Error> {
        let targets = mechanism
            .targets()
            .collect::<Result<Vec<_>, _>>()
            .expect("valid absolute targets");
        self.mechanisms
            .push((mechanism.probability().get(), targets));
        self.tags.push(mechanism.tag_bytes().map(<[u8]>::to_vec));
        if self
            .stop_after
            .is_some_and(|limit| self.mechanisms.len() == limit)
        {
            return Ok(ControlFlow::Break(()));
        }
        Ok(ControlFlow::Continue(()))
    }
}

fn shifted_repeated_model() -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(
        "shift_detectors 2\n\
         repeat 2 {\n\
             error[loop](0.25) D0 D0 ^ D1 L0\n\
             detector D1\n\
             shift_detectors 3\n\
         }\n\
         repeat 0 {\n\
             error(0.125) D9\n\
         }\n\
         error[tail](0.5) D0\n",
    )
    .expect("shifted repeated DEM")
}

fn detector(id: u64) -> DemErrorTarget {
    DemErrorTarget::Detector(DemDetectorId::try_new(id).expect("detector id"))
}

fn observable(id: u64) -> DemErrorTarget {
    DemErrorTarget::Observable(DemObservableId::try_new(id).expect("observable id"))
}

#[test]
fn mechanism_traversal_preserves_absolute_targets_duplicates_and_separators() {
    let model = shifted_repeated_model();
    let limits = DemErrorMechanismTraversalLimits::new(3, 9);
    let mut visitor = RecordingVisitor::default();

    assert_eq!(
        model
            .try_visit_error_mechanisms(limits, &mut visitor)
            .expect("bounded mechanism traversal"),
        ControlFlow::Continue(())
    );
    assert_eq!(
        visitor.mechanisms,
        vec![
            (
                0.25,
                vec![
                    detector(2),
                    detector(2),
                    DemErrorTarget::Separator,
                    detector(3),
                    observable(0),
                ],
            ),
            (
                0.25,
                vec![
                    detector(5),
                    detector(5),
                    DemErrorTarget::Separator,
                    detector(6),
                    observable(0),
                ],
            ),
            (0.5, vec![detector(8)]),
        ]
    );
    assert_eq!(
        visitor.tags,
        vec![
            Some(b"loop".to_vec()),
            Some(b"loop".to_vec()),
            Some(b"tail".to_vec()),
        ]
    );
}

#[test]
fn mechanism_count_is_admitted_before_callbacks_and_visitor_stop_is_immediate() {
    let model = shifted_repeated_model();
    let mut rejected = RecordingVisitor::default();
    let error = model
        .try_visit_error_mechanisms(DemErrorMechanismTraversalLimits::new(2, 100), &mut rejected)
        .expect_err("three represented mechanisms exceed two");
    assert!(matches!(
        error,
        DemErrorMechanismVisitError::MechanismLimit {
            actual: 3,
            limit: 2
        }
    ));
    assert!(rejected.mechanisms.is_empty());

    let mut stopped = RecordingVisitor {
        stop_after: Some(2),
        ..RecordingVisitor::default()
    };
    assert_eq!(
        model
            .try_visit_error_mechanisms(
                DemErrorMechanismTraversalLimits::new(3, 100),
                &mut stopped,
            )
            .expect("early visitor stop"),
        ControlFlow::Break(())
    );
    assert_eq!(stopped.mechanisms.len(), 2);
}

#[test]
fn represented_instruction_work_is_bounded_independently_of_mechanism_count() {
    let model = shifted_repeated_model();
    let mut visitor = RecordingVisitor::default();
    let error = model
        .try_visit_error_mechanisms(DemErrorMechanismTraversalLimits::new(3, 7), &mut visitor)
        .expect_err("eighth represented instruction exceeds work limit");
    assert!(matches!(
        error,
        DemErrorMechanismVisitError::InstructionVisitLimit {
            actual_at_least: 8,
            limit: 7
        }
    ));
}

#[test]
fn nested_error_free_repeats_are_skipped_without_spending_represented_work() {
    let model = DetectorErrorModel::from_dem_str(
        "repeat 1000000 {\n\
             repeat 1000000 {\n\
                 detector D0\n\
                 shift_detectors 1\n\
             }\n\
         }\n",
    )
    .expect("error-free nested DEM");
    let mut visitor = RecordingVisitor::default();

    assert_eq!(
        model
            .try_visit_error_mechanisms(DemErrorMechanismTraversalLimits::new(0, 0), &mut visitor,)
            .expect("skip error-free repeats"),
        ControlFlow::Continue(())
    );
    assert!(visitor.mechanisms.is_empty());
}

#[test]
fn nested_error_bearing_repeats_apply_each_detector_shift_once() {
    let model = DetectorErrorModel::from_dem_str(
        "repeat 2 {\n\
             error(0.1) D0\n\
             repeat 2 {\n\
                 error(0.2) D1\n\
                 shift_detectors 2\n\
             }\n\
             shift_detectors 5\n\
         }\n",
    )
    .expect("nested shifted DEM");
    let mut visitor = RecordingVisitor::default();

    assert_eq!(
        model
            .try_visit_error_mechanisms(DemErrorMechanismTraversalLimits::new(6, 12), &mut visitor,)
            .expect("nested mechanism traversal"),
        ControlFlow::Continue(())
    );
    assert_eq!(
        visitor.mechanisms,
        vec![
            (0.1, vec![detector(0)]),
            (0.2, vec![detector(1)]),
            (0.2, vec![detector(3)]),
            (0.1, vec![detector(9)]),
            (0.2, vec![detector(10)]),
            (0.2, vec![detector(12)]),
        ]
    );
}

#[test]
fn represented_repeat_count_does_not_allocate_per_mechanism_or_target() {
    fn measure(repetitions: u64) -> (allocation_counter::AllocationInfo, CountingVisitor) {
        let model = DetectorErrorModel::from_dem_str(&format!(
            "repeat {repetitions} {{\nerror(0.25) D0 D1 L0\n}}\n"
        ))
        .expect("compact repeated DEM");
        let mut visitor = CountingVisitor::default();
        let allocations = allocation_counter::measure(|| {
            assert_eq!(
                model
                    .try_visit_error_mechanisms(
                        DemErrorMechanismTraversalLimits::new(repetitions, repetitions),
                        &mut visitor,
                    )
                    .expect("count repeated mechanisms"),
                ControlFlow::Continue(())
            );
        });
        (allocations, visitor)
    }

    let (one, one_visitor) = measure(1);
    let (many, many_visitor) = measure(1_024);
    assert_eq!(one_visitor.mechanisms, 1);
    assert_eq!(many_visitor.mechanisms, 1_024);
    assert_eq!(one_visitor.targets, 3);
    assert_eq!(many_visitor.targets, 3_072);
    assert_eq!(
        many.count_total, one.count_total,
        "one={one:?} many={many:?}"
    );
    assert_eq!(
        many.bytes_total, one.bytes_total,
        "one={one:?} many={many:?}"
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VisitorFailure;

impl std::fmt::Display for VisitorFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("intentional visitor failure")
    }
}

impl std::error::Error for VisitorFailure {}

struct FailingVisitor;

impl DemErrorMechanismVisitor for FailingVisitor {
    type Error = VisitorFailure;

    fn visit_error_mechanism(
        &mut self,
        _mechanism: DemErrorMechanismView<'_>,
    ) -> Result<ControlFlow<()>, Self::Error> {
        Err(VisitorFailure)
    }
}

#[test]
fn traversal_distinguishes_model_failures_from_visitor_failures() {
    let model = DetectorErrorModel::from_dem_str("error(0.25) D0\n").expect("valid DEM");
    let visitor_error = model
        .try_visit_error_mechanisms(
            DemErrorMechanismTraversalLimits::new(1, 1),
            &mut FailingVisitor,
        )
        .expect_err("visitor error");
    assert_eq!(
        visitor_error,
        DemErrorMechanismVisitError::Visitor(VisitorFailure)
    );
    assert!(
        visitor_error
            .to_string()
            .contains("intentional visitor failure")
    );

    let overflowing = DetectorErrorModel::from_dem_str(
        "repeat 1152921504606846975 {\n\
             shift_detectors 1152921504606846975\n\
         }\n\
         error(0.25) D0\n",
    )
    .expect("individually valid compact repeat");
    let model_error = overflowing
        .try_visit_error_mechanisms(
            DemErrorMechanismTraversalLimits::new(1, 1),
            &mut RecordingVisitor::default(),
        )
        .expect_err("aggregate detector offset overflow");
    assert!(matches!(model_error, DemErrorMechanismVisitError::Model(_)));
}
