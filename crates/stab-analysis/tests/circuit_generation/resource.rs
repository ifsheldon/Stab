use stab_analysis::{
    CodeDistance, ColorCodeParams, ColorCodeTask, GeneratedCircuit, RepetitionCodeParams,
    RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
};
use stab_model::{CircuitItem, MemoryBudget};

/// A 256 MiB budget at 2048 projected bytes per physical qubit admits exactly the 131,072
/// physical qubits that the retired fixed generator cap admitted.
const BUDGET: MemoryBudget = MemoryBudget::from_bytes(256 * 1024 * 1024);
const BUDGET_QUBITS: u64 = 131_072;
const BYTES_PER_QUBIT: u64 = 2048;

/// Admission projects each family's physical qubit count from the distance and refuses a request
/// whose projected size exceeds the memory budget before materializing anything. The projection
/// is checked against materialized small circuits, the boundary distances sit under the budget
/// while the next distance is refused with the projected count, the tightest boundary (rotated
/// surface code, 131,071 qubits) is materialized once to prove the accepted side end to end, and
/// the per-qubit projection is checked to dominate the generator's peak allocation.
#[test]
fn cq2_generation_resource_admission_checks_family_boundaries_before_materialization() {
    assert_repetition_generation_boundary();
    assert_surface_generation_boundary(SurfaceCodeTask::RotatedMemoryZ, 256);
    assert_surface_generation_boundary(SurfaceCodeTask::UnrotatedMemoryZ, 181);
    assert_color_generation_boundary();
    assert_rotated_surface_boundary_materializes();
    assert_projection_dominates_peak_allocation();
    assert_huge_round_counts_stay_folded();
}

fn assert_repetition_generation_boundary() {
    let accepted = RepetitionCodeParams::new(
        RoundCount::try_new(1).expect("rounds"),
        CodeDistance::try_new(2047).expect("maximum CLI distance"),
        RepetitionCodeTask::Memory,
    )
    .expect("repetition parameters");
    let generated = generate_repetition_code_circuit(&accepted)
        .expect("maximum repetition distance remains admissible");
    assert_eq!(generated.circuit().count_qubits(), 4093);
    super::assert_invalid_domain_value(CodeDistance::try_new(2048), "code distance", "2048");
}

fn surface_params(task: SurfaceCodeTask, distance: u32) -> SurfaceCodeParams {
    SurfaceCodeParams::new(
        RoundCount::try_new(1).expect("rounds"),
        CodeDistance::try_new(distance).expect("domain-valid distance"),
        task,
    )
    .expect("surface parameters")
    .with_memory_budget(BUDGET)
}

fn budget_refusal(qubits: u64, family: &str) -> String {
    format!(
        "{qubits} physical qubits for {family} need at least {} bytes, exceeding the {}-byte memory budget; raise the memory budget to admit it",
        qubits * BYTES_PER_QUBIT,
        BUDGET.bytes()
    )
}

fn surface_physical_qubits(task: SurfaceCodeTask, distance: u32) -> u64 {
    let distance = u64::from(distance);
    if matches!(
        task,
        SurfaceCodeTask::RotatedMemoryX | SurfaceCodeTask::RotatedMemoryZ
    ) {
        distance * distance * 2 - 1
    } else {
        (distance * 2 - 1) * (distance * 2 - 1)
    }
}

fn assert_surface_generation_boundary(task: SurfaceCodeTask, accepted_distance: u32) {
    for distance in 2..=9 {
        let generated = generate_surface_code_circuit(&surface_params(task, distance))
            .expect("small surface code");
        assert_eq!(
            qubit_coordinate_target_count(&generated),
            surface_physical_qubits(task, distance),
            "{task:?} d={distance} projected and materialized qubit counts diverged"
        );
    }
    assert!(surface_physical_qubits(task, accepted_distance) <= BUDGET_QUBITS);

    let rejected_distance = accepted_distance + 1;
    let family = if matches!(
        task,
        SurfaceCodeTask::RotatedMemoryX | SurfaceCodeTask::RotatedMemoryZ
    ) {
        "rotated surface code"
    } else {
        "unrotated surface code"
    };
    super::assert_invalid_domain_value(
        generate_surface_code_circuit(&surface_params(task, rejected_distance)),
        "generated circuit size",
        &budget_refusal(surface_physical_qubits(task, rejected_distance), family),
    );
}

