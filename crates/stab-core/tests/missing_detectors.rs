use stab_core::{
    Circuit, Gate, MissingDetectorOptions, PauliBasis, PauliSign, PauliString, SingleQubitClifford,
    missing_detectors,
};

fn missing_with_options(
    text: &str,
    ignore_non_deterministic_measurements: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let circuit = Circuit::from_stim_str(text)?;
    missing_circuit_with_options(&circuit, ignore_non_deterministic_measurements)
}

fn missing_circuit_with_options(
    circuit: &Circuit,
    ignore_non_deterministic_measurements: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let output = missing_detectors(
        circuit,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements,
        },
    )?;
    Ok(output.to_stim_string())
}

fn missing(text: &str) -> Result<String, Box<dyn std::error::Error>> {
    missing_with_options(text, true)
}

fn require_missing_eq(
    text: &str,
    ignore_non_deterministic_measurements: bool,
    expected: &str,
    context: impl std::fmt::Display,
) -> Result<(), Box<dyn std::error::Error>> {
    let actual = missing_with_options(text, ignore_non_deterministic_measurements)?;
    if actual != expected {
        return Err(std::io::Error::other(format!(
            "{context}: expected {expected:?}, got {actual:?}"
        ))
        .into());
    }
    Ok(())
}

fn reset_gate(basis: PauliBasis) -> &'static str {
    match basis {
        PauliBasis::I => "R",
        PauliBasis::X => "RX",
        PauliBasis::Y => "RY",
        PauliBasis::Z => "R",
    }
}

fn measurement_gate(basis: PauliBasis) -> Result<&'static str, Box<dyn std::error::Error>> {
    match basis {
        PauliBasis::I => Err(std::io::Error::other("identity basis is not measurable").into()),
        PauliBasis::X => Ok("MX"),
        PauliBasis::Y => Ok("MY"),
        PauliBasis::Z => Ok("M"),
    }
}

fn mpp_target(bases: impl IntoIterator<Item = (usize, PauliBasis)>) -> String {
    bases
        .into_iter()
        .filter_map(|(qubit, basis)| match basis {
            PauliBasis::I => None,
            PauliBasis::X => Some(format!("X{qubit}")),
            PauliBasis::Y => Some(format!("Y{qubit}")),
            PauliBasis::Z => Some(format!("Z{qubit}")),
        })
        .collect::<Vec<_>>()
        .join("*")
}

#[test]
fn pf5_missing_detectors_clifford_tracks_single_qubit_basis_changes()
-> Result<(), Box<dyn std::error::Error>> {
    require_missing_eq("R 0\nH 0\nMX 0\n", true, "DETECTOR rec[-1]\n", "R H MX")?;
    require_missing_eq(
        "H 0\nMX 0\n",
        false,
        "DETECTOR rec[-1]\n",
        "known-input H MX",
    )?;
    require_missing_eq("H 0\nMX 0\n", true, "", "unknown-input H MX")?;
    require_missing_eq("R 0\nH 0\nM 0\n", true, "", "R H M nondeterministic")?;
    require_missing_eq("RX 0\nS 0\nMY 0\n", true, "DETECTOR rec[-1]\n", "RX S MY")?;
    Ok(())
}

#[test]
fn pf5_missing_detectors_clifford_covers_all_single_qubit_cliffords()
-> Result<(), Box<dyn std::error::Error>> {
    let input_bases = [PauliBasis::X, PauliBasis::Y, PauliBasis::Z];
    for clifford in SingleQubitClifford::all() {
        let gate = Gate::from_name(clifford.canonical_name())?;
        let tableau = gate.tableau()?;
        for input_basis in input_bases {
            let output = tableau.apply(&PauliString::from_bases(PauliSign::Plus, [input_basis]))?;
            let output_basis = output
                .get(0)
                .ok_or_else(|| std::io::Error::other("missing single-qubit tableau output"))?;
            let circuit = format!(
                "{} 0\n{} 0\n{} 0\n",
                reset_gate(input_basis),
                gate.canonical_name(),
                measurement_gate(output_basis)?
            );
            require_missing_eq(
                &circuit,
                true,
                "DETECTOR rec[-1]\n",
                format!("{} input {input_basis:?}", gate.canonical_name()),
            )?;
        }
    }
    Ok(())
}

