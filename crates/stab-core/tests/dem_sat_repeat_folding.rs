#![allow(
    clippy::panic_in_result_fn,
    reason = "integration tests use direct assertions for compact diagnostics"
)]

use stab_core::{
    CircuitResult, DetectorErrorModel, likeliest_error_sat_problem, shortest_error_sat_problem,
};

fn dem(input: &str) -> CircuitResult<DetectorErrorModel> {
    DetectorErrorModel::from_dem_str(input)
}

#[test]
fn sat_problem_shortest_folds_large_flat_zero_shift_repeats() -> CircuitResult<()> {
    let model = dem("\
repeat 100001 {
    error(0.1) D0 L0
    error(0.2) D0
}
")?;
    let expected = shortest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.2) D0\n")?)?;
    assert_eq!(shortest_error_sat_problem(&model)?, expected);
    Ok(())
}

#[test]
fn sat_problem_shortest_folds_large_flat_zero_shift_zero_probability_repeats() -> CircuitResult<()>
{
    let model = dem("\
repeat 100001 {
    error(0) D0 L0
    error(0) D0
}
")?;
    let expected = shortest_error_sat_problem(&dem("error(0) D0 L0\nerror(0) D0\n")?)?;
    assert_eq!(shortest_error_sat_problem(&model)?, expected);
    Ok(())
}

#[test]
fn sat_problem_shortest_folds_large_nested_zero_shift_repeats() -> CircuitResult<()> {
    let model = dem("\
repeat 100001 {
    detector(1, 2) D0
    repeat 100001 {
        error(0.1) D0 L0
        shift_detectors 0
        error(0.2) D0
    }
}
")?;
    let expected = shortest_error_sat_problem(&dem("error(0.1) D0 L0\nerror(0.2) D0\n")?)?;
    assert_eq!(shortest_error_sat_problem(&model)?, expected);

    let zero_probability = dem("\
repeat 100001 {
    repeat 100001 {
        error(0) D0 L0
        shift_detectors 0
        error(0) D0
    }
}
")?;
    let expected_zero = shortest_error_sat_problem(&dem("error(0) D0 L0\nerror(0) D0\n")?)?;
    assert_eq!(
        shortest_error_sat_problem(&zero_probability)?,
        expected_zero
    );

    let no_target = dem("\
repeat 100001 {
    repeat 100001 {
        error(0.1)
        shift_detectors 0
    }
}
error(0.1) D0
error(0.1) D0 L0
")?;
    let expected_no_target =
        shortest_error_sat_problem(&dem("error(0.1)\nerror(0.1) D0\nerror(0.1) D0 L0\n")?)?;
    assert_eq!(shortest_error_sat_problem(&no_target)?, expected_no_target);

    let shifted = dem("\
repeat 100001 {
    repeat 100001 {
        error(0.1) D0 L0
        shift_detectors 1
    }
}
")?;
    let error = match shortest_error_sat_problem(&shifted) {
        Ok(output) => {
            format!("nested shifted SAT repeats unexpectedly bypassed the cap: {output}")
        }
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("DEM SAT problem generation currently supports repeat counts up to"),
        "{error}"
    );
    Ok(())
}

#[test]
fn sat_problem_shortest_compresses_large_flat_zero_shift_high_observable_repeat()
-> CircuitResult<()> {
    let model = dem("\
repeat 100001 {
    error(0) L1000001
}
")?;
    assert_eq!(
        shortest_error_sat_problem(&model)?,
        shortest_error_sat_problem(&dem("error(0) L0\n")?)?
    );
    Ok(())
}

