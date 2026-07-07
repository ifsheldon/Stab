use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemInstructionKind,
    DemItem, RepeatCount,
};

use super::{
    Analyzer, DemInstruction, DemRepeatBlock, DemTarget, DetectorErrorModel, ErrorAnalyzerOptions,
    MAX_ANALYZER_REPEAT_UNROLL, RepeatBlock,
};

pub(super) struct FoldedAnalyzer {
    options: ErrorAnalyzerOptions,
}

impl FoldedAnalyzer {
    pub(super) fn new(options: ErrorAnalyzerOptions) -> Self {
        Self { options }
    }

    pub(super) fn analyze(&self, circuit: &Circuit) -> CircuitResult<DetectorErrorModel> {
        if let Some((prefix, repeat, tail)) = prefixed_single_repeat_with_tail(circuit) {
            if let Ok(Some(dem)) = self.analyze_prefixed_repeat_tail(&prefix, repeat, &tail) {
                return Ok(dem);
            }
            return self.analyze_bounded_unfolded(circuit);
        }

        if let Some((prefix, repeat)) = prefixed_single_repeat(circuit) {
            return self.analyze_prefixed_repeat(&prefix, repeat);
        }

        let mut dem = DetectorErrorModel::new();
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(_) => {
                    return self.analyze_bounded_unfolded(circuit);
                }
                CircuitItem::RepeatBlock(repeat) => {
                    dem.push_repeat_block(self.analyze_repeat(repeat)?);
                }
            }
        }
        Ok(dem)
    }

    fn analyze_prefixed_repeat_tail(
        &self,
        prefix: &[CircuitInstruction],
        repeat: &RepeatBlock,
        tail: &[CircuitInstruction],
    ) -> CircuitResult<Option<DetectorErrorModel>> {
        if repeat.repeat_count().get() <= 1 {
            return Ok(None);
        }

        let mut body_options = self.options;
        body_options.fold_loops = false;

        let prefix_circuit = instruction_circuit(prefix);
        let prefix_result = Analyzer::new(body_options).analyze_with_stats(&prefix_circuit)?;

        let first_iteration = prefixed_body_circuit(prefix, repeat.body());
        let first_result = Analyzer::new(body_options).analyze_with_stats(&first_iteration)?;
        let Some(body_detector_shift) = first_result
            .detector_count
            .checked_sub(prefix_result.detector_count)
        else {
            return Ok(None);
        };
        if body_detector_shift == 0 {
            return Ok(None);
        }
        let Some(validation_repeat_count) =
            prefixed_repeat_tail_validation_count(repeat.body(), tail)?
        else {
            return Ok(None);
        };

        let one_iteration_with_tail = prefixed_body_tail_circuit(prefix, repeat.body(), tail);
        let one_result =
            Analyzer::new(body_options).analyze_with_stats(&one_iteration_with_tail)?;
        if one_result.detector_count < first_result.detector_count {
            return Ok(None);
        }

        let Some(body_dem) = subtract_dem_item_multiset(&first_result.dem, &prefix_result.dem)
        else {
            return Ok(None);
        };
        if body_dem.is_empty() {
            return Ok(None);
        }
        let loop_carried_tail = LoopCarriedTailObservableInput {
            prefix,
            repeat_body: repeat.body(),
            repeat_count: repeat.repeat_count(),
            repeat_tag: repeat.tag(),
            tail,
            prefix_dem: &prefix_result.dem,
            body_dem: &body_dem,
            body_detector_shift,
            one_result_dem: &one_result.dem,
            body_options,
            validation_repeat_count,
        };
        if let Some(candidate) = analyze_loop_carried_tail_observable(&loop_carried_tail)? {
            return Ok(Some(candidate));
        }
        let Some(tail_absolute_dem) =
            subtract_dem_item_multiset(&one_result.dem, &first_result.dem)
        else {
            return Ok(None);
        };
        let Some(tail_dem) = rebase_dem_detector_targets(&tail_absolute_dem, body_detector_shift)?
        else {
            return Ok(None);
        };

        let candidate = compose_prefixed_repeat_tail_dem(
            &prefix_result.dem,
            &body_dem,
            body_detector_shift,
            repeat.repeat_count(),
            repeat.tag(),
            &tail_dem,
        )?;

        if !validates_prefixed_repeat_tail_candidate(PrefixedRepeatTailCandidate {
            prefix,
            body: repeat.body(),
            repeat_tag: repeat.tag(),
            tail,
            prefix_dem: &prefix_result.dem,
            body_dem: &body_dem,
            body_detector_shift,
            tail_dem: &tail_dem,
            body_options,
            validation_repeat_count,
        })? {
            return Ok(None);
        }

        Ok(Some(candidate))
    }

    fn analyze_bounded_unfolded(&self, circuit: &Circuit) -> CircuitResult<DetectorErrorModel> {
        // Unsupported folded shapes still use the normal analyzer budget before unrolling.
        let mut options = self.options;
        options.fold_loops = false;
        Analyzer::new(options).analyze(circuit)
    }

    fn analyze_prefixed_repeat(
        &self,
        prefix: &[CircuitInstruction],
        repeat: &RepeatBlock,
    ) -> CircuitResult<DetectorErrorModel> {
        let first_iteration = prefixed_body_circuit(prefix, repeat.body());
        let mut body_options = self.options;
        body_options.fold_loops = false;
        let first_result = Analyzer::new(body_options).analyze_with_stats(&first_iteration)?;
        let body_result = Analyzer::new(body_options).analyze_with_stats(repeat.body())?;
        let prefix_dem = subtract_trailing_body_dem(&first_result.dem, &body_result.dem)?;

        let mut dem = DetectorErrorModel::new();
        push_dem_items(&mut dem, prefix_dem.items());
        if repeat.repeat_count().get() > 1 {
            let mut loop_body = body_result.dem.clone();
            push_detector_shift(&mut loop_body, body_result.detector_count)?;
            dem.push_repeat_block(DemRepeatBlock::new(
                RepeatCount::try_new(repeat.repeat_count().get() - 1)?,
                loop_body,
                repeat.tag().map(ToOwned::to_owned),
            ));
        }
        push_dem_items(&mut dem, body_result.dem.items());
        Ok(dem)
    }

    fn analyze_repeat(&self, repeat: &RepeatBlock) -> CircuitResult<DemRepeatBlock> {
        if repeat_body_contains_only_repeats(repeat.body()) {
            return Ok(DemRepeatBlock::new(
                repeat.repeat_count(),
                self.analyze(repeat.body())?,
                repeat.tag().map(ToOwned::to_owned),
            ));
        }

        let mut body_options = self.options;
        body_options.fold_loops = false;
        let mut result = Analyzer::new(body_options).analyze_with_stats(repeat.body())?;
        push_detector_shift(&mut result.dem, result.detector_count)?;
        Ok(DemRepeatBlock::new(
            repeat.repeat_count(),
            result.dem,
            repeat.tag().map(ToOwned::to_owned),
        ))
    }
}