#[test]
fn pf5_missing_detectors_clifford_tracks_two_qubit_and_swap_gates()
-> Result<(), Box<dyn std::error::Error>> {
    require_missing_eq(
        "R 0 1\nH 0\nCX 0 1\nMXX 0 1\n",
        true,
        "DETECTOR rec[-1]\n",
        "R H CX MXX",
    )?;
    require_missing_eq(
        "R 0\nSWAP 0 1\nM 1\n",
        true,
        "DETECTOR rec[-1]\n",
        "R SWAP M",
    )?;
    require_missing_eq(
        "RX 0\nISWAP_DAG 0 1\nMPP Z0*Y1\n",
        true,
        "DETECTOR rec[-1]\n",
        "RX ISWAP_DAG signed output",
    )?;
    require_missing_eq(
        "R 0 1\nH 0\nCX 0 1\nM 0\n",
        true,
        "",
        "entangled Z0 is not deterministic",
    )?;
    Ok(())
}

#[test]
fn pf5_missing_detectors_clifford_covers_all_fixed_two_qubit_tableau_gates()
-> Result<(), Box<dyn std::error::Error>> {
    let input_bases = [PauliBasis::I, PauliBasis::X, PauliBasis::Y, PauliBasis::Z];
    for gate in Gate::all().filter(|gate| gate.has_tableau() && gate.is_two_qubit_gate()) {
        let tableau = gate.tableau()?;
        for left_basis in input_bases {
            for right_basis in input_bases {
                if left_basis == PauliBasis::I && right_basis == PauliBasis::I {
                    continue;
                }
                let output = tableau.apply(&PauliString::from_bases(
                    PauliSign::Plus,
                    [left_basis, right_basis],
                ))?;
                let output_bases = [
                    output
                        .get(0)
                        .ok_or_else(|| std::io::Error::other("missing left tableau output"))?,
                    output
                        .get(1)
                        .ok_or_else(|| std::io::Error::other("missing right tableau output"))?,
                ];
                let input_resets = [(0, left_basis), (1, right_basis)]
                    .into_iter()
                    .filter(|(_, basis)| *basis != PauliBasis::I)
                    .map(|(qubit, basis)| format!("{} {qubit}\n", reset_gate(basis)))
                    .collect::<String>();
                let circuit = format!(
                    "{input_resets}{} 0 1\nMPP {}\n",
                    gate.canonical_name(),
                    mpp_target([(0, output_bases[0]), (1, output_bases[1])])
                );
                require_missing_eq(
                    &circuit,
                    true,
                    "DETECTOR rec[-1]\n",
                    format!(
                        "{} input {left_basis:?}{right_basis:?}",
                        gate.canonical_name()
                    ),
                )?;
            }
        }
    }
    Ok(())
}

#[test]
fn pf5_missing_detectors_clifford_rejects_non_plain_unitary_targets()
-> Result<(), Box<dyn std::error::Error>> {
    let circuit = Circuit::from_stim_str("M 0\nCX rec[-1] 1\nM 1\n")?;
    let Err(error) = missing_detectors(
        &circuit,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: true,
        },
    ) else {
        return Err(std::io::Error::other("expected non-plain unitary target rejection").into());
    };
    if !error.to_string().contains("plain qubit") {
        return Err(std::io::Error::other(format!("unexpected error: {error}")).into());
    }
    Ok(())
}

#[test]
fn pf5_missing_detectors_spp_has_pinned_outputs() -> Result<(), Box<dyn std::error::Error>> {
    require_missing_eq(
        "RX 0\nSPP Z0\nMY 0\n",
        true,
        "DETECTOR rec[-1]\n",
        "SPP Z rotates known X input into deterministic Y measurement",
    )?;
    require_missing_eq(
        "RX 0\nSPP_DAG Z0\nMY 0\n",
        true,
        "DETECTOR rec[-1]\n",
        "SPP_DAG Z rotates known X input into deterministic Y measurement",
    )?;
    require_missing_eq(
        "RX 0\nSPP !Z0\nMY 0\n",
        true,
        "DETECTOR rec[-1]\n",
        "inverted SPP Z rotates known X input into deterministic Y measurement",
    )?;
    require_missing_eq(
        "RX 0 1\nSPP Z0 Z1\nMY 0 1\n",
        true,
        "DETECTOR rec[-2]\nDETECTOR rec[-1]\n",
        "multi-group SPP Z rotations preserve per-target deterministic measurements",
    )?;
    require_missing_eq(
        "SPP Z0\nMY 0\n",
        true,
        "",
        "unknown input after SPP stays nondeterministic when nondeterministic rows are ignored",
    )?;
    Ok(())
}