fn assert_rotated_surface_boundary_materializes() {
    let generated =
        generate_surface_code_circuit(&surface_params(SurfaceCodeTask::RotatedMemoryZ, 256))
            .expect("rotated distance 256 is the largest admissible rotated surface code");
    assert!(!generated.layout_text().is_empty());
    assert_eq!(qubit_coordinate_target_count(&generated), 131_071);
}

fn color_params(distance: u32) -> ColorCodeParams {
    ColorCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(distance).expect("domain-valid distance"),
        ColorCodeTask::MemoryXyz,
    )
    .expect("color parameters")
    .with_memory_budget(BUDGET)
}

fn color_physical_qubits(distance: u32) -> u64 {
    let distance = u64::from(distance);
    let width = distance + (distance - 1) / 2;
    width * (width + 1) / 2
}

fn assert_color_generation_boundary() {
    for distance in [3, 5, 7, 9] {
        let generated = generate_color_code_circuit(&color_params(distance)).expect("small color");
        assert_eq!(
            qubit_coordinate_target_count(&generated),
            color_physical_qubits(distance),
            "color d={distance} projected and materialized qubit counts diverged"
        );
    }
    assert_eq!(color_physical_qubits(341), 130_816);
    super::assert_invalid_domain_value(
        generate_color_code_circuit(&color_params(343)),
        "generated circuit size",
        &budget_refusal(132_355, "color code"),
    );
}

fn assert_projection_dominates_peak_allocation() {
    for (task, distance) in [
        (SurfaceCodeTask::RotatedMemoryZ, 33),
        (SurfaceCodeTask::UnrotatedMemoryZ, 17),
    ] {
        let params = surface_params(task, distance);
        let measured = allocation_counter::measure(|| {
            std::hint::black_box(generate_surface_code_circuit(&params).expect("generate"));
        });
        let projected = surface_physical_qubits(task, distance) * BYTES_PER_QUBIT;
        assert!(
            measured.bytes_max <= projected,
            "{task:?} d={distance}: peak {} exceeds projection {projected}",
            measured.bytes_max
        );
    }
    let params = color_params(21);
    let measured = allocation_counter::measure(|| {
        std::hint::black_box(generate_color_code_circuit(&params).expect("generate"));
    });
    let projected = color_physical_qubits(21) * BYTES_PER_QUBIT;
    assert!(
        measured.bytes_max <= projected,
        "color d=21: peak {} exceeds projection {projected}",
        measured.bytes_max
    );
}

fn qubit_coordinate_target_count(generated: &GeneratedCircuit) -> u64 {
    generated
        .circuit()
        .items()
        .iter()
        .filter_map(|item| match item {
            CircuitItem::Instruction(instruction)
                if instruction.gate().canonical_name() == "QUBIT_COORDS" =>
            {
                Some(u64::try_from(instruction.targets().len()).expect("target count fits u64"))
            }
            CircuitItem::Instruction(_) | CircuitItem::RepeatBlock(_) => None,
        })
        .sum()
}

fn assert_huge_round_counts_stay_folded() {
    let rounds = RoundCount::try_new(u64::MAX).expect("maximum round count");

    let repetition = RepetitionCodeParams::new(
        rounds,
        CodeDistance::try_new(2).expect("distance"),
        RepetitionCodeTask::Memory,
    )
    .expect("repetition parameters");
    let generated = generate_repetition_code_circuit(&repetition).expect("fold repetition rounds");
    assert!(
        generated
            .circuit()
            .to_stim_string()
            .contains("REPEAT 18446744073709551614")
    );

    let surface = SurfaceCodeParams::new(
        rounds,
        CodeDistance::try_new(2).expect("distance"),
        SurfaceCodeTask::RotatedMemoryX,
    )
    .expect("surface parameters");
    let generated = generate_surface_code_circuit(&surface).expect("fold surface rounds");
    assert!(
        generated
            .circuit()
            .to_stim_string()
            .contains("REPEAT 18446744073709551614")
    );

    let color = ColorCodeParams::new(
        rounds,
        CodeDistance::try_new(3).expect("distance"),
        ColorCodeTask::MemoryXyz,
    )
    .expect("color parameters");
    let generated = generate_color_code_circuit(&color).expect("fold color rounds");
    assert!(
        generated
            .circuit()
            .to_stim_string()
            .contains("REPEAT 18446744073709551613")
    );
}