struct PrefixedRepeatTailCandidate<'a> {
    prefix: &'a [CircuitInstruction],
    body: &'a Circuit,
    repeat_tag: Option<&'a str>,
    tail: &'a [CircuitInstruction],
    prefix_dem: &'a DetectorErrorModel,
    body_dem: &'a DetectorErrorModel,
    body_detector_shift: u64,
    tail_dem: &'a DetectorErrorModel,
    body_options: ErrorAnalyzerOptions,
    validation_repeat_count: RepeatCount,
}

fn validates_prefixed_repeat_tail_candidate(
    candidate: PrefixedRepeatTailCandidate<'_>,
) -> CircuitResult<bool> {
    let validation_candidate = compose_prefixed_repeat_tail_dem(
        candidate.prefix_dem,
        candidate.body_dem,
        candidate.body_detector_shift,
        candidate.validation_repeat_count,
        candidate.repeat_tag,
        candidate.tail_dem,
    )?;
    let validation_circuit = prefixed_repeat_tail_circuit(
        candidate.prefix,
        candidate.body,
        candidate.validation_repeat_count,
        candidate.repeat_tag,
        candidate.tail,
    );
    let expected = Analyzer::new(candidate.body_options).analyze(&validation_circuit)?;
    Ok(flattened_instruction_multiset(&validation_candidate)?
        == flattened_instruction_multiset(&expected)?)
}

