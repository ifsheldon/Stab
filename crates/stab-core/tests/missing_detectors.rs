use stab_core::{
    Circuit, Gate, MissingDetectorOptions, PauliBasis, PauliSign, PauliString, SingleQubitClifford,
    missing_detectors,
};

fn missing_with_options(
    text: &str,
    ignore_non_deterministic_measurements: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let circuit = Circuit::from_stim_str(text)?;
    let output = missing_detectors(
        &circuit,
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
