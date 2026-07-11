use super::*;

#[test]
fn gate_surface_contract_mpp_stochastic() {
    let plan = statistical_plan("pfm3-contract-mpp-stochastic");
    let sampler_circuit = circuit("MPP(0.25) Z0\n");
    let sampler = CompiledSampler::compile(&sampler_circuit).expect("compile stochastic MPP");
    let samples = sampler.sample_zero_one_with_seed(statistical_shot_count(plan), Some(plan.seed));
    assert_statistical_counts(
        plan.case_id,
        &count_binary_records(&samples, "mpp-zero", "mpp-one"),
    );

    let detection_circuit = circuit("MPP(0.25) Z0\nDETECTOR rec[-1]\n");
    let detections = sample_detection_events(
        &detection_circuit,
        statistical_shot_count(plan),
        Some(plan.seed),
    )
    .expect("sample stochastic MPP detections");
    let detection_bits = detections
        .records
        .iter()
        .map(|record| record.detectors.clone())
        .collect::<Vec<_>>();
    assert_statistical_counts(
        plan.case_id,
        &count_binary_records(&detection_bits, "mpp-zero", "mpp-one"),
    );

    let frame_circuit =
        circuit("MPP(0.25) Z0\nOBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(1) X1\n");
    let frames = sample_detection_events(
        &frame_circuit,
        statistical_shot_count(plan),
        Some(plan.seed),
    )
    .expect("sample stochastic MPP frames");
    let frame_bits = frames
        .records
        .iter()
        .map(|record| {
            let [observable, ..] = record.observables.as_slice() else {
                panic!("expected an MPP frame observable: {record:?}");
            };
            vec![*observable]
        })
        .collect::<Vec<_>>();
    assert_statistical_counts(
        plan.case_id,
        &count_binary_records(&frame_bits, "mpp-zero", "mpp-one"),
    );
    assert_eq!(
        circuit_to_detector_error_model(&detection_circuit, ErrorAnalyzerOptions::default())
            .expect("analyze stochastic MPP")
            .to_string(),
        "error(0.25) D0\n"
    );
}

#[test]
fn gate_surface_contract_mpad_stochastic() {
    let plan = statistical_plan("pfm3-contract-mpad-stochastic");
    let sampler_circuit = circuit("MPAD(0.25) 0\n");
    let sampler = CompiledSampler::compile(&sampler_circuit).expect("compile stochastic MPAD");
    let samples = sampler.sample_zero_one_with_seed(statistical_shot_count(plan), Some(plan.seed));
    assert_statistical_counts(
        plan.case_id,
        &count_binary_records(&samples, "mpad-zero", "mpad-one"),
    );

    let detection_circuit = circuit("MPAD(0.25) 0\nDETECTOR rec[-1]\n");
    let detections = sample_detection_events(
        &detection_circuit,
        statistical_shot_count(plan),
        Some(plan.seed),
    )
    .expect("sample stochastic MPAD detections");
    let detection_bits = detections
        .records
        .iter()
        .map(|record| record.detectors.clone())
        .collect::<Vec<_>>();
    assert_statistical_counts(
        plan.case_id,
        &count_binary_records(&detection_bits, "mpad-zero", "mpad-one"),
    );

    let frame_circuit =
        circuit("MPAD(0.25) 0\nOBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(1) X0\n");
    let frames = sample_detection_events(
        &frame_circuit,
        statistical_shot_count(plan),
        Some(plan.seed),
    )
    .expect("sample stochastic MPAD frames");
    let frame_bits = frames
        .records
        .iter()
        .map(|record| {
            let [observable, ..] = record.observables.as_slice() else {
                panic!("expected an MPAD frame observable: {record:?}");
            };
            vec![*observable]
        })
        .collect::<Vec<_>>();
    assert_statistical_counts(
        plan.case_id,
        &count_binary_records(&frame_bits, "mpad-zero", "mpad-one"),
    );
    assert_eq!(
        circuit_to_detector_error_model(&detection_circuit, ErrorAnalyzerOptions::default())
            .expect("analyze stochastic MPAD")
            .to_string(),
        "error(0.25) D0\n"
    );
}