fn prefixed_single_repeat_with_tail(
    circuit: &Circuit,
) -> Option<(
    Vec<CircuitInstruction>,
    &RepeatBlock,
    Vec<CircuitInstruction>,
)> {
    let repeat_index = circuit
        .items()
        .iter()
        .position(|item| matches!(item, CircuitItem::RepeatBlock(_)))?;
    if circuit
        .items()
        .iter()
        .skip(repeat_index + 1)
        .any(|item| matches!(item, CircuitItem::RepeatBlock(_)))
    {
        return None;
    }
    if repeat_index == 0 || repeat_index + 1 == circuit.items().len() {
        return None;
    }

    let CircuitItem::RepeatBlock(repeat) = circuit.items().get(repeat_index)? else {
        return None;
    };
    let mut prefix = Vec::with_capacity(repeat_index);
    for item in circuit.items().get(..repeat_index)? {
        let CircuitItem::Instruction(instruction) = item else {
            return None;
        };
        prefix.push(instruction.clone());
    }
    let mut tail = Vec::with_capacity(circuit.items().len() - repeat_index - 1);
    for item in circuit.items().get(repeat_index + 1..)? {
        let CircuitItem::Instruction(instruction) = item else {
            return None;
        };
        tail.push(instruction.clone());
    }
    Some((prefix, repeat, tail))
}

fn prefixed_repeat_tail_validation_count(
    body: &Circuit,
    tail: &[CircuitInstruction],
) -> CircuitResult<Option<RepeatCount>> {
    if body
        .items()
        .iter()
        .any(|item| matches!(item, CircuitItem::RepeatBlock(_)))
    {
        return Ok(None);
    }

    let mut max_lookback = max_measurement_record_lookback_in_circuit(body)?;
    for instruction in tail {
        max_lookback =
            max_lookback.max(max_measurement_record_lookback_in_instruction(instruction)?);
    }

    let validation_count = 3_u64.max(max_lookback.checked_add(1).ok_or_else(|| {
        CircuitError::invalid_detector_error_model("measurement-record lookback overflowed")
    })?);
    if validation_count > MAX_ANALYZER_REPEAT_UNROLL {
        return Ok(None);
    }
    RepeatCount::try_new(validation_count).map(Some)
}

fn max_measurement_record_lookback_in_circuit(circuit: &Circuit) -> CircuitResult<u64> {
    let mut max_lookback = 0;
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                max_lookback =
                    max_lookback.max(max_measurement_record_lookback_in_instruction(instruction)?);
            }
            CircuitItem::RepeatBlock(_) => return Ok(max_lookback),
        }
    }
    Ok(max_lookback)
}

fn max_measurement_record_lookback_in_instruction(
    instruction: &CircuitInstruction,
) -> CircuitResult<u64> {
    let mut max_lookback = 0;
    for target in instruction.targets() {
        if let Some(offset) = target.measurement_record_offset() {
            let lookback = u64::try_from(-i64::from(offset.get())).map_err(|_| {
                CircuitError::invalid_detector_error_model("measurement-record lookback overflowed")
            })?;
            max_lookback = max_lookback.max(lookback);
        }
    }
    Ok(max_lookback)
}

fn prefixed_single_repeat(circuit: &Circuit) -> Option<(Vec<CircuitInstruction>, &RepeatBlock)> {
    let (last, prefix) = circuit.items().split_last()?;
    let CircuitItem::RepeatBlock(repeat) = last else {
        return None;
    };
    if prefix.is_empty() {
        return None;
    }
    let mut instructions = Vec::with_capacity(prefix.len());
    for item in prefix {
        let CircuitItem::Instruction(instruction) = item else {
            return None;
        };
        instructions.push(instruction.clone());
    }
    Some((instructions, repeat))
}

