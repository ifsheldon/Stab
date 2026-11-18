use std::collections::{BTreeMap, BTreeSet};

use crate::{ModelError, ModelResult};

use super::api::{
    FlatRepeatScan, add_coordinate_shift_mul, add_detector_shift_mul, apply_detector_shift,
    coordinate_shift_with_repeat, detector_offset_with_repeat,
};
use super::traversal::{CoordinateShifts, FoldedDemBlock, FoldedDemItem};
use super::{DemDetectorId, DemInstructionKind, DemRepeatBlock, DemTarget};

pub(super) const MAX_DEM_SELECTED_COORDINATE_FLATTENED_DECLARATIONS: u64 = 1_000_000;

pub(super) fn flattened_detector_declaration_count_up_to(
    block: &FoldedDemBlock<'_>,
    limit: u64,
) -> ModelResult<Option<u64>> {
    Ok(block
        .summary()
        .detector_declaration_count()
        .filter(|count| *count <= limit))
}

pub(super) fn find_selected_detector_coordinates_in_bounded_flattened_repeat_body(
    repeat: &DemRepeatBlock,
    body: &FoldedDemBlock<'_>,
    detector_set: &BTreeSet<DemDetectorId>,
    coordinates: &mut BTreeMap<DemDetectorId, Vec<f64>>,
    detector_offset: u64,
    coordinate_shift: &[f64],
    geometry: RepeatScanGeometry<'_>,
) -> ModelResult<()> {
    let mut scan = FlatRepeatScan {
        detector_set,
        existing_coordinates: coordinates,
        best: BTreeMap::new(),
        outer_detector_offset: detector_offset,
        outer_coordinate_shift: coordinate_shift,
        body_detector_shift: geometry.body_detector_shift,
        body_coordinate_shift: geometry.body_coordinate_shift,
        repeat_count: repeat.repeat_count().get(),
    };
    record_flattened_detector_declarations(body, &mut scan, &mut CoordinateShifts::default())?;
    for (detector, (_, detector_coordinates)) in scan.best {
        coordinates.insert(detector, detector_coordinates);
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct RepeatScanGeometry<'a> {
    pub(super) body_detector_shift: u64,
    pub(super) body_coordinate_shift: &'a [f64],
}

/// One block of a flattened declaration scan: the scanned body, or one iteration of a repeat
/// inside it.
struct ScanFrame<'b, 'a> {
    items: std::slice::Iter<'b, FoldedDemItem<'a>>,
    detector_offset: u64,
    coordinate_shift: Vec<f64>,
    /// The repeat in this block whose iterations are being scanned above this frame.
    open: Option<ScanRepeat<'b, 'a>>,
}

struct ScanRepeat<'b, 'a> {
    repeat: &'a DemRepeatBlock,
    body: &'b FoldedDemBlock<'a>,
    body_detector_shift: u64,
    body_coordinate_shift: Vec<f64>,
    next_iteration: u64,
}

/// Records every detector declaration of `block`, repeat iterations flattened, in body order.
///
/// Each repeat iteration is a stack frame instead of a recursive call, with the same checked
/// arithmetic in the same order, so the scan uses constant native stack at any nesting depth.
fn record_flattened_detector_declarations(
    block: &FoldedDemBlock<'_>,
    scan: &mut FlatRepeatScan<'_>,
    shifts: &mut CoordinateShifts,
) -> ModelResult<()> {
    let mut body_order = 0_usize;
    let mut frames = vec![ScanFrame {
        items: block.items().iter(),
        detector_offset: 0,
        coordinate_shift: Vec::new(),
        open: None,
    }];
    loop {
        let frame = frames.last_mut().ok_or_else(lost_scan_frame)?;
        match frame.items.next() {
            Some(FoldedDemItem::Instruction(instruction)) => match instruction.kind() {
                DemInstructionKind::Detector => {
                    for target in instruction.targets() {
                        if let DemTarget::RelativeDetector(detector) = target {
                            let local_detector = frame
                                .detector_offset
                                .checked_add(detector.get())
                                .ok_or_else(|| {
                                    ModelError::invalid_detector_error_model(
                                        "flattened detector declaration id overflowed",
                                    )
                                })?;
                            scan.record_declaration_with_shift(
                                local_detector,
                                instruction.args(),
                                &frame.coordinate_shift,
                                body_order,
                            )?;
                            body_order = body_order.checked_add(1).ok_or_else(|| {
                                ModelError::invalid_detector_error_model(
                                    "flattened detector declaration order overflowed",
                                )
                            })?;
                        }
                    }
                }
                DemInstructionKind::ShiftDetectors => {
                    apply_detector_shift(&mut frame.detector_offset, instruction)?;
                    add_coordinate_shift_mul(&mut frame.coordinate_shift, instruction.args(), 1.0)?;
                }
                DemInstructionKind::Error | DemInstructionKind::LogicalObservable => {}
            },
            Some(FoldedDemItem::Repeat { repeat, body }) => {
                let body_count = flattened_detector_declaration_count_up_to(body, 1)?.unwrap_or(2);
                if body_count == 0 {
                    add_detector_shift_mul(
                        &mut frame.detector_offset,
                        body.summary().detector_shift()?,
                        repeat.repeat_count().get(),
                    )?;
                    add_coordinate_shift_mul(
                        &mut frame.coordinate_shift,
                        shifts.of(body)?,
                        repeat.repeat_count().get() as f64,
                    )?;
                    continue;
                }
                let body_detector_shift = body.summary().detector_shift()?;
                let body_coordinate_shift = shifts.of(body)?.to_vec();
                frame.open = Some(ScanRepeat {
                    repeat,
                    body,
                    body_detector_shift,
                    body_coordinate_shift,
                    next_iteration: 0,
                });
                step_scan_repeat(&mut frames)?;
            }
            None => {
                if frames.len() == 1 {
                    return Ok(());
                }
                frames.pop();
                step_scan_repeat(&mut frames)?;
            }
        }
    }
}

/// Starts the next iteration of the top frame's open repeat, or, when none remains, closes the
/// repeat and advances the top frame past it.
fn step_scan_repeat(frames: &mut Vec<ScanFrame<'_, '_>>) -> ModelResult<()> {
    let parent = frames.last_mut().ok_or_else(lost_scan_frame)?;
    let open = parent.open.as_mut().ok_or_else(|| {
        ModelError::invalid_detector_error_model("flattened detector scan lost its open repeat")
    })?;
    let repeat_count = open.repeat.repeat_count().get();
    if open.next_iteration < repeat_count {
        let iteration = open.next_iteration;
        open.next_iteration += 1;
        let detector_offset = detector_offset_with_repeat(
            parent.detector_offset,
            open.body_detector_shift,
            iteration,
        )?;
        let coordinate_shift = coordinate_shift_with_repeat(
            &parent.coordinate_shift,
            &open.body_coordinate_shift,
            iteration,
        )?;
        let items = open.body.items().iter();
        frames.push(ScanFrame {
            items,
            detector_offset,
            coordinate_shift,
            open: None,
        });
        return Ok(());
    }
    let open = parent.open.take().ok_or_else(|| {
        ModelError::invalid_detector_error_model("flattened detector scan lost its open repeat")
    })?;
    add_detector_shift_mul(
        &mut parent.detector_offset,
        open.body_detector_shift,
        repeat_count,
    )?;
    add_coordinate_shift_mul(
        &mut parent.coordinate_shift,
        &open.body_coordinate_shift,
        repeat_count as f64,
    )
}

fn lost_scan_frame() -> ModelError {
    ModelError::invalid_detector_error_model("flattened detector scan stack became empty")
}