#[test]
fn gate_surface_contract_pauli_noise() {
    assert_family_names(
        &[GateSemanticFamily::PauliNoise],
        &["X_ERROR", "Y_ERROR", "Z_ERROR"],
    );
    let plan = statistical_plan("pfm3-contract-pauli-noise");
    let noise = "X_ERROR(0.5) 0\nY_ERROR(0.5) 0\nZ_ERROR(0.5) 0\n";
    let names = [("identity", "identity"), ("x", "x"), ("y", "y"), ("z", "z")];
    assert_single_pauli_plan_across_surfaces(plan, noise, "", &names);
    assert_all_semantic_surfaces_execute(&bell_detection_circuit(noise));
    for gate in ["X_ERROR", "Y_ERROR", "Z_ERROR"] {
        assert_empty_target_semantic_noop(gate, "(0.5)");
    }
}

#[test]
fn gate_surface_contract_pauli_noise_general_circuit() {
    let text = "X_ERROR(1) 0\nY_ERROR(1) 0\nZ_ERROR(1) 0\nM 0\n";
    assert_exact_reference_and_samples(text, &[false]);
    assert_all_semantic_surfaces_execute(text);
}

#[test]
fn gate_surface_contract_pauli_channels() {
    assert_family_names(
        &[GateSemanticFamily::PauliChannel],
        &["PAULI_CHANNEL_1", "PAULI_CHANNEL_2"],
    );
    let plan = statistical_plan("pfm3-contract-pauli-channels");
    let pc1 = "PAULI_CHANNEL_1(0.1,0.2,0.3) 0\n";
    let pc2 = "PAULI_CHANNEL_2(0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04) 0 1\n";

    let mut sampler_counts = count_single_pauli_samples(
        &sample_records(&bell_measurement_circuit(pc1), plan),
        "pc1-",
    );
    sampler_counts.extend(count_two_pauli_samples(
        &sample_records(&two_bell_measurement_circuit(pc2), plan),
        "pc2-",
    ));
    assert_statistical_counts(plan.case_id, &sampler_counts);

    let mut detection_counts = count_single_pauli_detection(
        &sample_detections(&bell_detection_circuit(pc1), plan),
        "pc1-",
    );
    detection_counts.extend(count_two_pauli_detection(
        &sample_detections(&two_bell_detection_circuit(pc2), plan),
        "pc2-",
    ));
    assert_statistical_counts(plan.case_id, &detection_counts);

    let mut frame_counts = count_single_pauli_frame(
        &sample_detections(&single_pauli_frame_circuit(pc1), plan),
        "pc1-",
    );
    frame_counts.extend(count_two_pauli_frame(
        &sample_detections(&two_pauli_frame_circuit(pc2), plan),
        "pc2-",
    ));
    assert_statistical_counts(plan.case_id, &frame_counts);

    assert_all_semantic_surfaces_execute(&bell_detection_circuit(pc1));
    assert_all_semantic_surfaces_execute(&two_bell_detection_circuit(pc2));
    assert_empty_target_semantic_noop("PAULI_CHANNEL_1", "(0.1,0.2,0.3)");
    assert_empty_target_semantic_noop(
        "PAULI_CHANNEL_2",
        "(0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04,0.04)",
    );
}

#[test]
fn gate_surface_contract_pauli_channels_general_circuit() {
    let text =
        "PAULI_CHANNEL_1(0,0,0) 0\nPAULI_CHANNEL_2(0,0,0,0,0,0,0,0,0,0,0,0,0,0,0) 0 1\nM 0 1\n";
    assert_exact_reference_and_samples(text, &[false, false]);
    assert_all_semantic_surfaces_execute(text);
}