fn instruction_circuit(instructions: &[CircuitInstruction]) -> Circuit {
    let mut circuit = Circuit::new();
    for instruction in instructions {
        circuit.append_instruction(instruction.clone());
    }
    circuit
}

fn repeat_body_contains_only_repeats(body: &Circuit) -> bool {
    !body.items().is_empty()
        && body
            .items()
            .iter()
            .all(|item| matches!(item, CircuitItem::RepeatBlock(_)))
}

fn prefixed_body_circuit(prefix: &[CircuitInstruction], body: &Circuit) -> Circuit {
    let mut circuit = Circuit::new();
    for instruction in prefix {
        circuit.append_instruction(instruction.clone());
    }
    for item in body.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                circuit.append_instruction(instruction.clone())
            }
            CircuitItem::RepeatBlock(repeat) => circuit.append_repeat_block(repeat.clone()),
        }
    }
    circuit
}

fn prefixed_body_tail_circuit(
    prefix: &[CircuitInstruction],
    body: &Circuit,
    tail: &[CircuitInstruction],
) -> Circuit {
    let mut circuit = prefixed_body_circuit(prefix, body);
    for instruction in tail {
        circuit.append_instruction(instruction.clone());
    }
    circuit
}

fn prefixed_repeat_tail_circuit(
    prefix: &[CircuitInstruction],
    body: &Circuit,
    repeat_count: RepeatCount,
    repeat_tag: Option<&str>,
    tail: &[CircuitInstruction],
) -> Circuit {
    let mut circuit = instruction_circuit(prefix);
    circuit.append_repeat_block(RepeatBlock::new(
        repeat_count,
        body.clone(),
        repeat_tag.map(ToOwned::to_owned),
    ));
    for instruction in tail {
        circuit.append_instruction(instruction.clone());
    }
    circuit
}

struct LoopCarriedTailObservableInput<'a> {
    prefix: &'a [CircuitInstruction],
    repeat_body: &'a Circuit,
    repeat_count: RepeatCount,
    repeat_tag: Option<&'a str>,
    tail: &'a [CircuitInstruction],
    prefix_dem: &'a DetectorErrorModel,
    body_dem: &'a DetectorErrorModel,
    body_detector_shift: u64,
    one_result_dem: &'a DetectorErrorModel,
    body_options: ErrorAnalyzerOptions,
    validation_repeat_count: RepeatCount,
}

fn analyze_loop_carried_tail_observable(
    input: &LoopCarriedTailObservableInput<'_>,
) -> CircuitResult<Option<DetectorErrorModel>> {
    let Some(body_with_tail_observable) =
        subtract_dem_item_multiset(input.one_result_dem, input.prefix_dem)
    else {
        return Ok(None);
    };
    if !same_error_terms_with_added_observables(input.body_dem, &body_with_tail_observable) {
        return Ok(None);
    }

    let Some(validation_candidate) = compose_odd_period_loop_dem(
        input.prefix_dem,
        &body_with_tail_observable,
        input.body_detector_shift,
        input.validation_repeat_count,
        input.repeat_tag,
    )?
    else {
        return Ok(None);
    };
    let validation_circuit = prefixed_repeat_tail_circuit(
        input.prefix,
        input.repeat_body,
        input.validation_repeat_count,
        input.repeat_tag,
        input.tail,
    );
    let expected = Analyzer::new(input.body_options).analyze(&validation_circuit)?;
    if flattened_instruction_multiset(&validation_candidate)?
        != flattened_instruction_multiset(&expected)?
    {
        return Ok(None);
    }

    compose_odd_period_loop_dem(
        input.prefix_dem,
        &body_with_tail_observable,
        input.body_detector_shift,
        input.repeat_count,
        input.repeat_tag,
    )
}

