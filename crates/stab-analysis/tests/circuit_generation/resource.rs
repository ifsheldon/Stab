use stab_analysis::{
    AnalysisOperation, CircuitGenerationOptions, CodeCircuitParams, CodeDistance, CodeTask,
    ColorCodeTask, GeneratedCircuit, GeneratorParameter, RepetitionCodeTask, ResourceKind,
    RoundCount, SurfaceCodeTask,
};
use stab_model::{CircuitItem, MemoryBudget, Probability};

/// Admission charges a fixed 1 MiB plus 768 bytes per physical qubit, so a 64 MiB budget admits
/// at most (64 MiB - 1 MiB) / 768 = 86,016 physical qubits.
const BUDGET: MemoryBudget = MemoryBudget::from_bytes(64 * 1024 * 1024);
const BUDGET_QUBITS: u64 = 86_016;
const FIXED_BYTES: u64 = 1024 * 1024;
const BYTES_PER_QUBIT: u64 = 768;

/// Admission projects each family's physical qubit count from the distance and refuses a request
/// whose projected size exceeds the memory budget before materializing anything. The projection
/// is checked against materialized small circuits, the boundary distances sit under the budget
/// while the next distance is refused with the projected count, the tightest boundary (rotated
/// surface code, 85,697 qubits) is materialized once to prove the accepted side end to end, and
/// the projection is checked to dominate the generator's peak allocation.
#[test]
fn cq2_generation_resource_admission_checks_family_boundaries_before_materialization() {
    assert_eq!(
        (BUDGET.bytes() - FIXED_BYTES) / BYTES_PER_QUBIT,
        BUDGET_QUBITS
    );
    assert_repetition_generation_boundary();
    assert_generation_boundary(CodeTask::Surface(SurfaceCodeTask::RotatedMemoryZ), 207, 208);
    assert_generation_boundary(
        CodeTask::Surface(SurfaceCodeTask::UnrotatedMemoryZ),
        147,
        148,
    );
    assert_generation_boundary(CodeTask::Color(ColorCodeTask::MemoryXyz), 275, 277);
    assert_rotated_surface_boundary_materializes();
    assert_projection_dominates_peak_allocation();
    assert_huge_round_counts_stay_folded();
}

fn assert_repetition_generation_boundary() {
    let accepted = CodeCircuitParams::try_new(
        CodeTask::Repetition(RepetitionCodeTask::Memory),
        CodeDistance::try_new(2047).expect("maximum CLI distance"),
        RoundCount::try_new(1).expect("rounds"),
    )
    .expect("repetition parameters");
    let generated = accepted
        .generate(&CircuitGenerationOptions::default())
        .expect("maximum repetition distance remains admissible");
    assert_eq!(generated.circuit().count_qubits(), 4093);
    assert_eq!(accepted.physical_qubit_count(), 4093);
    super::assert_invalid_generator_parameter(
        CodeDistance::try_new(2048),
        GeneratorParameter::CodeDistance,
        2048,
        "invalid code distance value 2048",
    );
}

fn params(task: CodeTask, distance: u32, rounds: u64) -> CodeCircuitParams {
    CodeCircuitParams::try_new(
        task,
        CodeDistance::try_new(distance).expect("domain-valid distance"),
        RoundCount::try_new(rounds).expect("rounds"),
    )
    .expect("valid parameters")
}

fn budget_options() -> CircuitGenerationOptions {
    CircuitGenerationOptions {
        memory_budget: BUDGET,
    }
}

fn budget_refusal(qubits: u64, family: &str) -> String {
    format!(
        "{qubits} physical qubits for {family} need at least {} bytes, exceeding the {}-byte memory budget; raise the memory budget to admit it",
        FIXED_BYTES + qubits * BYTES_PER_QUBIT,
        BUDGET.bytes()
    )
}

/// Each family's physical qubit count, derived independently of the generator.
fn physical_qubits(task: CodeTask, distance: u32) -> u64 {
    let distance = u64::from(distance);
    match task {
        CodeTask::Repetition(_) => 2 * distance - 1,
        CodeTask::Surface(SurfaceCodeTask::RotatedMemoryX | SurfaceCodeTask::RotatedMemoryZ) => {
            distance * distance * 2 - 1
        }
        CodeTask::Surface(_) => (distance * 2 - 1) * (distance * 2 - 1),
        CodeTask::Color(_) => {
            let width = distance + (distance - 1) / 2;
            width * (width + 1) / 2
        }
    }
}

fn family_name(task: CodeTask) -> &'static str {
    match task {
        CodeTask::Repetition(_) => "repetition code",
        CodeTask::Surface(SurfaceCodeTask::RotatedMemoryX | SurfaceCodeTask::RotatedMemoryZ) => {
            "rotated surface code"
        }
        CodeTask::Surface(_) => "unrotated surface code",
        CodeTask::Color(_) => "color code",
    }
}

