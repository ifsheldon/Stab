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
fn detecting_regions_target_shape_supports_cz_sweep_sweep_noop() -> Result<(), String> {
    let cases = [
        (
            "single bit-bit group",
            "
            RX 0
            TICK
            CZ sweep[0] sweep[1]
            MX 0
            DETECTOR rec[-1]
            ",
        ),
        (
            "multiple bit-bit groups",
            "
            RX 0
            TICK
            CZ sweep[0] sweep[1] sweep[2] sweep[3]
            MX 0
            DETECTOR rec[-1]
            ",
        ),
    ];

    for (name, circuit_text) in cases {
        let actual = detector_region_at(circuit_text, 0)?;
        if actual != "+X" {
            return Err(format!(
                "{name}\nactual region: {actual}\nexpected region: +X"
            ));
        }
    }
    Ok(())
}

#[test]
fn detecting_regions_target_shape_keeps_non_cz_sweep_sweep_fail_closed() -> Result<(), String> {
    for (name, circuit_text) in [
        (
            "CX bit-bit",
            "CX sweep[0] sweep[1]\nTICK\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "CY bit-bit",
            "CY sweep[0] sweep[1]\nTICK\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "XCZ bit-bit",
            "XCZ sweep[0] sweep[1]\nTICK\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "YCZ bit-bit",
            "YCZ sweep[0] sweep[1]\nTICK\nMY 0\nDETECTOR rec[-1]\n",
        ),
    ] {
        let error = detecting_region_error(circuit_text)?;
        require_contains(
            &error,
            "exactly one sweep bit and one plain qubit target",
            name,
        )?;
    }
    Ok(())
}

#[test]
fn detecting_regions_target_shape_keeps_non_cz_record_sweep_fail_closed() -> Result<(), String> {
    for (name, circuit_text, expected_error) in [
        (
            "CX record first",
            "
            M 0
            TICK
            CX rec[-1] sweep[0]
            M 1
            DETECTOR rec[-1]
            ",
            "sweep-controlled targets",
        ),
        (
            "CX record second",
            "
            M 0
            TICK
            CX sweep[0] rec[-1]
            M 1
            DETECTOR rec[-1]
            ",
            "plain qubit target",
        ),
        (
            "CY record first",
            "
            M 0
            TICK
            CY rec[-1] sweep[0]
            MY 1
            DETECTOR rec[-1]
            ",
            "sweep-controlled targets",
        ),
        (
            "CY record second",
            "
            M 0
            TICK
            CY sweep[0] rec[-1]
            MY 1
            DETECTOR rec[-1]
            ",
            "plain qubit target",
        ),
        (
            "XCZ record first",
            "
            M 0
            TICK
            XCZ rec[-1] sweep[0]
            M 1
            DETECTOR rec[-1]
            ",
            "plain qubit target",
        ),
        (
            "XCZ record second",
            "
            M 0
            TICK
            XCZ sweep[0] rec[-1]
            M 1
            DETECTOR rec[-1]
            ",
            "sweep-controlled targets",
        ),
        (
            "YCZ record first",
            "
            M 0
            TICK
            YCZ rec[-1] sweep[0]
            MY 1
            DETECTOR rec[-1]
            ",
            "plain qubit targets",
        ),
        (
            "YCZ record second",
            "
            M 0
            TICK
            YCZ sweep[0] rec[-1]
            MY 1
            DETECTOR rec[-1]
            ",
            "sweep-controlled targets",
        ),
    ] {
        let error = detecting_region_error(circuit_text)?;
        require_contains(&error, expected_error, name)?;
    }
    Ok(())
}