#[test]
fn pf5_missing_detectors_spp_supports_unitary_products() -> Result<(), Box<dyn std::error::Error>> {
    let mut saw_non_empty_detector_suggestion = false;
    for (name, text) in [
        ("spp z", "RX 0\nSPP Z0\nMY 0\n"),
        ("spp_dag z", "RX 0\nSPP_DAG Z0\nMY 0\n"),
        ("spp product", "RX 0\nRY 1\nSPP X0*Y1*Z2\nMPP Z0*Z1*Z2\n"),
        (
            "spp_dag inverted product",
            "R 0\nRX 1\nSPP_DAG !Z0*X1\nMPP Y0*Y1\n",
        ),
    ] {
        let circuit = Circuit::from_stim_str(text)?;
        let expected = missing_circuit_with_options(&circuit.decomposed()?, true)?;
        let actual = missing_circuit_with_options(&circuit, true)?;
        if actual != expected {
            return Err(std::io::Error::other(format!(
                "{name}: expected decomposed missing detectors {expected:?}, got {actual:?}"
            ))
            .into());
        }
        saw_non_empty_detector_suggestion |= !actual.is_empty();
    }
    if !saw_non_empty_detector_suggestion {
        return Err(
            std::io::Error::other("expected at least one SPP case to suggest a detector").into(),
        );
    }
    Ok(())
}

#[test]
fn pf5_missing_detectors_spp_rejects_anti_hermitian_unitary_products()
-> Result<(), Box<dyn std::error::Error>> {
    for gate in ["SPP", "SPP_DAG"] {
        let circuit = Circuit::from_stim_str(&format!("{gate} X0*Z0\nM 0\n"))?;
        let Err(error) = missing_detectors(
            &circuit,
            MissingDetectorOptions {
                ignore_non_deterministic_measurements: true,
            },
        ) else {
            return Err(
                std::io::Error::other(format!("expected anti-Hermitian {gate} rejection")).into(),
            );
        };
        if !error.to_string().contains("anti-Hermitian") {
            return Err(std::io::Error::other(format!("unexpected {gate} error: {error}")).into());
        }
    }
    Ok(())
}

#[test]
fn pf5_missing_detectors_repeat_tracks_deterministic_measurements()
-> Result<(), Box<dyn std::error::Error>> {
    require_missing_eq(
        "R 0\nREPEAT 3 {\n    M 0\n}\n",
        true,
        "DETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
        "three repeated deterministic measurements",
    )?;
    require_missing_eq(
        "R 0\nREPEAT 2 {\n    M 0\n    DETECTOR rec[-1]\n}\n",
        true,
        "",
        "repeat body covers each deterministic measurement",
    )?;
    Ok(())
}

#[test]
fn pf5_missing_detectors_repeat_handles_nested_rows_and_known_rows()
-> Result<(), Box<dyn std::error::Error>> {
    require_missing_eq(
        "R 0\nREPEAT 2 {\n    REPEAT 2 {\n        M 0\n    }\n}\nDETECTOR rec[-1]\n",
        true,
        "DETECTOR rec[-4]\nDETECTOR rec[-3]\nDETECTOR rec[-2]\n",
        "nested repeats with final known row",
    )?;
    require_missing_eq(
        "R 0\nREPEAT 2 {\n    M 0\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        true,
        "DETECTOR rec[-2]\n",
        "repeat rows reduced against observable row",
    )?;
    Ok(())
}

#[test]
fn pf5_missing_detectors_repeat_folds_final_covered_deterministic_loop()
-> Result<(), Box<dyn std::error::Error>> {
    require_missing_eq(
        "REPEAT 1000001 {\n    M 0\n    DETECTOR rec[-1]\n}\n",
        false,
        "",
        "known-input final repeat with local detector rows",
    )?;
    require_missing_eq(
        "R 0\nREPEAT 1000001 {\n    M 0\n    DETECTOR rec[-1]\n}\n",
        true,
        "",
        "reset-prefix final repeat with local detector rows",
    )?;
    Ok(())
}