#[test]
fn gate_surface_contract_depolarization() {
    assert_family_names(
        &[GateSemanticFamily::Depolarization],
        &["DEPOLARIZE1", "DEPOLARIZE2"],
    );
    let plan = statistical_plan("pfm3-contract-depolarization");
    let depol1 = "DEPOLARIZE1(0.6) 0\n";
    let depol2 = "DEPOLARIZE2(0.75) 0 1\n";

    let mut sampler_counts = count_single_pauli_samples(
        &sample_records(&bell_measurement_circuit(depol1), plan),
        "depol1-",
    );
    sampler_counts.extend(count_identity_vs_nonidentity_samples(
        &sample_records(&two_bell_measurement_circuit(depol2), plan),
        "depol2-ii",
        "depol2-nonidentity",
    ));
    assert_statistical_counts(plan.case_id, &sampler_counts);

    let mut detection_counts = count_single_pauli_detection(
        &sample_detections(&bell_detection_circuit(depol1), plan),
        "depol1-",
    );
    detection_counts.extend(count_identity_vs_nonidentity_detection(
        &sample_detections(&two_bell_detection_circuit(depol2), plan),
        "depol2-ii",
        "depol2-nonidentity",
    ));
    assert_statistical_counts(plan.case_id, &detection_counts);

    let mut frame_counts = count_single_pauli_frame(
        &sample_detections(&single_pauli_frame_circuit(depol1), plan),
        "depol1-",
    );
    frame_counts.extend(count_identity_vs_nonidentity_frame(
        &sample_detections(&two_pauli_frame_circuit(depol2), plan),
        "depol2-ii",
        "depol2-nonidentity",
    ));
    assert_statistical_counts(plan.case_id, &frame_counts);

    assert_all_semantic_surfaces_execute(&bell_detection_circuit(depol1));
    assert_all_semantic_surfaces_execute(&two_bell_detection_circuit(depol2));
    assert_empty_target_semantic_noop("DEPOLARIZE1", "(0.6)");
    assert_empty_target_semantic_noop("DEPOLARIZE2", "(0.75)");
}

#[test]
fn gate_surface_contract_depolarization_general_circuit() {
    let text = "DEPOLARIZE1(0) 0\nDEPOLARIZE2(0) 0 1\nM 0 1\n";
    assert_exact_reference_and_samples(text, &[false, false]);
    assert_all_semantic_surfaces_execute(text);
}

#[test]
fn gate_surface_contract_correlated_errors() {
    assert_family_names(
        &[GateSemanticFamily::CorrelatedError],
        &["E", "ELSE_CORRELATED_ERROR"],
    );
    let plan = statistical_plan("pfm3-contract-correlated-errors");
    let noise = "E(0.2) X0\nELSE_CORRELATED_ERROR(0.25) Y0\nELSE_CORRELATED_ERROR(0.5) Z0\n";
    let names = [
        ("identity", "no-error"),
        ("x", "first-branch"),
        ("y", "else-branch-one"),
        ("z", "else-branch-two"),
    ];
    assert_single_pauli_plan_across_surfaces(plan, noise, "", &names);
    assert_all_semantic_surfaces_execute(&bell_detection_circuit(noise));

    let empty_first = circuit("E(1)\nELSE_CORRELATED_ERROR(1) X0\nM 0\n");
    assert_eq!(
        CompiledSampler::compile(&empty_first)
            .expect("compile empty first branch")
            .sample_zero_one_with_seed(4, Some(1)),
        vec![vec![false]; 4],
        "an empty successful first branch must suppress ELSE_CORRELATED_ERROR"
    );
    let empty_else =
        circuit("E(0) X0\nELSE_CORRELATED_ERROR(1)\nELSE_CORRELATED_ERROR(1) X0\nM 0\n");
    assert_eq!(
        CompiledSampler::compile(&empty_else)
            .expect("compile empty else branch")
            .sample_zero_one_with_seed(4, Some(1)),
        vec![vec![false]; 4],
        "an empty successful else branch must suppress later ELSE_CORRELATED_ERROR"
    );
}

#[test]
fn gate_surface_contract_correlated_errors_general_circuit() {
    let text = "E(0) X0\nELSE_CORRELATED_ERROR(0) Y0\nELSE_CORRELATED_ERROR(0) Z0\nM 0\n";
    assert_exact_reference_and_samples(text, &[false]);
    assert_all_semantic_surfaces_execute(text);
}

