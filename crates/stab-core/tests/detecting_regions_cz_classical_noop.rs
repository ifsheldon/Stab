use stab_core::{Circuit, DemDetectorId, DetectingRegionOptions, circuit_detecting_regions};

fn detector(index: u64) -> Result<DemDetectorId, String> {
    DemDetectorId::try_new(index).map_err(|error| error.to_string())
}

fn detector_region_at(circuit_text: &str, tick: u64) -> Result<String, String> {
    let circuit = Circuit::from_stim_str(circuit_text).map_err(|error| error.to_string())?;
    let detector = detector(0)?;
    let regions = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector],
            ticks: vec![tick],
            ignore_anticommutation_errors: false,
        },
    )
    .map_err(|error| error.to_string())?;
    let ticks = regions
        .get(&detector)
        .ok_or_else(|| "missing detector 0 region map".to_owned())?;
    let region = ticks
        .get(&tick)
        .ok_or_else(|| format!("missing detector 0 tick {tick} region"))?;
    Ok(region.to_string())
}

fn detecting_region_error(circuit_text: &str) -> Result<String, String> {
    let circuit = Circuit::from_stim_str(circuit_text).map_err(|error| error.to_string())?;
    match circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)?],
            ticks: vec![0],
            ignore_anticommutation_errors: false,
        },
    )
    .map_err(|error| error.to_string())
    {
        Ok(_) => Err("detecting-region extraction unexpectedly succeeded".to_owned()),
        Err(error) => Ok(error),
    }
}

fn require_contains(haystack: &str, needle: &str, context: &str) -> Result<(), String> {
    if haystack.contains(needle) {
        Ok(())
    } else {
        Err(format!("{context}\nmissing: {needle}\nactual: {haystack}"))
    }
}

#[test]
fn detecting_regions_target_shape_supports_cz_record_sweep_noop() -> Result<(), String> {
    for (name, circuit_text) in [
        (
            "record first",
            "
            M 0
            RX 1
            TICK
            CZ rec[-1] sweep[0]
            MX 1
            DETECTOR rec[-1]
            ",
        ),
        (
            "record second",
            "
            M 0
            RX 1
            TICK
            CZ sweep[0] rec[-1]
            MX 1
            DETECTOR rec[-1]
            ",
        ),
    ] {
        let actual = detector_region_at(circuit_text, 0)?;
        if actual != "+_X" {
            return Err(format!(
                "{name}\nactual region: {actual}\nexpected region: +_X"
            ));
        }
    }
    Ok(())
}

#[test]
fn detecting_regions_target_shape_supports_cz_record_record_noop() -> Result<(), String> {
    let actual = detector_region_at(
        "
        M 0 1
        RX 2
        TICK
        CZ rec[-1] rec[-2]
        MX 2
        DETECTOR rec[-1]
        ",
        0,
    )?;
    if actual != "+__X" {
        return Err(format!("actual region: {actual}\nexpected region: +__X"));
    }
    Ok(())
}

#[test]
fn detecting_regions_target_shape_cz_classical_noop_skips_record_history() -> Result<(), String> {
    let actual = detector_region_at(
        "
        RX 1
        TICK
        CZ rec[-1] sweep[0]
        MX 1
        DETECTOR rec[-1]
        ",
        0,
    )?;
    if actual != "+_X" {
        return Err(format!("actual region: {actual}\nexpected region: +_X"));
    }
    Ok(())
}

#[test]
fn detecting_regions_target_shape_cz_classical_noop_keeps_quantum_groups() -> Result<(), String> {
    let actual = detector_region_at(
        "
        R 0 1
        TICK
        H 0
        CZ rec[-1] sweep[0] 0 1
        TICK
        MX 0
        DETECTOR rec[-1]
        ",
        0,
    )?;
    if actual != "+ZZ" {
        return Err(format!("actual region: {actual}\nexpected region: +ZZ"));
    }
    Ok(())
}

#[test]
fn detecting_regions_target_shape_keeps_non_cz_record_record_fail_closed() -> Result<(), String> {
    for (name, circuit_text) in [
        (
            "CX record-record",
            "M 0 1\nTICK\nCX rec[-1] rec[-2]\nM 2\nDETECTOR rec[-1]\n",
        ),
        (
            "CY record-record",
            "M 0 1\nTICK\nCY rec[-1] rec[-2]\nMY 2\nDETECTOR rec[-1]\n",
        ),
        (
            "XCZ record-record",
            "M 0 1\nTICK\nXCZ rec[-1] rec[-2]\nM 2\nDETECTOR rec[-1]\n",
        ),
        (
            "YCZ record-record",
            "M 0 1\nTICK\nYCZ rec[-1] rec[-2]\nMY 2\nDETECTOR rec[-1]\n",
        ),
    ] {
        let error = detecting_region_error(circuit_text)?;
        require_contains(&error, "exactly one plain qubit target", name)?;
    }
    Ok(())
}
