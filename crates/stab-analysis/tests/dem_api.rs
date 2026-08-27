#![allow(
    clippy::expect_used,
    reason = "generated DEM integration tests use direct fixture assertions"
)]

use stab_analysis::{
    CodeDistance, ErrorAnalyzerOptions, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    circuit_to_detector_error_model, generate_surface_code_circuit,
};
use stab_model::{CircuitDetectorId, DemItem, Probability};

#[test]
fn pf6_dem_generated_surface_code_fold_loop_coordinates_match_circuit() {
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(7).expect("round count"),
        CodeDistance::try_new(5).expect("code distance"),
        SurfaceCodeTask::RotatedMemoryX,
    )
    .expect("surface-code params")
    .with_after_clifford_depolarization(Probability::try_new(0.01).expect("probability"));
    let generated = generate_surface_code_circuit(&params).expect("generate surface-code circuit");
    let circuit = generated.circuit();
    let circuit_coordinates = circuit
        .detector_coordinates()
        .expect("all circuit detector coordinates");
    let options = ErrorAnalyzerOptions {
        fold_loops: true,
        decompose_errors: true,
        block_decomposition_from_introducing_remnant_edges: true,
        ..ErrorAnalyzerOptions::default()
    };
    let dem = circuit_to_detector_error_model(circuit, options)
        .expect("analyze generated surface-code circuit");
    assert!(
        dem.items()
            .iter()
            .any(|item| matches!(item, DemItem::RepeatBlock(_))),
        "folded surface-code DEM must preserve a compact repeat block"
    );
    let dem_coordinates = dem
        .detector_coordinates()
        .expect("all DEM detector coordinates");

    assert_eq!(dem.count_detectors().expect("DEM detector count"), 168);
    assert_eq!(circuit_coordinates.len(), 168);
    assert_eq!(dem_coordinates.len(), 168);
    for (detector, dem_coordinate) in &dem_coordinates {
        assert_eq!(
            circuit_coordinates
                .get(&CircuitDetectorId::new(detector.get()))
                .expect("matching circuit detector coordinate"),
            dem_coordinate,
            "coordinate mismatch for D{}",
            detector.get()
        );
    }
}