#[test]
fn gate_surface_contract_heralded_noise() {
    assert_family_names(
        &[GateSemanticFamily::HeraldedNoise],
        &["HERALDED_ERASE", "HERALDED_PAULI_CHANNEL_1"],
    );
    let plan = statistical_plan("pfm3-contract-heralded-noise");
    let erase = "HERALDED_ERASE(0.1) 0\n";
    assert_heralded_plan_across_surfaces(plan, erase, true);
    assert_all_semantic_surfaces_execute(&heralded_bell_detection_circuit(erase));
    assert_empty_target_semantic_noop("HERALDED_ERASE", "(0.5)");
}

#[test]
fn gate_surface_contract_heralded_channel() {
    let plan = statistical_plan("pfm3-contract-heralded-channel");
    let channel = "HERALDED_PAULI_CHANNEL_1(0.05,0.1,0.15,0.25) 0\n";
    assert_heralded_plan_across_surfaces(plan, channel, false);
    assert_all_semantic_surfaces_execute(&heralded_bell_detection_circuit(channel));
    assert_empty_target_semantic_noop("HERALDED_PAULI_CHANNEL_1", "(0.1,0.2,0.3,0.1)");
}

#[test]
fn gate_surface_contract_heralded_erase_offset() {
    let plan = statistical_plan("pfm3-contract-heralded-erase-offset");
    let erase = "HERALDED_ERASE(0.1) 2\n";
    let circuit = heralded_bell_detection_circuit_on(erase, 2, 3);
    let counts = count_heralded_detection(&sample_detections(&circuit, plan), true);
    assert_statistical_counts(plan.case_id, &counts);
    assert_all_semantic_surfaces_execute(&circuit);
}

#[test]
fn gate_surface_contract_heralded_channel_offset() {
    let plan = statistical_plan("pfm3-contract-heralded-channel-offset");
    let channel = "HERALDED_PAULI_CHANNEL_1(0.05,0.1,0.15,0.25) 2\n";
    let circuit = heralded_bell_detection_circuit_on(channel, 2, 3);
    let counts = count_heralded_detection(&sample_detections(&circuit, plan), false);
    assert_statistical_counts(plan.case_id, &counts);
    assert_all_semantic_surfaces_execute(&circuit);
}

fn assert_heralded_plan_across_surfaces(
    plan: &super::super::super::GateContractStatisticalPlan,
    noise: &str,
    erase: bool,
) {
    let sampler_counts = count_heralded_samples(
        &sample_records(&heralded_bell_measurement_circuit(noise), plan),
        erase,
    );
    assert_statistical_counts(plan.case_id, &sampler_counts);

    let detection_counts = count_heralded_detection(
        &sample_detections(&heralded_bell_detection_circuit(noise), plan),
        erase,
    );
    assert_statistical_counts(plan.case_id, &detection_counts);

    let frame_counts = count_heralded_frame(
        &sample_detections(&heralded_frame_circuit(noise), plan),
        erase,
    );
    assert_statistical_counts(plan.case_id, &frame_counts);
}

fn assert_single_pauli_plan_across_surfaces(
    plan: &super::super::super::GateContractStatisticalPlan,
    noise: &str,
    prefix: &str,
    names: &[(&str, &str)],
) {
    let remap = |counts: BTreeMap<&'static str, usize>| {
        counts
            .into_iter()
            .map(|(name, count)| {
                let target = names
                    .iter()
                    .find_map(|(source, target)| (*source == name).then_some(*target))
                    .unwrap_or_else(|| panic!("missing remap for {name}"));
                (target, count)
            })
            .collect::<BTreeMap<_, _>>()
    };
    let samples = sample_records(&bell_measurement_circuit(noise), plan);
    assert_statistical_counts(
        plan.case_id,
        &remap(count_single_pauli_samples(&samples, prefix)),
    );
    let detections = sample_detections(&bell_detection_circuit(noise), plan);
    assert_statistical_counts(
        plan.case_id,
        &remap(count_single_pauli_detection(&detections, prefix)),
    );
    let frames = sample_detections(&single_pauli_frame_circuit(noise), plan);
    assert_statistical_counts(
        plan.case_id,
        &remap(count_single_pauli_frame(&frames, prefix)),
    );
}