fn subtract_trailing_body_dem(
    first_iteration: &DetectorErrorModel,
    body: &DetectorErrorModel,
) -> CircuitResult<DetectorErrorModel> {
    let first_items = first_iteration.items();
    let body_items = body.items();
    let Some(prefix_len) = first_items.len().checked_sub(body_items.len()) else {
        return Err(prefixed_repeat_unsupported_error());
    };
    let Some(trailing_items) = first_items.get(prefix_len..) else {
        return Err(prefixed_repeat_unsupported_error());
    };
    if trailing_items != body_items {
        return Err(prefixed_repeat_unsupported_error());
    }
    let Some(prefix_items) = first_items.get(..prefix_len) else {
        return Err(prefixed_repeat_unsupported_error());
    };
    let mut prefix = DetectorErrorModel::new();
    push_dem_items(&mut prefix, prefix_items);
    Ok(prefix)
}

fn subtract_dem_item_multiset(
    model: &DetectorErrorModel,
    remove: &DetectorErrorModel,
) -> Option<DetectorErrorModel> {
    let mut items = model.items().to_vec();
    for item in remove.items() {
        let index = items.iter().position(|candidate| candidate == item)?;
        items.remove(index);
    }
    let mut result = DetectorErrorModel::new();
    push_dem_items(&mut result, &items);
    Some(result)
}

fn compose_prefixed_repeat_tail_dem(
    prefix_dem: &DetectorErrorModel,
    body_dem: &DetectorErrorModel,
    body_detector_shift: u64,
    repeat_count: RepeatCount,
    repeat_tag: Option<&str>,
    tail_dem: &DetectorErrorModel,
) -> CircuitResult<DetectorErrorModel> {
    let mut dem = DetectorErrorModel::new();
    push_dem_items(&mut dem, prefix_dem.items());

    let mut loop_body = body_dem.clone();
    push_detector_shift(&mut loop_body, body_detector_shift)?;
    dem.push_repeat_block(DemRepeatBlock::new(
        repeat_count,
        loop_body,
        repeat_tag.map(ToOwned::to_owned),
    ));

    push_dem_items(&mut dem, tail_dem.items());
    Ok(dem)
}

fn rebase_dem_detector_targets(
    model: &DetectorErrorModel,
    detector_offset: u64,
) -> CircuitResult<Option<DetectorErrorModel>> {
    let mut rebased = DetectorErrorModel::new();
    for item in model.items() {
        match rebase_dem_item_detector_targets(item, detector_offset)? {
            Some(rebased_item) => push_dem_items(&mut rebased, &[rebased_item]),
            None => return Ok(None),
        }
    }
    Ok(Some(rebased))
}

fn compose_odd_period_loop_dem(
    prefix_dem: &DetectorErrorModel,
    body_dem: &DetectorErrorModel,
    body_detector_shift: u64,
    repeat_count: RepeatCount,
    repeat_tag: Option<&str>,
) -> CircuitResult<Option<DetectorErrorModel>> {
    if body_detector_shift == 0 {
        return Ok(None);
    }
    let repeat_count = repeat_count.get();
    if repeat_count < 3 || repeat_count.is_multiple_of(2) {
        return Ok(None);
    }
    if !model_contains_only_error_instructions(body_dem) {
        return Ok(None);
    }

    let mut dem = DetectorErrorModel::new();
    push_dem_items(&mut dem, prefix_dem.items());
    push_shifted_dem_items(&mut dem, body_dem.items(), 0)?;

    let middle_repeat_count = (repeat_count - 3) / 2;
    if middle_repeat_count > 0 {
        let mut loop_body = DetectorErrorModel::new();
        push_shifted_dem_items(&mut loop_body, body_dem.items(), body_detector_shift)?;
        let second_shift = body_detector_shift.checked_mul(2).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "analyze_errors folded loop detector shift overflowed",
            )
        })?;
        push_shifted_dem_items(&mut loop_body, body_dem.items(), second_shift)?;
        push_detector_shift(&mut loop_body, second_shift)?;
        dem.push_repeat_block(DemRepeatBlock::new(
            RepeatCount::try_new(middle_repeat_count)?,
            loop_body,
            repeat_tag.map(ToOwned::to_owned),
        ));
    }

    push_shifted_dem_items(&mut dem, body_dem.items(), body_detector_shift)?;
    let second_shift = body_detector_shift.checked_mul(2).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(
            "analyze_errors folded loop detector shift overflowed",
        )
    })?;
    push_shifted_dem_items(&mut dem, body_dem.items(), second_shift)?;
    Ok(Some(dem))
}