#[test]
fn sat_problem_likeliest_folds_large_flat_zero_shift_repeats_by_map_cost() -> CircuitResult<()> {
    let model = dem("\
repeat 100001 {
    error(0.000001) D0 L0
    error(0.25) D1 L1
}
error(0.1) D0
error(0.1) D0 L0
error(0.1) D1 L1
")?;
    let expected = likeliest_error_sat_problem(
        &dem("\
error(0.000001) D0 L0
error(0.25) D1 L1
error(0.1) D0
error(0.1) D0 L0
error(0.1) D1 L1
")?,
        100,
    )?;
    assert_eq!(likeliest_error_sat_problem(&model, 100)?, expected);

    let small_probability_counterexample = dem("\
repeat 100001 {
    error(0.000001) L0
}
error(0.01) L0
")?;
    let compact_counterexample =
        likeliest_error_sat_problem(&dem("error(0.000001) L0\nerror(0.01) L0\n")?, 100)?;
    assert_eq!(
        likeliest_error_sat_problem(&small_probability_counterexample, 100)?,
        compact_counterexample
    );

    let even_high_probability = dem("\
repeat 100002 {
    error(0.9) D0 L0
}
error(0.1) D0
error(0.1) D0 L0
")?;
    let even_high_probability_compact = likeliest_error_sat_problem(
        &dem("error(0.1) D0 L0\nerror(0.1) D0\nerror(0.1) D0 L0\n")?,
        100,
    )?;
    assert_eq!(
        likeliest_error_sat_problem(&even_high_probability, 100)?,
        even_high_probability_compact
    );

    let odd_high_probability = dem("\
repeat 100001 {
    error(0.9) D0 L0
}
error(0.1) D0
error(0.1) D0 L0
")?;
    let odd_high_probability_compact = likeliest_error_sat_problem(
        &dem("error(0.9) D0 L0\nerror(0.1) D0\nerror(0.1) D0 L0\n")?,
        100,
    )?;
    assert_eq!(
        likeliest_error_sat_problem(&odd_high_probability, 100)?,
        odd_high_probability_compact
    );

    let even_deterministic = dem("\
repeat 100002 {
    error(1) D0 L0
}
error(0.1) D0
error(0.1) D0 L0
")?;
    let without_even_deterministic =
        likeliest_error_sat_problem(&dem("error(0.1) D0\nerror(0.1) D0 L0\n")?, 100)?;
    assert_eq!(
        likeliest_error_sat_problem(&even_deterministic, 100)?,
        without_even_deterministic
    );

    let odd_deterministic = dem("\
repeat 100001 {
    error(1) D0 L0
}
")?;
    let one_deterministic = likeliest_error_sat_problem(&dem("error(1) D0 L0\n")?, 100)?;
    assert_eq!(
        likeliest_error_sat_problem(&odd_deterministic, 100)?,
        one_deterministic
    );
    Ok(())
}

#[test]
fn sat_problem_likeliest_folds_large_nested_zero_shift_repeats_by_map_cost() -> CircuitResult<()> {
    let model = dem("\
repeat 100001 {
    detector(1, 2) D0
    repeat 100001 {
        error(0.000001) D0 L0
        shift_detectors 0
        error(0.25) D1 L1
    }
}
error(0.1) D0
error(0.1) D0 L0
error(0.1) D1 L1
")?;
    let expected = likeliest_error_sat_problem(
        &dem("\
error(0.000001) D0 L0
error(0.25) D1 L1
error(0.1) D0
error(0.1) D0 L0
error(0.1) D1 L1
")?,
        100,
    )?;
    assert_eq!(likeliest_error_sat_problem(&model, 100)?, expected);

    let even_high_probability = dem("\
repeat 100001 {
    repeat 100002 {
        error(0.9) D0 L0
        shift_detectors 0
    }
}
error(0.1) D0
error(0.1) D0 L0
")?;
    let even_high_probability_compact = likeliest_error_sat_problem(
        &dem("error(0.1) D0 L0\nerror(0.1) D0\nerror(0.1) D0 L0\n")?,
        100,
    )?;
    assert_eq!(
        likeliest_error_sat_problem(&even_high_probability, 100)?,
        even_high_probability_compact
    );

    let even_deterministic = dem("\
repeat 100001 {
    repeat 100002 {
        error(1) D0 L0
        shift_detectors 0
    }
}
error(0.1) D0
error(0.1) D0 L0
")?;
    let without_even_deterministic =
        likeliest_error_sat_problem(&dem("error(0.1) D0\nerror(0.1) D0 L0\n")?, 100)?;
    assert_eq!(
        likeliest_error_sat_problem(&even_deterministic, 100)?,
        without_even_deterministic
    );

    let odd_deterministic = dem("\
repeat 100001 {
    repeat 100001 {
        error(1) D0 L0
        shift_detectors 0
    }
}
")?;
    let one_deterministic = likeliest_error_sat_problem(&dem("error(1) D0 L0\n")?, 100)?;
    assert_eq!(
        likeliest_error_sat_problem(&odd_deterministic, 100)?,
        one_deterministic
    );

    let zero_probability = dem("\
repeat 100001 {
    repeat 100001 {
        error(0) D1000000 L1000
        shift_detectors 0
    }
}
error(0.1) D0
error(0.1) D0 L0
")?;
    let expected_zero =
        likeliest_error_sat_problem(&dem("error(0.1) D0\nerror(0.1) D0 L0\n")?, 100)?;
    assert_eq!(
        likeliest_error_sat_problem(&zero_probability, 100)?,
        expected_zero
    );

    let no_target = dem("\
repeat 100001 {
    repeat 100001 {
        error(0.1)
        shift_detectors 0
    }
}
error(0.1) D0
error(0.1) D0 L0
")?;
    let expected_no_target =
        likeliest_error_sat_problem(&dem("error(0.1)\nerror(0.1) D0\nerror(0.1) D0 L0\n")?, 100)?;
    assert_eq!(
        likeliest_error_sat_problem(&no_target, 100)?,
        expected_no_target
    );

    let shifted = dem("\
repeat 100001 {
    repeat 100001 {
        error(0.1) D0 L0
        shift_detectors 1
    }
}
")?;
    let error = match likeliest_error_sat_problem(&shifted, 100) {
        Ok(output) => {
            format!("nested shifted weighted SAT repeats unexpectedly bypassed the cap: {output}")
        }
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains("DEM SAT problem generation currently supports repeat counts up to"),
        "{error}"
    );
    Ok(())
}