fn sample_records(
    text: &str,
    plan: &super::super::super::GateContractStatisticalPlan,
) -> Vec<Vec<bool>> {
    CompiledSampler::compile(&circuit(text))
        .expect("compile statistical sampler")
        .sample_zero_one_with_seed(statistical_shot_count(plan), Some(plan.seed))
}

fn sample_detections(
    text: &str,
    plan: &super::super::super::GateContractStatisticalPlan,
) -> crate::DetectionConversionOutput {
    sample_detection_events(
        &circuit(text),
        statistical_shot_count(plan),
        Some(plan.seed),
    )
    .expect("sample statistical detections")
}

fn bell_measurement_circuit(noise: &str) -> String {
    format!("H 0\nCX 0 1\n{noise}MPP X0*X1 Z0*Z1\n")
}

fn bell_detection_circuit(noise: &str) -> String {
    format!(
        "{}DETECTOR rec[-2]\nDETECTOR rec[-1]\n",
        bell_measurement_circuit(noise)
    )
}

fn single_pauli_frame_circuit(noise: &str) -> String {
    format!("H 0\nCX 0 1\n{noise}OBSERVABLE_INCLUDE(0) X0 X1\nOBSERVABLE_INCLUDE(1) Z0 Z1\n")
}

fn two_bell_measurement_circuit(noise: &str) -> String {
    format!("H 0 1\nCX 0 2 1 3\n{noise}MPP X0*X2 Z0*Z2 X1*X3 Z1*Z3\n")
}

fn two_bell_detection_circuit(noise: &str) -> String {
    format!(
        "{}DETECTOR rec[-4]\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
        two_bell_measurement_circuit(noise)
    )
}

fn two_pauli_frame_circuit(noise: &str) -> String {
    format!(
        "H 0 1\nCX 0 2 1 3\n{noise}OBSERVABLE_INCLUDE(0) X0 X2\nOBSERVABLE_INCLUDE(1) Z0 Z2\nOBSERVABLE_INCLUDE(2) X1 X3\nOBSERVABLE_INCLUDE(3) Z1 Z3\n"
    )
}

fn heralded_bell_measurement_circuit(noise: &str) -> String {
    format!("H 0\nCX 0 1\n{noise}CX rec[-1] 2\nM 2\nMPP X0*X1 Z0*Z1\n")
}

fn heralded_bell_detection_circuit(noise: &str) -> String {
    heralded_bell_detection_circuit_on(noise, 0, 1)
}

fn heralded_bell_detection_circuit_on(noise: &str, first: usize, second: usize) -> String {
    format!(
        "H {first}\nCX {first} {second}\n{noise}DETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) X{first} X{second}\nOBSERVABLE_INCLUDE(1) Z{first} Z{second}\n"
    )
}

fn heralded_frame_circuit(noise: &str) -> String {
    format!(
        "H 0\nCX 0 1\n{noise}OBSERVABLE_INCLUDE(0) X0 X1\nOBSERVABLE_INCLUDE(1) Z0 Z1\nOBSERVABLE_INCLUDE(2) rec[-1]\n"
    )
}

fn count_binary_records(
    records: &[Vec<bool>],
    zero: &'static str,
    one: &'static str,
) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::from([(zero, 0), (one, 0)]);
    for record in records {
        let [bit] = record.as_slice() else {
            panic!("expected one statistical bit, got {record:?}");
        };
        *counts
            .get_mut(if *bit { one } else { zero })
            .expect("bucket") += 1;
    }
    counts
}

fn count_single_pauli_samples(
    records: &[Vec<bool>],
    prefix: &str,
) -> BTreeMap<&'static str, usize> {
    count_single_paulis(records.iter().map(Vec::as_slice), prefix)
}

fn count_single_pauli_detection(
    output: &crate::DetectionConversionOutput,
    prefix: &str,
) -> BTreeMap<&'static str, usize> {
    count_single_paulis(
        output
            .records
            .iter()
            .map(|record| record.detectors.as_slice()),
        prefix,
    )
}