#[test]
fn pf5_missing_detectors_repeat_keeps_unselected_large_repeats_capped()
-> Result<(), Box<dyn std::error::Error>> {
    let cross_iteration = Circuit::from_stim_str(
        "R 0\nM 0\nREPEAT 1000001 {\n    M 0\n    DETECTOR rec[-1] rec[-2]\n}\n",
    )?;
    let Err(error) = missing_detectors(
        &cross_iteration,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: true,
        },
    ) else {
        return Err(std::io::Error::other("expected cross-iteration repeat rejection").into());
    };
    if !error.to_string().contains("expanded repeat iterations") {
        return Err(std::io::Error::other(format!("unexpected error: {error}")).into());
    }

    let observable_merging = Circuit::from_stim_str(
        "R 0\nREPEAT 1000001 {\n    M 0\n    OBSERVABLE_INCLUDE(0) rec[-1]\n}\n",
    )?;
    let Err(error) = missing_detectors(
        &observable_merging,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: true,
        },
    ) else {
        return Err(std::io::Error::other("expected observable-row repeat rejection").into());
    };
    if !error.to_string().contains("expanded repeat iterations") {
        return Err(std::io::Error::other(format!("unexpected error: {error}")).into());
    }

    for (context, text) in [
        (
            "unsupported shift coords",
            "REPEAT 1000001 {\n    SHIFT_COORDS(1)\n}\n",
        ),
        (
            "unsupported measurement pad",
            "REPEAT 1000001 {\n    MPAD 0\n}\n",
        ),
    ] {
        let circuit = Circuit::from_stim_str(text)?;
        let Err(error) = missing_detectors(
            &circuit,
            MissingDetectorOptions {
                ignore_non_deterministic_measurements: true,
            },
        ) else {
            return Err(
                std::io::Error::other(format!("expected {context} repeat rejection")).into(),
            );
        };
        if !error.to_string().contains("expanded repeat iterations") {
            return Err(
                std::io::Error::other(format!("{context}: unexpected error: {error}")).into(),
            );
        }
    }

    let tracker_changing =
        Circuit::from_stim_str("REPEAT 1000001 {\n    R 0\n    M 0\n    DETECTOR rec[-1]\n}\n")?;
    let Err(error) = missing_detectors(
        &tracker_changing,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: true,
        },
    ) else {
        return Err(std::io::Error::other("expected tracker-changing repeat rejection").into());
    };
    if !error.to_string().contains("expanded repeat iterations") {
        return Err(std::io::Error::other(format!("unexpected error: {error}")).into());
    }
    Ok(())
}

#[test]
fn pf5_missing_detectors_repeat_rejects_excessive_expansion()
-> Result<(), Box<dyn std::error::Error>> {
    let circuit = Circuit::from_stim_str("REPEAT 1000001 {\n    M 0\n}\n")?;
    let Err(error) = missing_detectors(
        &circuit,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: true,
        },
    ) else {
        return Err(std::io::Error::other("expected excessive repeat expansion rejection").into());
    };
    if !error.to_string().contains("expanded repeat iterations") {
        return Err(std::io::Error::other(format!("unexpected error: {error}")).into());
    }

    let circuit = Circuit::from_stim_str("REPEAT 1000000 {\n    M 0 1\n}\n")?;
    let Err(error) = missing_detectors(
        &circuit,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: true,
        },
    ) else {
        return Err(std::io::Error::other("expected excessive repeat work-unit rejection").into());
    };
    if !error.to_string().contains("expanded work units") {
        return Err(std::io::Error::other(format!("unexpected error: {error}")).into());
    }

    let circuit = Circuit::from_stim_str("REPEAT 500001 {\n    SPP X0\n}\n")?;
    let Err(error) = missing_detectors(
        &circuit,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: true,
        },
    ) else {
        return Err(std::io::Error::other("expected decomposed SPP repeat work rejection").into());
    };
    if !error.to_string().contains("expanded work units") {
        return Err(std::io::Error::other(format!("unexpected error: {error}")).into());
    }
    Ok(())
}

#[test]
fn missing_detectors_supports_honeycomb_generated_code_suffix()
-> Result<(), Box<dyn std::error::Error>> {
    // Adapted from Stim v1.16.0 src/stim/util_top/missing_detectors.test.cc.
    let actual = missing(include_str!(
        "fixtures/missing_detectors_honeycomb_missing_detector.stim"
    ))?;
    let expected = "DETECTOR rec[-377] rec[-375] rec[-374] rec[-317] rec[-315] rec[-314]\n";
    if actual != expected {
        return Err(std::io::Error::other(format!("expected {expected:?}, got {actual:?}")).into());
    }
    Ok(())
}
