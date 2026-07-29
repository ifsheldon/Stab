#![allow(
    clippy::expect_used,
    reason = "generation integration checks use direct assertions for compact diagnostics"
)]

use stab_core::advanced::compat::{DetectionConversionOutput, sample_detection_events};
use stab_core::{
    CircuitError, CodeDistance, ColorCodeParams, ColorCodeTask, ErrorAnalyzerOptions,
    GeneratedCircuit, Probability, RepetitionCodeParams, RepetitionCodeTask, RoundCount,
    SurfaceCodeParams, SurfaceCodeTask, circuit_to_detector_error_model,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
};

#[test]
fn generation_facade_preserves_resource_rejection_contract() {
    assert_eq!(
        CodeDistance::try_new(1),
        Err(CircuitError::InvalidDomainValue {
            kind: "code distance",
            value: "1".to_string(),
        })
    );
    assert_eq!(
        RoundCount::try_new(0),
        Err(CircuitError::InvalidDomainValue {
            kind: "round count",
            value: "0".to_string(),
        })
    );

    let surface = SurfaceCodeParams::new(
        RoundCount::try_new(1).expect("rounds"),
        CodeDistance::try_new(257).expect("domain-valid distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("surface parameters");
    assert_rejection_uses_constant_allocation(
        || generate_surface_code_circuit(&surface),
        "rotated surface d=257",
    );

    let color = ColorCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(343).expect("domain-valid distance"),
        ColorCodeTask::MemoryXyz,
    )
    .expect("color parameters");
    assert_rejection_uses_constant_allocation(
        || generate_color_code_circuit(&color),
        "color d=343",
    );
}

fn assert_rejection_uses_constant_allocation(
    reject: impl Fn() -> stab_core::CircuitResult<GeneratedCircuit>,
    context: &str,
) {
    let allocations = allocation_counter::measure(|| {
        let result = reject();
        assert!(
            matches!(result, Err(CircuitError::InvalidDomainValue { .. })),
            "{context}: facade did not preserve InvalidDomainValue"
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

#[test]
fn generation_facade_preserves_zero_allocation_parameter_builders() {
    let rounds = RoundCount::try_new(1).expect("rounds");
    let distance = CodeDistance::try_new(2).expect("distance");
    let color_distance = CodeDistance::try_new(3).expect("color distance");
    let before_round = Probability::try_new(0.0625).expect("probability");
    let before_measure = Probability::try_new(0.125).expect("probability");
    let after_reset = Probability::try_new(0.25).expect("probability");
    let after_clifford = Probability::try_new(0.5).expect("probability");

    let allocations = allocation_counter::measure(|| {
        for _ in 0..128 {
            let repetition =
                RepetitionCodeParams::new(rounds, distance, RepetitionCodeTask::Memory)
                    .expect("fixed repetition parameters")
                    .with_before_round_data_depolarization(before_round)
                    .with_before_measure_flip_probability(before_measure)
                    .with_after_reset_flip_probability(after_reset)
                    .with_after_clifford_depolarization(after_clifford);
            let surface = SurfaceCodeParams::new(rounds, distance, SurfaceCodeTask::RotatedMemoryX)
                .expect("fixed surface parameters")
                .with_before_round_data_depolarization(before_round)
                .with_before_measure_flip_probability(before_measure)
                .with_after_reset_flip_probability(after_reset)
                .with_after_clifford_depolarization(after_clifford);
            let color = ColorCodeParams::new(rounds, color_distance, ColorCodeTask::MemoryXyz)
                .expect("fixed color parameters")
                .with_before_round_data_depolarization(before_round)
                .with_before_measure_flip_probability(before_measure)
                .with_after_reset_flip_probability(after_reset)
                .with_after_clifford_depolarization(after_clifford);
            std::hint::black_box((repetition, surface, color));
        }
    });
    assert_eq!(
        allocations.count_total, 0,
        "fixed-size facade operations allocated: {allocations:?}"
    );
    assert_eq!(
        allocations.bytes_total, 0,
        "fixed-size facade operations allocated: {allocations:?}"
    );
}

#[test]
fn cq2_generation_no_noise_matrix_has_no_detection_or_observable_events() {
    let distances = [2, 3, 4, 5, 6, 7, 15];
    let rounds = [1, 2, 3, 4, 5, 6, 20];
    let surface_tasks = [
        SurfaceCodeTask::RotatedMemoryX,
        SurfaceCodeTask::RotatedMemoryZ,
        SurfaceCodeTask::UnrotatedMemoryX,
        SurfaceCodeTask::UnrotatedMemoryZ,
    ];
    let cases = distances
        .into_iter()
        .flat_map(|distance| rounds.into_iter().map(move |rounds| (distance, rounds)))
        .collect::<Vec<_>>();
    let next_case = std::sync::atomic::AtomicUsize::new(0);
    let worker_count = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .min(8)
        .min(cases.len());
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| {
                loop {
                    let index = next_case.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&(distance, rounds)) = cases.get(index) else {
                        break;
                    };
                    assert_no_noise_matrix_cell(distance, rounds, surface_tasks);
                }
            });
        }
    });

    assert_representative_no_noise_samples(surface_tasks);

    let batch_reference = SurfaceCodeParams::new(
        RoundCount::try_new(3).expect("rounds"),
        CodeDistance::try_new(3).expect("distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("surface parameters");
    let generated =
        generate_surface_code_circuit(&batch_reference).expect("generate batch reference case");
    assert_generated_samples_are_zero(&generated, 256, "portable 256-shot reference");

    let python_reference = SurfaceCodeParams::new(
        RoundCount::try_new(10).expect("rounds"),
        CodeDistance::try_new(5).expect("distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("surface parameters");
    let generated =
        generate_surface_code_circuit(&python_reference).expect("generate Python reference case");
    let output = sample_detection_events(generated.circuit(), 5, Some(0xC0DE))
        .expect("sample Python reference case");
    assert_eq!(output.detector_count, 24 * 10);
    assert_eq!(output.observable_count, 1);
    assert_detection_output_is_zero(&output, 5, "Python d=5 r=10 reference");
}

fn assert_no_noise_matrix_cell(
    distance_value: u32,
    round_value: u64,
    surface_tasks: [SurfaceCodeTask; 4],
) {
    let distance = CodeDistance::try_new(distance_value).expect("matrix distance");
    let round_count = RoundCount::try_new(round_value).expect("matrix rounds");
    let repetition = RepetitionCodeParams::new(round_count, distance, RepetitionCodeTask::Memory)
        .expect("repetition parameters");
    let generated =
        generate_repetition_code_circuit(&repetition).expect("generate repetition matrix");
    assert_generated_structure(
        &generated,
        u64::from(distance_value - 1) * (round_value + 1),
        &format!("repetition d={distance_value} r={round_value}"),
    );

    for task in surface_tasks {
        let params =
            SurfaceCodeParams::new(round_count, distance, task).expect("surface parameters");
        let generated = generate_surface_code_circuit(&params).expect("generate surface matrix");
        let rotated = matches!(
            task,
            SurfaceCodeTask::RotatedMemoryX | SurfaceCodeTask::RotatedMemoryZ
        );
        let (x_measurements, z_measurements) = surface_measurement_counts(distance_value, rotated);
        let chosen_measurements = match task {
            SurfaceCodeTask::RotatedMemoryX | SurfaceCodeTask::UnrotatedMemoryX => x_measurements,
            SurfaceCodeTask::RotatedMemoryZ | SurfaceCodeTask::UnrotatedMemoryZ => z_measurements,
        };
        assert_generated_structure(
            &generated,
            (round_value - 1) * (x_measurements + z_measurements) + 2 * chosen_measurements,
            &format!("surface {task:?} d={distance_value} r={round_value}"),
        );
    }

    if round_value >= 2 && distance_value >= 3 && distance_value % 2 == 1 {
        let params = ColorCodeParams::new(round_count, distance, ColorCodeTask::MemoryXyz)
            .expect("color parameters");
        let generated = generate_color_code_circuit(&params).expect("generate color matrix");
        assert_generated_structure(
            &generated,
            color_measurement_count(distance_value) * round_value,
            &format!("color d={distance_value} r={round_value}"),
        );
    }
}

fn assert_representative_no_noise_samples(surface_tasks: [SurfaceCodeTask; 4]) {
    let distance = CodeDistance::try_new(7).expect("sample distance");
    let rounds = RoundCount::try_new(6).expect("sample rounds");
    let repetition = RepetitionCodeParams::new(rounds, distance, RepetitionCodeTask::Memory)
        .expect("repetition parameters");
    let generated =
        generate_repetition_code_circuit(&repetition).expect("generate repetition sample");
    assert_generated_samples_are_zero(&generated, 1, "repetition d=7 r=6");

    for task in surface_tasks {
        let params = SurfaceCodeParams::new(rounds, distance, task).expect("surface parameters");
        let generated = generate_surface_code_circuit(&params).expect("generate surface sample");
        assert_generated_samples_are_zero(&generated, 1, &format!("surface {task:?} d=7 r=6"));
    }

    let color =
        ColorCodeParams::new(rounds, distance, ColorCodeTask::MemoryXyz).expect("color parameters");
    let generated = generate_color_code_circuit(&color).expect("generate color sample");
    assert_generated_samples_are_zero(&generated, 1, "color d=7 r=6");
}

fn assert_generated_structure(
    generated: &GeneratedCircuit,
    expected_detectors: u64,
    context: &str,
) {
    let circuit = generated.circuit();
    assert_eq!(
        circuit.count_detectors().expect("detector count"),
        expected_detectors,
        "{context}: detector count"
    );
    assert_eq!(
        circuit.count_observables().expect("observable count"),
        1,
        "{context}: observable count"
    );
    let text = circuit.to_stim_string();
    for noise_gate in ["X_ERROR", "Z_ERROR", "DEPOLARIZE1", "DEPOLARIZE2"] {
        assert!(
            !text.contains(noise_gate),
            "{context}: zero-noise circuit contains {noise_gate}"
        );
    }
    let dem = circuit_to_detector_error_model(
        circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    );
    assert!(
        dem.is_ok(),
        "{context}: deterministic analysis failed: {:?}",
        dem.as_ref().err()
    );
    let dem = dem.expect("deterministic analysis result was checked above");
    assert_eq!(
        dem.count_errors().expect("analyzed error count"),
        0,
        "{context}: noiseless generated circuit produced an error mechanism"
    );
}

fn color_measurement_count(distance: u32) -> u64 {
    let width = distance + (distance - 1) / 2;
    let mut count = 0_u64;
    for y in 0..width {
        for x in 0..(width - y) {
            if (x + 2 * y) % 3 == 2 {
                count += 1;
            }
        }
    }
    count
}

fn surface_measurement_counts(distance: u32, rotated: bool) -> (u64, u64) {
    let mut x_measurements = 0_u64;
    let mut z_measurements = 0_u64;
    if rotated {
        for x in 0..=distance {
            for y in 0..=distance {
                let on_x_boundary = x == 0 || x == distance;
                let on_y_boundary = y == 0 || y == distance;
                let parity = x % 2 != y % 2;
                if (on_x_boundary && parity) || (on_y_boundary && !parity) {
                    continue;
                }
                if parity {
                    x_measurements += 1;
                } else {
                    z_measurements += 1;
                }
            }
        }
    } else {
        for x in 0..(2 * distance - 1) {
            for y in 0..(2 * distance - 1) {
                if x % 2 == y % 2 {
                    continue;
                }
                if x % 2 == 0 {
                    z_measurements += 1;
                } else {
                    x_measurements += 1;
                }
            }
        }
    }
    (x_measurements, z_measurements)
}

fn assert_generated_samples_are_zero(generated: &GeneratedCircuit, shots: usize, context: &str) {
    let output = sample_detection_events(generated.circuit(), shots, Some(0x5EED));
    assert!(
        output.is_ok(),
        "{context}: failed to sample: {:?}",
        output.as_ref().err()
    );
    let output = output.expect("sampling result was checked above");
    assert_detection_output_is_zero(&output, shots, context);
}

fn assert_detection_output_is_zero(
    output: &DetectionConversionOutput,
    shots: usize,
    context: &str,
) {
    assert_eq!(output.records.len(), shots, "{context}: shot count");
    for record in &output.records {
        assert!(
            record.detectors.iter().all(|bit| !bit),
            "{context}: nonzero detector"
        );
        assert!(
            record.observables.iter().all(|bit| !bit),
            "{context}: nonzero observable"
        );
    }
}