fn count_single_pauli_frame(
    output: &crate::DetectionConversionOutput,
    prefix: &str,
) -> BTreeMap<&'static str, usize> {
    count_single_paulis(
        output
            .records
            .iter()
            .map(|record| record.observables.as_slice()),
        prefix,
    )
}

fn count_single_paulis<'a>(
    records: impl Iterator<Item = &'a [bool]>,
    prefix: &str,
) -> BTreeMap<&'static str, usize> {
    let names = single_pauli_names(prefix);
    let mut counts = names
        .iter()
        .map(|(_, name)| (*name, 0))
        .collect::<BTreeMap<_, _>>();
    for record in records {
        let source = single_pauli_label(record);
        let name = names
            .iter()
            .find_map(|(candidate, name)| (*candidate == source).then_some(*name))
            .expect("single-Pauli bucket name");
        *counts.get_mut(name).expect("single-Pauli bucket") += 1;
    }
    counts
}

fn single_pauli_names(prefix: &str) -> [(&'static str, &'static str); 4] {
    match prefix {
        "" => [("i", "identity"), ("x", "x"), ("y", "y"), ("z", "z")],
        "pc1-" => [
            ("i", "pc1-i"),
            ("x", "pc1-x"),
            ("y", "pc1-y"),
            ("z", "pc1-z"),
        ],
        "depol1-" => [
            ("i", "depol1-i"),
            ("x", "depol1-x"),
            ("y", "depol1-y"),
            ("z", "depol1-z"),
        ],
        other => panic!("unknown single-Pauli prefix {other}"),
    }
}

fn single_pauli_label(bits: &[bool]) -> &'static str {
    match bits {
        [false, false] => "i",
        [false, true] => "x",
        [true, true] => "y",
        [true, false] => "z",
        other => panic!("expected two Pauli-classification bits, got {other:?}"),
    }
}

fn count_two_pauli_samples(records: &[Vec<bool>], prefix: &str) -> BTreeMap<&'static str, usize> {
    count_two_paulis(records.iter().map(Vec::as_slice), prefix)
}

fn count_two_pauli_detection(
    output: &crate::DetectionConversionOutput,
    prefix: &str,
) -> BTreeMap<&'static str, usize> {
    count_two_paulis(
        output
            .records
            .iter()
            .map(|record| record.detectors.as_slice()),
        prefix,
    )
}

fn count_two_pauli_frame(
    output: &crate::DetectionConversionOutput,
    prefix: &str,
) -> BTreeMap<&'static str, usize> {
    count_two_paulis(
        output
            .records
            .iter()
            .map(|record| record.observables.as_slice()),
        prefix,
    )
}

fn count_two_paulis<'a>(
    records: impl Iterator<Item = &'a [bool]>,
    prefix: &str,
) -> BTreeMap<&'static str, usize> {
    assert_eq!(prefix, "pc2-");
    let names = [
        "pc2-ii", "pc2-ix", "pc2-iy", "pc2-iz", "pc2-xi", "pc2-xx", "pc2-xy", "pc2-xz", "pc2-yi",
        "pc2-yx", "pc2-yy", "pc2-yz", "pc2-zi", "pc2-zx", "pc2-zy", "pc2-zz",
    ];
    let mut counts = names
        .into_iter()
        .map(|name| (name, 0))
        .collect::<BTreeMap<_, _>>();
    for record in records {
        let [a, b, c, d] = record else {
            panic!("expected four two-Pauli classification bits, got {record:?}");
        };
        let first = pauli_index(&[*a, *b]);
        let second = pauli_index(&[*c, *d]);
        let name = names
            .get(first * 4 + second)
            .copied()
            .expect("two-Pauli classification index");
        *counts.get_mut(name).expect("two-Pauli bucket") += 1;
    }
    counts
}

fn pauli_index(bits: &[bool; 2]) -> usize {
    match bits {
        [false, false] => 0,
        [false, true] => 1,
        [true, true] => 2,
        [true, false] => 3,
    }
}

