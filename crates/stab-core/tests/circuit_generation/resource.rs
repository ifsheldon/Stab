use stab_core::{
    CircuitError, CircuitItem, CodeDistance, ColorCodeParams, ColorCodeTask, GeneratedCircuit,
    RepetitionCodeParams, RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
};

#[test]
fn cq2_generation_resource_admission_checks_family_boundaries_before_materialization() {
    assert_repetition_generation_boundary();
    assert_surface_generation_boundary(SurfaceCodeTask::RotatedMemoryZ, 256, 257, 132_097);
    assert_surface_generation_boundary(SurfaceCodeTask::UnrotatedMemoryZ, 181, 182, 131_769);
    assert_color_generation_boundary();
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

fn assert_surface_generation_boundary(
    task: SurfaceCodeTask,
    accepted_distance: u32,
    rejected_distance: u32,
    rejected_qubits: u64,
) {
    let accepted = SurfaceCodeParams::new(
        RoundCount::try_new(1).expect("rounds"),
        CodeDistance::try_new(accepted_distance).expect("accepted distance"),
        task,
    )
    .expect("surface parameters");
    let generated = generate_surface_code_circuit(&accepted);
    assert!(
        generated.is_ok(),
        "{task:?} d={accepted_distance} should fit: {:?}",
        generated.as_ref().err()
    );
    let generated = generated.expect("accepted surface result was checked above");
    assert!(!generated.circuit().items().is_empty());
    assert!(!generated.layout_text().is_empty());
    let accepted_physical_qubits = if matches!(
        task,
        SurfaceCodeTask::RotatedMemoryX | SurfaceCodeTask::RotatedMemoryZ
    ) {
        u64::from(accepted_distance) * u64::from(accepted_distance) * 2 - 1
    } else {
        let width = u64::from(accepted_distance) * 2 - 1;
        width * width
    };
    assert_eq!(
        qubit_coordinate_target_count(&generated),
        accepted_physical_qubits,
        "{task:?} projected and materialized qubit counts diverged"
    );
    drop(generated);

    let rejected = SurfaceCodeParams::new(
        RoundCount::try_new(1).expect("rounds"),
        CodeDistance::try_new(rejected_distance).expect("domain-valid rejected distance"),
        task,
    )
    .expect("surface parameters");
    let family = if matches!(
        task,
        SurfaceCodeTask::RotatedMemoryX | SurfaceCodeTask::RotatedMemoryZ
    ) {
        "rotated surface code"
    } else {
        "unrotated surface code"
    };
    super::assert_invalid_domain_value(
        generate_surface_code_circuit(&rejected),
        "generated circuit physical qubit count",
        &format!("{rejected_qubits} for {family}; current limit is 131072"),
    );
    assert_rejection_uses_constant_allocation(
        || generate_surface_code_circuit(&rejected),
        &format!("{task:?} d={rejected_distance}"),
    );
}

fn assert_color_generation_boundary() {
    let accepted = ColorCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(341).expect("accepted distance"),
        ColorCodeTask::MemoryXyz,
    )
    .expect("color parameters");
    let generated = generate_color_code_circuit(&accepted)
        .expect("largest family-valid color distance under the limit should fit");
    assert!(!generated.circuit().items().is_empty());
    assert!(!generated.layout_text().is_empty());
    assert_eq!(
        qubit_coordinate_target_count(&generated),
        130_816,
        "color projected and materialized qubit counts diverged"
    );
    drop(generated);

    let rejected = ColorCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(343).expect("domain-valid rejected distance"),
        ColorCodeTask::MemoryXyz,
    )
    .expect("color parameters");
    super::assert_invalid_domain_value(
        generate_color_code_circuit(&rejected),
        "generated circuit physical qubit count",
        "132355 for color code; current limit is 131072",
    );
    assert_rejection_uses_constant_allocation(
        || generate_color_code_circuit(&rejected),
        "color d=343",
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

fn assert_rejection_uses_constant_allocation(
    reject: impl Fn() -> stab_core::CircuitResult<GeneratedCircuit>,
    context: &str,
) {
    let allocations = allocation_counter::measure(|| {
        let result = reject();
        assert!(
            matches!(result, Err(CircuitError::InvalidDomainValue { .. })),
            "{context}: measured call did not return InvalidDomainValue"
        );
        drop(std::hint::black_box(result));
    });
    assert!(
        allocations.count_total <= 8,
        "{context}: rejection performed too many allocations: {allocations:?}"
    );
    assert!(
        allocations.bytes_total <= 1_024,
        "{context}: rejection allocated too many bytes: {allocations:?}"
    );
    assert!(
        allocations.bytes_max <= 512,
        "{context}: rejection retained too many live bytes: {allocations:?}"
    );
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
