use stab_core::{Circuit, MissingDetectorOptions, missing_detectors};

fn missing(text: &str) -> Result<String, Box<dyn std::error::Error>> {
    let circuit = Circuit::from_stim_str(text)?;
    let output = missing_detectors(
        &circuit,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: true,
        },
    )?;
    Ok(output.to_stim_string())
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