fn count_identity_vs_nonidentity_samples(
    records: &[Vec<bool>],
    identity: &'static str,
    nonidentity: &'static str,
) -> BTreeMap<&'static str, usize> {
    count_identity_vs_nonidentity(records.iter().map(Vec::as_slice), identity, nonidentity)
}

fn count_identity_vs_nonidentity_detection(
    output: &crate::DetectionConversionOutput,
    identity: &'static str,
    nonidentity: &'static str,
) -> BTreeMap<&'static str, usize> {
    count_identity_vs_nonidentity(
        output
            .records
            .iter()
            .map(|record| record.detectors.as_slice()),
        identity,
        nonidentity,
    )
}

fn count_identity_vs_nonidentity_frame(
    output: &crate::DetectionConversionOutput,
    identity: &'static str,
    nonidentity: &'static str,
) -> BTreeMap<&'static str, usize> {
    count_identity_vs_nonidentity(
        output
            .records
            .iter()
            .map(|record| record.observables.as_slice()),
        identity,
        nonidentity,
    )
}

fn count_identity_vs_nonidentity<'a>(
    records: impl Iterator<Item = &'a [bool]>,
    identity: &'static str,
    nonidentity: &'static str,
) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::from([(identity, 0), (nonidentity, 0)]);
    for record in records {
        let name = if record.iter().all(|bit| !bit) {
            identity
        } else {
            nonidentity
        };
        *counts.get_mut(name).expect("identity bucket") += 1;
    }
    counts
}

fn count_heralded_samples(records: &[Vec<bool>], erase: bool) -> BTreeMap<&'static str, usize> {
    count_heralded(records.iter().map(Vec::as_slice), erase)
}

fn count_heralded_detection(
    output: &crate::DetectionConversionOutput,
    erase: bool,
) -> BTreeMap<&'static str, usize> {
    count_heralded(
        output.records.iter().map(|record| {
            let [herald] = record.detectors.as_slice() else {
                panic!("expected one herald detector: {record:?}");
            };
            let [first, second] = record.observables.as_slice() else {
                panic!("expected two heralded Pauli observables: {record:?}");
            };
            [*herald, *first, *second]
        }),
        erase,
    )
}

fn count_heralded_frame(
    output: &crate::DetectionConversionOutput,
    erase: bool,
) -> BTreeMap<&'static str, usize> {
    count_heralded(
        output.records.iter().map(|record| {
            let [first, second, herald] = record.observables.as_slice() else {
                panic!("expected three heralded frame observables: {record:?}");
            };
            [*herald, *first, *second]
        }),
        erase,
    )
}

fn count_heralded<'a, I, R>(records: I, erase: bool) -> BTreeMap<&'static str, usize>
where
    I: Iterator<Item = R>,
    R: AsRef<[bool]> + 'a,
{
    let names = if erase {
        vec![
            "erase-no-herald",
            "erase-i",
            "erase-x",
            "erase-y",
            "erase-z",
        ]
    } else {
        vec![
            "no-herald",
            "herald-no-data-error",
            "herald-x",
            "herald-y",
            "herald-z",
        ]
    };
    let mut counts = names
        .iter()
        .map(|name| (*name, 0))
        .collect::<BTreeMap<_, _>>();
    for record in records {
        let record = record.as_ref();
        let [herald, first, second] = record else {
            panic!("expected herald and two Pauli-classification bits, got {record:?}");
        };
        let pauli = single_pauli_label(&[*first, *second]);
        let name = if erase && !*herald {
            assert_eq!(pauli, "i", "unheralded erasure outcome must be identity");
            "erase-no-herald"
        } else if erase {
            match pauli {
                "i" => "erase-i",
                "x" => "erase-x",
                "y" => "erase-y",
                "z" => "erase-z",
                _ => unreachable!(),
            }
        } else if !*herald {
            assert_eq!(pauli, "i", "unheralded channel outcome must be identity");
            "no-herald"
        } else {
            match pauli {
                "i" => "herald-no-data-error",
                "x" => "herald-x",
                "y" => "herald-y",
                "z" => "herald-z",
                _ => unreachable!(),
            }
        };
        *counts.get_mut(name).expect("heralded bucket") += 1;
    }
    counts
}
