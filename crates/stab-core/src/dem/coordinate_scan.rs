use std::collections::{BTreeMap, BTreeSet};

use crate::{CircuitError, CircuitResult};

use super::api::{
    FlatRepeatScan, add_coordinate_shift_mul, add_detector_shift_mul, apply_detector_shift,
    coordinate_shift_with_repeat, detector_offset_with_repeat,
};
use super::traversal::{FoldedDemBlock, FoldedDemItem};
use super::{DemDetectorId, DemInstructionKind, DemRepeatBlock, DemTarget};

pub(super) const MAX_DEM_SELECTED_COORDINATE_FLATTENED_DECLARATIONS: u64 = 1_000_000;

pub(super) fn flattened_detector_declaration_count_up_to(
    block: &FoldedDemBlock<'_>,
    limit: u64,
) -> CircuitResult<Option<u64>> {
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
) -> CircuitResult<()> {
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
    let mut local_detector_offset = 0_u64;
    let mut local_coordinate_shift = Vec::new();
    let mut body_order = 0_usize;
    record_flattened_detector_declarations(
        body,
        &mut local_detector_offset,
        &mut local_coordinate_shift,
        &mut body_order,
        &mut scan,
    )?;
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

fn record_flattened_detector_declarations(
    block: &FoldedDemBlock<'_>,
    detector_offset: &mut u64,
    coordinate_shift: &mut Vec<f64>,
    body_order: &mut usize,
    scan: &mut FlatRepeatScan<'_>,
) -> CircuitResult<()> {
    for item in block.items() {
        match item {
            FoldedDemItem::Instruction(instruction) => match instruction.kind() {
                DemInstructionKind::Detector => {
                    for target in instruction.targets() {
                        if let DemTarget::RelativeDetector(detector) = target {
                            let local_detector =
                                detector_offset.checked_add(detector.get()).ok_or_else(|| {
                                    CircuitError::invalid_detector_error_model(
                                        "flattened detector declaration id overflowed",
                                    )
                                })?;
                            scan.record_declaration_with_shift(
                                local_detector,
                                instruction.args(),
                                coordinate_shift,
                                *body_order,
                            )?;
                            *body_order = body_order.checked_add(1).ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(
                                    "flattened detector declaration order overflowed",
                                )
                            })?;
                        }
                    }
                }
                DemInstructionKind::ShiftDetectors => {
                    apply_detector_shift(detector_offset, instruction)?;
                    add_coordinate_shift_mul(coordinate_shift, instruction.args(), 1.0)?;
                }
                DemInstructionKind::Error | DemInstructionKind::LogicalObservable => {}
            },
            FoldedDemItem::Repeat { repeat, body } => {
                let body_count = flattened_detector_declaration_count_up_to(body, 1)?.unwrap_or(2);
                if body_count == 0 {
                    add_detector_shift_mul(
                        detector_offset,
                        body.summary().detector_shift()?,
                        repeat.repeat_count().get(),
                    )?;
                    add_coordinate_shift_mul(
                        coordinate_shift,
                        &body.coordinate_shift()?,
                        repeat.repeat_count().get() as f64,
                    )?;
                    continue;
                }
                let body_detector_shift = body.summary().detector_shift()?;
                let body_coordinate_shift = body.coordinate_shift()?;
                for iteration in 0..repeat.repeat_count().get() {
                    let mut iteration_detector_offset = detector_offset_with_repeat(
                        *detector_offset,
                        body_detector_shift,
                        iteration,
                    )?;
                    let mut iteration_coordinate_shift = coordinate_shift_with_repeat(
                        coordinate_shift,
                        &body_coordinate_shift,
                        iteration,
                    )?;
                    record_flattened_detector_declarations(
                        body,
                        &mut iteration_detector_offset,
                        &mut iteration_coordinate_shift,
                        body_order,
                        scan,
                    )?;
                }
                add_detector_shift_mul(
                    detector_offset,
                    body_detector_shift,
                    repeat.repeat_count().get(),
                )?;
                add_coordinate_shift_mul(
                    coordinate_shift,
                    &body_coordinate_shift,
                    repeat.repeat_count().get() as f64,
                )?;
            }
        }
    }
    Ok(())
}
