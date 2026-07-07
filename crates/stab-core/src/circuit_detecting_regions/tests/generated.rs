use super::*;
use crate::{
    CodeDistance, RepetitionCodeParams, RepetitionCodeTask, RoundCount, SurfaceCodeParams,
    SurfaceCodeTask, generate_repetition_code_circuit, generate_surface_code_circuit,
};

#[test]
fn detecting_regions_generated_repetition_code_filters_and_regions() {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(3).unwrap(),
        CodeDistance::try_new(3).unwrap(),
        RepetitionCodeTask::Memory,
    )
    .unwrap();
    let generated = generate_repetition_code_circuit(&params).unwrap();
    let circuit = generated.circuit();

    let all_targets = all_detecting_region_targets(circuit).unwrap();
    assert_eq!(all_targets.len(), 9);
    assert_eq!(
        all_detecting_region_ticks(circuit).unwrap(),
        (0..9).collect::<Vec<_>>()
    );

    let actual = circuit_detecting_regions_for_targets(
        circuit,
        DetectingRegionTargetOptions {
            targets: all_targets,
            ticks: vec![0, 1, 2, 6, 7, 8],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();

    let d0 = DemTarget::relative_detector(0).unwrap();
    assert_eq!(actual[&d0][&0].to_string(), "+ZZZ__");
    assert_eq!(actual[&d0][&1].to_string(), "+_ZZ__");
    assert_eq!(actual[&d0][&2].to_string(), "+_Z___");
    assert!(!actual[&d0].contains_key(&6));

    let d6 = DemTarget::relative_detector(6).unwrap();
    assert_eq!(actual[&d6][&6].to_string(), "+_Z___");
    assert_eq!(actual[&d6][&7].to_string(), "+ZZ___");
    assert_eq!(actual[&d6][&8].to_string(), "+ZZZ__");

    let logical = DemTarget::logical_observable(0).unwrap();
    for tick in [0, 1, 2, 6, 7, 8] {
        assert_eq!(actual[&logical][&tick].to_string(), "+____Z");
    }
}

#[test]
fn detecting_regions_generated_rotated_surface_code_filters_and_regions() {
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(3).unwrap(),
        CodeDistance::try_new(3).unwrap(),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .unwrap();
    let generated = generate_surface_code_circuit(&params).unwrap();
    let circuit = generated.circuit();

    let all_targets = all_detecting_region_targets(circuit).unwrap();
    let all_ticks = all_detecting_region_ticks(circuit).unwrap();
    assert_eq!(all_targets.len(), 25);
    assert_eq!(all_ticks, (0..=20).collect::<Vec<_>>());

    let selected_targets = vec![
        DemTarget::relative_detector(0).unwrap(),
        DemTarget::relative_detector(4).unwrap(),
        DemTarget::logical_observable(0).unwrap(),
    ];
    let selected_ticks = all_ticks.iter().copied().take(6).collect::<Vec<_>>();
    let actual = circuit_detecting_regions_for_targets(
        circuit,
        DetectingRegionTargetOptions {
            targets: selected_targets.clone(),
            ticks: selected_ticks.clone(),
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();
    assert_eq!(actual.len(), 3);

    let d0 = DemTarget::relative_detector(0).unwrap();
    assert_eq!(actual[&d0][&0].to_string(), "+________Z_____ZZ__________");
    assert_eq!(actual[&d0][&1].to_string(), "+________Z_____ZZ__________");
    assert_eq!(actual[&d0][&2].to_string(), "+________Z_____Z___________");
    assert_eq!(actual[&d0][&3].to_string(), "+______________Z___________");
    assert_eq!(actual[&d0][&4].to_string(), "+______________Z___________");
    assert_eq!(actual[&d0][&5].to_string(), "+______________Z___________");

    let d4 = DemTarget::relative_detector(4).unwrap();
    assert_eq!(actual[&d4][&0].to_string(), "+__Z_______________________");
    assert_eq!(actual[&d4][&1].to_string(), "+__X_______________________");
    assert_eq!(actual[&d4][&2].to_string(), "+__XX______________________");
    assert_eq!(actual[&d4][&3].to_string(), "+_XXX_____X________________");
    assert_eq!(actual[&d4][&4].to_string(), "+_XXX_____X________________");
    assert_eq!(actual[&d4][&5].to_string(), "+_XXX______________________");

    let logical = DemTarget::logical_observable(0).unwrap();
    for (tick, expected) in [
        (0, "+_Z_Z_Z____________________"),
        (1, "+_Z_Z_Z____________________"),
        (2, "+_ZZZ_Z____________________"),
        (3, "+_Z_Z_Z____________________"),
        (4, "+_Z_Z_Z_____Z______________"),
        (5, "+_Z_Z_Z____________________"),
    ] {
        assert_eq!(actual[&logical][&tick].to_string(), expected);
    }
}

#[test]
fn detecting_regions_generated_unrotated_surface_code_filters_and_regions() {
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(3).unwrap(),
        CodeDistance::try_new(3).unwrap(),
        SurfaceCodeTask::UnrotatedMemoryZ,
    )
    .unwrap();
    let generated = generate_surface_code_circuit(&params).unwrap();
    let circuit = generated.circuit();

    let all_targets = all_detecting_region_targets(circuit).unwrap();
    let all_ticks = all_detecting_region_ticks(circuit).unwrap();
    assert_eq!(all_targets.len(), 37);
    assert_eq!(all_ticks, (0..=20).collect::<Vec<_>>());

    let selected_targets = vec![
        DemTarget::relative_detector(0).unwrap(),
        DemTarget::relative_detector(4).unwrap(),
        DemTarget::logical_observable(0).unwrap(),
    ];
    let selected_ticks = all_ticks.iter().copied().take(6).collect::<Vec<_>>();
    let actual = circuit_detecting_regions_for_targets(
        circuit,
        DetectingRegionTargetOptions {
            targets: selected_targets,
            ticks: selected_ticks,
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();
    assert_eq!(actual.len(), 3);

    let d0 = DemTarget::relative_detector(0).unwrap();
    assert_eq!(actual[&d0][&0].to_string(), "+Z____ZZ___Z______________");
    assert_eq!(actual[&d0][&1].to_string(), "+Z____ZZ___Z______________");
    assert_eq!(actual[&d0][&2].to_string(), "+Z____Z____Z______________");
    assert_eq!(actual[&d0][&3].to_string(), "+Z____Z___________________");
    assert_eq!(actual[&d0][&4].to_string(), "+_____Z___________________");
    assert_eq!(actual[&d0][&5].to_string(), "+_____Z___________________");

    let d4 = DemTarget::relative_detector(4).unwrap();
    assert_eq!(actual[&d4][&0].to_string(), "+____Z___ZZ____Z__________");
    assert_eq!(actual[&d4][&1].to_string(), "+____Z___ZZ____Z__________");
    assert_eq!(actual[&d4][&2].to_string(), "+___ZZ___ZZ___ZZ__________");
    assert_eq!(actual[&d4][&3].to_string(), "+____Z___ZZ___Z___________");
    assert_eq!(actual[&d4][&4].to_string(), "+________ZZ_______________");
    assert_eq!(actual[&d4][&5].to_string(), "+_________Z_______________");

    let logical = DemTarget::logical_observable(0).unwrap();
    for (tick, expected) in [
        (0, "+Z_Z_Z____________________"),
        (1, "+Z_Z_Z____________________"),
        (2, "+ZZZZZ____________________"),
        (3, "+ZZZZZ____________________"),
        (4, "+ZZZZZ____________________"),
        (5, "+Z_Z_Z____________________"),
    ] {
        assert_eq!(actual[&logical][&tick].to_string(), expected);
    }
}
