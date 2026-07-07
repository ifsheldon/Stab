pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "pf4-dem-flatten-repeat" => Some(
            "contract-only: Stab measures the Rust DetectorErrorModel::flattened public API over repeat, tag, detector-shift, coordinate-shift, separator, and observable cases; pinned Stim exposes equivalent behavior but not a faithful Rust direct baseline",
        ),
        "pf4-dem-rounded" => Some(
            "contract-only: Stab measures the Rust DetectorErrorModel::rounded public API over top-level and nested error probabilities while preserving non-error coordinate args; pinned Stim exposes equivalent behavior but not a faithful Rust direct baseline",
        ),
        "pf4-dem-coordinate-map" => Some(
            "contract-only: Stab measures bounded all-detector DEM coordinate maps, selected detector coordinate lookup through a huge-repeat model, sparse flat and nested overlapping selected-coordinate lookups, and many-selected flat-overlap coordinate lookup; pinned Stim exposes equivalent behavior but not a faithful Rust direct baseline",
        ),
        "pf4-dem-sampler-folded-repeat" => Some(
            "contract-only: Stab measures folded CompiledDemSampler compile, stochastic direct sample behavior, zero-probability repeat skipping, deterministic zero-shift repeat parity folding, selected direct detection-event single-stochastic zero-shift repeat parity folding, selected direct detection-event flat stochastic zero-shift repeat parity folding, and selected direct detection-event nested zero-shift stochastic repeat parity folding; sampled-error materialization and replay caps are source-owned by PF4 tests instead of timed submeasurements, and non-selected excessive stochastic repeated-error work plus broader PF4 traversal consumers remain explicit follow-up work",
        ),
        "pf4-dem-folded-traversal" => Some(
            "contract-only: Stab measures current capped-repeat hypergraph search, zero-probability repeat skipping for hypergraph search, selected flat detector-touching zero-shift hypergraph search repeat folding, weighted SAT zero-probability variable elision and repeated-body skipping, capped unselected SAT problem generation, analyzer traversal, and ErrorMatcher circuit traversal; true folded traversal remains an explicit RPF4 follow-up",
        ),
        "pf4-dem-folded-graphlike-traversal" => Some(
            "contract-only: Stab measures current capped-repeat graphlike search behavior, zero-probability repeat skipping, selected flat detector-touching and detectorless logical-only zero-shift graphlike repeat folding, and selected flat no-target graphlike repeat skipping; true folded graphlike traversal remains an explicit RPF4 follow-up",
        ),
        "pf4-dem-hypergraph-logical-repeat" => Some(
            "contract-only: Stab measures selected flat detectorless logical-only zero-shift hypergraph search repeat folding; broader shifted, nested, and non-flat hypergraph repeat traversal remains capped or excluded, while raw numeric error targets are validation-owned instead of traversal-owned",
        ),
        "pf4-dem-hypergraph-no-target-repeat" => Some(
            "contract-only: Stab measures selected flat no-target zero-shift hypergraph search repeat skipping; broader shifted, nested, non-flat, and mixed-instruction hypergraph repeat traversal remains capped or excluded, while raw numeric error targets are validation-owned instead of traversal-owned",
        ),
        "pf4-dem-search-zero-shift-repeat" => Some(
            "contract-only: Stab measures selected flat zero-detector-shift graphlike and hypergraph search repeat folding; broader nonzero-shift, nested, non-flat, and mixed-instruction repeat traversal remains capped or excluded, while raw numeric and separator-only error target lists are validation-owned instead of traversal-owned",
        ),
        "pf4-dem-search-annotation-repeat" => Some(
            "contract-only: Stab measures selected flat annotation-bearing graphlike and hypergraph search repeat folding; broader nonzero-shift, nested, non-flat, and non-annotation mixed-instruction repeat traversal remains capped or excluded, while raw numeric and separator-only error target lists are validation-owned instead of traversal-owned",
        ),
        "pf4-dem-search-mixed-zero-probability-repeat" => Some(
            "contract-only: Stab measures selected mixed zero-probability plus active zero-shift graphlike and hypergraph search repeat folding; SAT/WCNF, nonzero-shift, non-flat, analyzer, ErrorMatcher, sampler, and broader DEM traversal remain scoped separately",
        ),
        "pf4-dem-search-nested-repeat" => Some(
            "contract-only: Stab measures selected nested zero-detector-shift graphlike and hypergraph search repeat folding; broader nonzero-shift, shifted nested, non-flat, SAT/WCNF, analyzer, ErrorMatcher, and sampler repeat traversal remains capped or excluded, while raw numeric error targets are validation-owned instead of traversal-owned",
        ),
        "pf4-dem-sat-flat-repeat-fold" => Some(
            "contract-only: Stab measures selected SAT/WCNF flat and nested zero-shift repeat folding for unweighted shortest-error SAT including zero-probability structural mechanisms and weighted concrete-MAP SAT; broader shifted, non-flat, and high-index dense-target structural SAT repeat traversal remains capped",
        ),
        "pf4-error-matcher-filter-flat-repeat" => Some(
            "contract-only: Stab measures selected ErrorMatcher filter DEM flat detector-touching zero-shift repeat folding by compact filter-key semantics; broader shifted, mixed-instruction, detectorless logical-only, circuit-repeat provenance, full ErrorMatcher provenance, and explain_errors CLI behavior remains scoped out",
        ),
        "pf4-error-matcher-filter-nested-repeat" => Some(
            "contract-only: Stab measures selected ErrorMatcher filter DEM nested detector-touching zero-shift repeat folding by compact filter-key semantics; broader shifted, detectorless logical-only, circuit-repeat provenance, full ErrorMatcher provenance, and explain_errors CLI behavior remains scoped out",
        ),
        _ => None,
    }
}
