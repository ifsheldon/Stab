use super::{DeferredProduct, FeatureId, UpstreamClassification};

pub(super) fn classify(value: &str, symbol: &str) -> UpstreamClassification {
    let leaf = symbol
        .rsplit('.')
        .next()
        .unwrap_or(symbol)
        .to_ascii_lowercase();

    if value.ends_with("detector_error_model_pybind_test.py") && leaf.contains("shortest_graphlike")
    {
        return UpstreamClassification::selected(FeatureId::Search);
    }

    let binding_only_python_case = if value.ends_with("dem_instruction_pybind_test.py") {
        matches!(
            leaf.as_str(),
            "test_args_copy" | "test_targets_copy" | "test_init_from_str"
        )
    } else if value.ends_with("detector_error_model_pybind_test.py") {
        matches!(
            leaf.as_str(),
            "test_init_get"
                | "test_approx_equals"
                | "test_append"
                | "test_append_bad"
                | "test_coords"
                | "test_dem_from_file"
                | "test_dem_to_file"
                | "test_append_dem_to_dem"
                | "test_init_parse"
        )
    } else {
        false
    };
    let deferred_convenience_case = value.ends_with("detector_error_model.test.cc")
        && matches!(
            leaf.as_str(),
            "from_file" | "py_get_slice" | "mul" | "imul" | "add" | "iadd"
        );
    if binding_only_python_case || deferred_convenience_case {
        return UpstreamClassification::deferred_for(
            [FeatureId::DemFormat],
            DeferredProduct::PythonBindings,
            "Python-style DEM copying, indexing, operators, overloaded append, and file helpers are deferred with Python bindings; selected Rust APIs own their semantic contracts independently.",
        );
    }

    if value.ends_with("detector_error_model.test.cc") && leaf == "movement" {
        return UpstreamClassification::not_applicable(
            "C++ moved-from object state has no Rust value-semantic compatibility contract.",
        );
    }

    if value.ends_with("detector_error_model.test.cc")
        && leaf == "general"
        && symbol.to_ascii_lowercase().starts_with("dem_instruction.")
    {
        return UpstreamClassification::not_applicable(
            "This mixed C++ utility case includes DemInstruction::approx_equals, which is not part of the selected Rust API; exact Rust instruction equality, validation, and canonical printing have independent API and semantic owners.",
        );
    }

    UpstreamClassification::selected(FeatureId::DemFormat)
}