fn same_error_terms_with_added_observables(
    body: &DetectorErrorModel,
    candidate: &DetectorErrorModel,
) -> bool {
    body.items().len() == candidate.items().len()
        && body
            .items()
            .iter()
            .zip(candidate.items())
            .all(|(left, right)| same_error_term_with_added_observables(left, right))
}

fn same_error_term_with_added_observables(left: &DemItem, right: &DemItem) -> bool {
    let (DemItem::Instruction(left), DemItem::Instruction(right)) = (left, right) else {
        return false;
    };
    if left.kind() != DemInstructionKind::Error
        || right.kind() != DemInstructionKind::Error
        || left.args() != right.args()
        || left.tag() != right.tag()
    {
        return false;
    }

    let left_non_observable = left
        .targets()
        .iter()
        .filter(|target| !matches!(target, DemTarget::LogicalObservable(_)))
        .collect::<Vec<_>>();
    let right_non_observable = right
        .targets()
        .iter()
        .filter(|target| !matches!(target, DemTarget::LogicalObservable(_)))
        .collect::<Vec<_>>();
    let mut left_observables = left
        .targets()
        .iter()
        .filter_map(|target| match target {
            DemTarget::LogicalObservable(observable) => Some(observable.get()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut right_observables = right
        .targets()
        .iter()
        .filter_map(|target| match target {
            DemTarget::LogicalObservable(observable) => Some(observable.get()),
            _ => None,
        })
        .collect::<Vec<_>>();
    left_observables.sort_unstable();
    right_observables.sort_unstable();

    left_non_observable == right_non_observable
        && right_observables.len() > left_observables.len()
        && sorted_observables_are_subset(&left_observables, &right_observables)
}

fn model_contains_only_error_instructions(model: &DetectorErrorModel) -> bool {
    !model.items().is_empty()
        && model.items().iter().all(|item| {
            matches!(
                item,
                DemItem::Instruction(instruction)
                    if instruction.kind() == DemInstructionKind::Error
            )
        })
}

fn sorted_observables_are_subset(left: &[u64], right: &[u64]) -> bool {
    let mut right_iter = right.iter();
    for left_observable in left {
        loop {
            let Some(right_observable) = right_iter.next() else {
                return false;
            };
            match right_observable.cmp(left_observable) {
                std::cmp::Ordering::Less => continue,
                std::cmp::Ordering::Equal => break,
                std::cmp::Ordering::Greater => return false,
            }
        }
    }
    true
}

fn rebase_dem_item_detector_targets(
    item: &DemItem,
    detector_offset: u64,
) -> CircuitResult<Option<DemItem>> {
    match item {
        DemItem::Instruction(instruction) => {
            let Some(rebased_targets) = rebase_dem_targets(instruction.targets(), detector_offset)?
            else {
                return Ok(None);
            };
            Ok(Some(DemItem::Instruction(DemInstruction::new(
                instruction.kind(),
                instruction.args().to_vec(),
                rebased_targets,
                instruction.tag().map(ToOwned::to_owned),
            )?)))
        }
        DemItem::RepeatBlock(repeat) => {
            let Some(rebased_body) = rebase_dem_detector_targets(repeat.body(), detector_offset)?
            else {
                return Ok(None);
            };
            Ok(Some(DemItem::RepeatBlock(DemRepeatBlock::new(
                repeat.repeat_count(),
                rebased_body,
                repeat.tag().map(ToOwned::to_owned),
            ))))
        }
    }
}

fn rebase_dem_targets(
    targets: &[crate::DemTarget],
    detector_offset: u64,
) -> CircuitResult<Option<Vec<crate::DemTarget>>> {
    let mut rebased = Vec::with_capacity(targets.len());
    for target in targets {
        match target {
            crate::DemTarget::RelativeDetector(detector) => {
                let Some(detector) = detector.get().checked_sub(detector_offset) else {
                    return Ok(None);
                };
                rebased.push(crate::DemTarget::relative_detector(detector)?);
            }
            crate::DemTarget::LogicalObservable(observable) => {
                rebased.push(crate::DemTarget::logical_observable(observable.get())?);
            }
            crate::DemTarget::Separator => rebased.push(crate::DemTarget::separator()),
            crate::DemTarget::Numeric(value) => rebased.push(crate::DemTarget::numeric(*value)),
        }
    }
    Ok(Some(rebased))
}

fn shift_dem_item_detector_targets(item: &DemItem, detector_offset: u64) -> CircuitResult<DemItem> {
    match item {
        DemItem::Instruction(instruction) => {
            let shifted_targets = shift_dem_targets(instruction.targets(), detector_offset)?;
            Ok(DemItem::Instruction(DemInstruction::new(
                instruction.kind(),
                instruction.args().to_vec(),
                shifted_targets,
                instruction.tag().map(ToOwned::to_owned),
            )?))
        }
        DemItem::RepeatBlock(_) => Err(CircuitError::invalid_detector_error_model(
            "analyze_errors selected loop-carried observable fold does not support nested DEM repeats",
        )),
    }
}

fn shift_dem_targets(targets: &[DemTarget], detector_offset: u64) -> CircuitResult<Vec<DemTarget>> {
    let mut shifted = Vec::with_capacity(targets.len());
    for target in targets {
        match target {
            DemTarget::RelativeDetector(detector) => {
                let detector = detector.get().checked_add(detector_offset).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "analyze_errors folded detector target overflowed",
                    )
                })?;
                shifted.push(DemTarget::relative_detector(detector)?);
            }
            DemTarget::LogicalObservable(observable) => {
                shifted.push(DemTarget::logical_observable(observable.get())?);
            }
            DemTarget::Separator => shifted.push(DemTarget::separator()),
            DemTarget::Numeric(value) => shifted.push(DemTarget::numeric(*value)),
        }
    }
    Ok(shifted)
}