fn assert_generation_boundary(task: CodeTask, accepted_distance: u32, rejected_distance: u32) {
    let small = match task {
        CodeTask::Color(_) => vec![3, 5, 7, 9],
        _ => (2..=9).collect(),
    };
    for distance in small {
        let params = params(task, distance, 2);
        let generated = params.generate(&budget_options()).expect("small code");
        assert_eq!(
            qubit_coordinate_target_count(&generated),
            physical_qubits(task, distance),
            "{task:?} d={distance} projected and materialized qubit counts diverged"
        );
        assert_eq!(
            params.physical_qubit_count(),
            physical_qubits(task, distance)
        );
    }
    let accepted = params(task, accepted_distance, 2);
    assert!(physical_qubits(task, accepted_distance) <= BUDGET_QUBITS);
    assert!(BUDGET.admits(u128::from(accepted.projected_generation_bytes())));

    let rejected_qubits = physical_qubits(task, rejected_distance);
    assert!(rejected_qubits > BUDGET_QUBITS);
    let error = params(task, rejected_distance, 2)
        .generate(&budget_options())
        .expect_err("a generation beyond the budget is refused");
    let resource = error
        .resource_limit_error()
        .expect("the generation budget refusal is a resource limit");
    assert_eq!(resource.operation(), AnalysisOperation::CircuitGeneration);
    assert_eq!(resource.resource(), ResourceKind::MaterializedBytes);
    assert_eq!(
        resource.actual(),
        FIXED_BYTES + rejected_qubits * BYTES_PER_QUBIT
    );
    assert_eq!(resource.limit(), BUDGET.bytes());
    let exceeded = error
        .memory_budget_exceeded()
        .expect("the generation refusal is a memory-budget refusal");
    assert_eq!(exceeded.budget(), BUDGET);
    assert_eq!(
        exceeded.required().value(),
        FIXED_BYTES + rejected_qubits * BYTES_PER_QUBIT
    );
    assert_eq!(
        error.to_string(),
        format!(
            "invalid generated circuit size value {}",
            budget_refusal(rejected_qubits, family_name(task))
        )
    );
}

fn assert_rotated_surface_boundary_materializes() {
    let generated = params(CodeTask::Surface(SurfaceCodeTask::RotatedMemoryZ), 207, 1)
        .generate(&budget_options())
        .expect("rotated distance 207 is the largest admissible rotated surface code");
    assert!(!generated.layout_text().is_empty());
    assert_eq!(qubit_coordinate_target_count(&generated), 85_697);
}

/// The projection bounds the generator's peak heap allocation, including the noisiest
/// configuration measured (a color code with every noise channel and three rounds, whose last
/// round is inlined).
fn assert_projection_dominates_peak_allocation() {
    let noise = Probability::try_new(0.001).expect("probability");
    for (task, distance, rounds) in [
        (CodeTask::Repetition(RepetitionCodeTask::Memory), 2047, 3),
        (CodeTask::Surface(SurfaceCodeTask::RotatedMemoryZ), 33, 2),
        (CodeTask::Surface(SurfaceCodeTask::UnrotatedMemoryZ), 17, 2),
        (CodeTask::Color(ColorCodeTask::MemoryXyz), 21, 2),
        (CodeTask::Color(ColorCodeTask::MemoryXyz), 51, 3),
    ] {
        let params = params(task, distance, rounds)
            .with_before_round_data_depolarization(noise)
            .with_before_measure_flip_probability(noise)
            .with_after_reset_flip_probability(noise)
            .with_after_clifford_depolarization(noise);
        let measured = allocation_counter::measure(|| {
            std::hint::black_box(params.generate(&budget_options()).expect("generate"));
        });
        let projected = params.projected_generation_bytes();
        assert_eq!(
            projected,
            FIXED_BYTES + physical_qubits(task, distance) * BYTES_PER_QUBIT
        );
        assert!(
            measured.bytes_max <= projected,
            "{task:?} d={distance}: peak {} exceeds projection {projected}",
            measured.bytes_max
        );
    }
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

    let repetition = CodeCircuitParams::try_new(
        CodeTask::Repetition(RepetitionCodeTask::Memory),
        CodeDistance::try_new(2).expect("distance"),
        rounds,
    )
    .expect("repetition parameters");
    let generated = repetition
        .generate(&CircuitGenerationOptions::default())
        .expect("fold repetition rounds");
    assert!(
        generated
            .circuit()
            .to_stim_string()
            .contains("REPEAT 18446744073709551614")
    );

    let surface = CodeCircuitParams::try_new(
        CodeTask::Surface(SurfaceCodeTask::RotatedMemoryX),
        CodeDistance::try_new(2).expect("distance"),
        rounds,
    )
    .expect("surface parameters");
    let generated = surface
        .generate(&CircuitGenerationOptions::default())
        .expect("fold surface rounds");
    assert!(
        generated
            .circuit()
            .to_stim_string()
            .contains("REPEAT 18446744073709551614")
    );

    let color = CodeCircuitParams::try_new(
        CodeTask::Color(ColorCodeTask::MemoryXyz),
        CodeDistance::try_new(3).expect("distance"),
        rounds,
    )
    .expect("color parameters");
    let generated = color
        .generate(&CircuitGenerationOptions::default())
        .expect("fold color rounds");
    assert!(
        generated
            .circuit()
            .to_stim_string()
            .contains("REPEAT 18446744073709551613")
    );
}
