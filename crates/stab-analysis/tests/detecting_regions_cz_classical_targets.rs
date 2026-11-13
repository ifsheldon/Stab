//! Detecting-region extraction for classically controlled two-qubit Pauli gates: Stim treats
//! `CZ` between classical bits (records or sweep bits) as a no-op, while the other controlled
//! Paulis fail closed on classical-classical target pairs.

use stab_analysis::{DetectingRegionOptions, circuit_detecting_regions};
use stab_model::{Circuit, CircuitTick, DemDetectorId};

fn detector(index: u64) -> Result<DemDetectorId, String> {
    DemDetectorId::try_new(index).map_err(|error| error.to_string())
}

fn detector_zero_region_at_tick_zero(circuit_text: &str) -> Result<String, String> {
    let circuit = Circuit::from_stim_str(circuit_text).map_err(|error| error.to_string())?;
    let detector = detector(0)?;
    let regions = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector],
            ticks: vec![CircuitTick::new(0)],
            ignore_anticommutation_errors: false,
        },
    )
    .map_err(|error| error.to_string())?;
    let ticks = regions
        .get(&detector)
        .ok_or_else(|| "missing detector 0 region map".to_owned())?;
    let region = ticks
        .get(&CircuitTick::new(0))
        .ok_or_else(|| "missing detector 0 tick 0 region".to_owned())?;
    Ok(region.to_string())
}

fn detecting_region_error(circuit_text: &str) -> Result<String, String> {
    let circuit = Circuit::from_stim_str(circuit_text).map_err(|error| error.to_string())?;
    match circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)?],
            ticks: vec![CircuitTick::new(0)],
            ignore_anticommutation_errors: false,
        },
    )
    .map_err(|error| error.to_string())
    {
        Ok(_) => Err("detecting-region extraction unexpectedly succeeded".to_owned()),
        Err(error) => Ok(error),
    }
}

#[test]
fn detecting_regions_treat_stim_negative_zero_detector_as_empty() -> Result<(), String> {
    let circuit = Circuit::from_stim_str("TICK\nM 0\nDETECTOR rec[-0]\n")
        .map_err(|error| error.to_string())?;
    let regions = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)?],
            ticks: vec![CircuitTick::new(0)],
            ignore_anticommutation_errors: true,
        },
    )
    .map_err(|error| error.to_string())?;

    (!regions.contains_key(&detector(0)?))
        .then_some(())
        .ok_or_else(|| format!("expected inactive detector D0 to be omitted, got {regions:?}"))
}

#[test]
fn detecting_regions_treat_classical_cz_pairs_as_noops() -> Result<(), String> {
    for (name, circuit_text, expected_region) in [
        (
            "record-sweep",
            "M 0\nRX 1\nTICK\nCZ rec[-1] sweep[0]\nMX 1\nDETECTOR rec[-1]\n",
            "+_X",
        ),
        (
            "sweep-record",
            "M 0\nRX 1\nTICK\nCZ sweep[0] rec[-1]\nMX 1\nDETECTOR rec[-1]\n",
            "+_X",
        ),
        (
            "record-record",
            "M 0 1\nRX 2\nTICK\nCZ rec[-1] rec[-2]\nMX 2\nDETECTOR rec[-1]\n",
            "+__X",
        ),
        (
            "record-sweep without record history",
            "RX 1\nTICK\nCZ rec[-1] sweep[0]\nMX 1\nDETECTOR rec[-1]\n",
            "+_X",
        ),
        (
            "classical pair beside a quantum pair",
            "R 0 1\nTICK\nH 0\nCZ rec[-1] sweep[0] 0 1\nTICK\nMX 0\nDETECTOR rec[-1]\n",
            "+ZZ",
        ),
        (
            "single sweep-sweep group",
            "RX 0\nTICK\nCZ sweep[0] sweep[1]\nMX 0\nDETECTOR rec[-1]\n",
            "+X",
        ),
        (
            "multiple sweep-sweep groups",
            "RX 0\nTICK\nCZ sweep[0] sweep[1] sweep[2] sweep[3]\nMX 0\nDETECTOR rec[-1]\n",
            "+X",
        ),
    ] {
        let actual = detector_zero_region_at_tick_zero(circuit_text)?;
        if actual != expected_region {
            return Err(format!(
                "{name}\nactual region: {actual}\nexpected region: {expected_region}"
            ));
        }
    }
    Ok(())
}

#[test]
fn detecting_regions_keep_non_cz_classical_pairs_fail_closed() -> Result<(), String> {
    for (circuit_text, expected_error) in [
        (
            "M 0 1\nTICK\nCX rec[-1] rec[-2]\nM 2\nDETECTOR rec[-1]\n",
            "exactly one plain qubit target",
        ),
        (
            "M 0 1\nTICK\nCY rec[-1] rec[-2]\nMY 2\nDETECTOR rec[-1]\n",
            "exactly one plain qubit target",
        ),
        (
            "M 0 1\nTICK\nXCZ rec[-1] rec[-2]\nM 2\nDETECTOR rec[-1]\n",
            "exactly one plain qubit target",
        ),
        (
            "M 0 1\nTICK\nYCZ rec[-1] rec[-2]\nMY 2\nDETECTOR rec[-1]\n",
            "exactly one plain qubit target",
        ),
        (
            "CX sweep[0] sweep[1]\nTICK\nM 0\nDETECTOR rec[-1]\n",
            "exactly one sweep bit and one plain qubit target",
        ),
        (
            "CY sweep[0] sweep[1]\nTICK\nM 0\nDETECTOR rec[-1]\n",
            "exactly one sweep bit and one plain qubit target",
        ),
        (
            "XCZ sweep[0] sweep[1]\nTICK\nM 0\nDETECTOR rec[-1]\n",
            "exactly one sweep bit and one plain qubit target",
        ),
        (
            "YCZ sweep[0] sweep[1]\nTICK\nMY 0\nDETECTOR rec[-1]\n",
            "exactly one sweep bit and one plain qubit target",
        ),
        (
            "M 0\nTICK\nCX rec[-1] sweep[0]\nM 1\nDETECTOR rec[-1]\n",
            "sweep-controlled targets",
        ),
        (
            "M 0\nTICK\nCX sweep[0] rec[-1]\nM 1\nDETECTOR rec[-1]\n",
            "plain qubit target",
        ),
        (
            "M 0\nTICK\nCY rec[-1] sweep[0]\nMY 1\nDETECTOR rec[-1]\n",
            "sweep-controlled targets",
        ),
        (
            "M 0\nTICK\nCY sweep[0] rec[-1]\nMY 1\nDETECTOR rec[-1]\n",
            "plain qubit target",
        ),
        (
            "M 0\nTICK\nXCZ rec[-1] sweep[0]\nM 1\nDETECTOR rec[-1]\n",
            "plain qubit target",
        ),
        (
            "M 0\nTICK\nXCZ sweep[0] rec[-1]\nM 1\nDETECTOR rec[-1]\n",
            "sweep-controlled targets",
        ),
        (
            "M 0\nTICK\nYCZ rec[-1] sweep[0]\nMY 1\nDETECTOR rec[-1]\n",
            "plain qubit targets",
        ),
        (
            "M 0\nTICK\nYCZ sweep[0] rec[-1]\nMY 1\nDETECTOR rec[-1]\n",
            "sweep-controlled targets",
        ),
    ] {
        let error = detecting_region_error(circuit_text)?;
        if !error.contains(expected_error) {
            return Err(format!(
                "{circuit_text}\nmissing: {expected_error}\nactual: {error}"
            ));
        }
    }
    Ok(())
}