fn flattened_instruction_multiset(model: &DetectorErrorModel) -> CircuitResult<Vec<String>> {
    let mut flattened = Vec::new();
    for instruction in model.iter_flattened_instructions() {
        let mut single = DetectorErrorModel::new();
        single.push_instruction(instruction?);
        flattened.push(single.to_dem_string());
    }
    flattened.sort();
    Ok(flattened)
}

fn prefixed_repeat_unsupported_error() -> CircuitError {
    CircuitError::invalid_detector_error_model(
        "analyze_errors --fold_loops currently supports prefixed repeats only when the first iteration ends with the standalone loop-body detector error model",
    )
}

fn push_detector_shift(model: &mut DetectorErrorModel, detector_count: u64) -> CircuitResult<()> {
    if detector_count > 0 {
        model.push_instruction(DemInstruction::shift_detectors(
            Vec::new(),
            detector_count,
            None,
        )?);
    }
    Ok(())
}

fn push_dem_items(model: &mut DetectorErrorModel, items: &[DemItem]) {
    for item in items {
        match item {
            DemItem::Instruction(instruction) => model.push_instruction(instruction.clone()),
            DemItem::RepeatBlock(repeat) => model.push_repeat_block(repeat.clone()),
        }
    }
}

fn push_shifted_dem_items(
    model: &mut DetectorErrorModel,
    items: &[DemItem],
    detector_offset: u64,
) -> CircuitResult<()> {
    for item in items {
        match shift_dem_item_detector_targets(item, detector_offset)? {
            DemItem::Instruction(instruction) => model.push_instruction(instruction),
            DemItem::RepeatBlock(repeat) => model.push_repeat_block(repeat),
        }
    }
    Ok(())
}
