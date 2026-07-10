pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "pf4-dem-flatten-repeat" => Some(
            "contract-only: Stab measures the Rust DetectorErrorModel::flattened public API over repeat, tag, detector-shift, coordinate-shift, separator, and observable cases; pinned Stim exposes equivalent behavior but not a faithful Rust direct baseline",
        ),
        "pf4-dem-rounded" => Some(
            "contract-only: Stab measures the direct compact DetectorErrorModel::rounded transform over top-level and nested error probabilities while preserving repeat structure and non-error coordinate args without auxiliary traversal-tree allocation; pinned Stim exposes equivalent behavior but not a faithful Rust direct baseline",
        ),
        "pf4-dem-coordinate-map" => Some(
            "contract-only: Stab measures the shared folded cursor behind bounded all-detector DEM coordinate maps, selected detector lookup through a huge-repeat model, sparse flat and nested overlapping selected-coordinate lookups, and many-selected flat-overlap lookup; pinned Stim exposes equivalent behavior but not a faithful Rust direct baseline",
        ),
        "pf4-dem-sampler-folded-repeat" => Some(
            "contract-only: Stab measures shared-tree CompiledDemSampler compilation, stochastic direct sample behavior, zero-probability repeat skipping, deterministic zero-shift parity folding, and single, flat, and nested stochastic zero-shift parity folding; sampled-error materialization, replay, and shifted stochastic work caps are source-owned because those outputs or applications are inherently expanded",
        ),
        "pf4-dem-folded-traversal" => Some(
            "contract-only: Stab measures shared folded-visitor hypergraph and SAT collection, zero-probability skipping, detector-touching zero-shift folding, weighted SAT probability folding, and source-owned complexity caps for shifted active mechanisms; analyzer and ErrorMatcher circuit traversal remain separately capped because they do not consume compact DEM input",
        ),
        "pf4-dem-folded-graphlike-traversal" => Some(
            "contract-only: Stab measures shared folded-visitor graphlike collection, bounded shifted traversal, zero-probability skipping, detector-touching and detectorless logical-only zero-shift folding, and no-target repeat skipping; remaining caps protect expanded graph/search complexity instead of compact-input inspection",
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
            "contract-only: Stab measures selected ErrorMatcher filter DEM flat detector-touching zero-shift repeat folding by compact filter-key semantics; detectorless logical-observable-only filter repeats are measured by pf4-error-matcher-filter-logical-repeat, while broader shifted, mixed-instruction, circuit-repeat provenance, full ErrorMatcher provenance, and explain_errors CLI behavior remains scoped out",
        ),
        "pf4-error-matcher-filter-nested-repeat" => Some(
            "contract-only: Stab measures selected ErrorMatcher filter DEM nested detector-touching zero-shift repeat folding by compact filter-key semantics; detectorless logical-observable-only filter repeats are measured by pf4-error-matcher-filter-logical-repeat, while broader shifted, circuit-repeat provenance, full ErrorMatcher provenance, and explain_errors CLI behavior remains scoped out",
        ),
        "pf4-error-matcher-filter-logical-repeat" => Some(
            "contract-only: Stab measures selected ErrorMatcher filter DEM flat and nested detectorless logical-observable-only zero-shift repeat folding by compact filter-key semantics; neutral annotation-only bodies are skipped while shifted active or broader mixed-instruction bodies, circuit-repeat provenance, full ErrorMatcher provenance, and explain_errors CLI behavior remain capped or deferred",
        ),
        "pf4-error-matcher-filter-annotation-repeat" => Some(
            "contract-only: Stab measures selected ErrorMatcher filter DEM flat and nested annotation-bearing zero-shift repeat folding by compact filter-key semantics; neutral annotation-only bodies are skipped while broader active mixed-instruction bodies, shifted repeats, circuit-repeat provenance, full ErrorMatcher provenance, and explain_errors CLI behavior remain capped or deferred",
        ),
        "pfm-b3-dem-traversal-core" => Some(
            "contract-only: Stab measures the shared folded DEM traversal through flat count summaries, nested billion-by-billion represented repeat counts, sparse selected-coordinate lookup, and coordinate-free counting over a deeply nested wide-coordinate model. The row reports compact work separately from represented expanded work, and allocation plus resident-memory evidence is available when compare runs with allocation tracking; pinned Stim has no faithful Rust internal traversal baseline",
        ),
        _ => None,
    }
}
