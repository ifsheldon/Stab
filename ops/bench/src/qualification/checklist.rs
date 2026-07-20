use std::collections::BTreeSet;

use serde::Deserialize;

use super::discovery::{PERFORMANCE_FEATURE_IDS, sha256_hex};
use super::model::{ChecklistChildOwnership, ChecklistScope};
use crate::error::BenchError;

pub(super) struct RawChecklistItem {
    pub(super) id: String,
    pub(super) source_line: u32,
    pub(super) anchor_digest: String,
    pub(super) section: String,
    pub(super) feature: String,
    pub(super) raw_status: String,
    pub(super) scope: ChecklistScope,
    pub(super) deferred_remainder: bool,
    pub(super) selected_child: Option<String>,
    pub(super) deferred_child: Option<String>,
    pub(super) selected_child_ids: Vec<String>,
    pub(super) deferred_child_ids: Vec<String>,
    pub(super) selected_child_ownership: Vec<ChecklistChildOwnership>,
    pub(super) performance_features: Vec<String>,
}

const INVENTORY_COUNTS_PREFIX: &str = "<!-- qualification-inventory-counts ";
const INVENTORY_COUNTS_SUFFIX: &str = " -->";
const INVENTORY_COUNTS_SUMMARY_PREFIX: &str = "Qualification inventory counts: ";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct AdvertisedInventoryCounts {
    public_api_items: usize,
    algebra_api_items: usize,
}

impl AdvertisedInventoryCounts {
    pub(super) fn validate(
        self,
        public_api_items: usize,
        algebra_api_items: usize,
    ) -> Result<(), BenchError> {
        if self.public_api_items == public_api_items && self.algebra_api_items == algebra_api_items
        {
            Ok(())
        } else {
            Err(BenchError::Qualification(format!(
                "feature-checklist qualification counts are stale: advertised public_api_items={} algebra_api_items={}, discovered public_api_items={public_api_items} algebra_api_items={algebra_api_items}",
                self.public_api_items, self.algebra_api_items
            )))
        }
    }

    pub(super) fn validate_rendered_summary(self, source: &str) -> Result<(), BenchError> {
        let expected = format!(
            "{INVENTORY_COUNTS_SUMMARY_PREFIX}**{}** default-feature public Rust API items and **{}** Algebra API items.",
            format_count(self.public_api_items),
            format_count(self.algebra_api_items),
        );
        let rendered = source
            .lines()
            .filter(|line| line.starts_with(INVENTORY_COUNTS_SUMMARY_PREFIX))
            .collect::<Vec<_>>();
        if rendered == [expected.as_str()] {
            Ok(())
        } else {
            Err(BenchError::Qualification(format!(
                "feature-checklist rendered qualification counts are stale or ambiguous: expected {expected:?}, found {rendered:?}"
            )))
        }
    }
}

fn format_count(value: usize) -> String {
    let digits = value.to_string();
    let mut rendered = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            rendered.push(',');
        }
        rendered.push(digit);
    }
    rendered
}

pub(super) fn parse_inventory_counts(
    source: &str,
) -> Result<AdvertisedInventoryCounts, BenchError> {
    let mut values = source.lines().filter_map(|line| {
        line.strip_prefix(INVENTORY_COUNTS_PREFIX)
            .and_then(|value| value.strip_suffix(INVENTORY_COUNTS_SUFFIX))
    });
    let value = values.next().ok_or_else(|| {
        BenchError::Qualification(
            "feature checklist is missing qualification-inventory-counts metadata".to_string(),
        )
    })?;
    if values.next().is_some() {
        return Err(BenchError::Qualification(
            "feature checklist has duplicate qualification-inventory-counts metadata".to_string(),
        ));
    }
    serde_json::from_str(value).map_err(|source| {
        BenchError::Qualification(format!(
            "feature-checklist qualification-inventory-counts metadata is invalid: {source}"
        ))
    })
}

pub(super) fn parse(source: &str) -> Result<Vec<RawChecklistItem>, BenchError> {
    let mut section = String::new();
    let mut items = Vec::new();
    for (line_index, line) in source.lines().enumerate() {
        if let Some(heading) = line.strip_prefix("## ") {
            section = heading.to_string();
            continue;
        }
        if !line.starts_with('|') {
            continue;
        }
        let cells = line
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        let Some(feature) = cells.first().copied() else {
            continue;
        };
        let Some(status) = cells.get(1).copied() else {
            continue;
        };
        if matches!(status, "Stab status" | "Status")
            || feature.starts_with("---")
            || status.starts_with("---")
        {
            continue;
        }
        if section.is_empty() || cells.len() < 3 {
            return Err(BenchError::Qualification(format!(
                "malformed feature checklist row {line:?}"
            )));
        }
        let scope = if status.starts_with("Deferred") {
            ChecklistScope::Deferred
        } else if status.starts_with("Done") || status.starts_with("Partial") {
            ChecklistScope::Selected
        } else {
            return Err(BenchError::Qualification(format!(
                "unknown checklist status {status:?} for {feature:?}"
            )));
        };
        let performance_features = classify(&section, feature, scope);
        let id = format!("PERFC-{}", stable_suffix(&format!("{section}\0{feature}")));
        let (selected_child, deferred_child, selected_child_ids, deferred_child_ids) =
            children(feature, status, &id)?;
        let selected_child_ownership =
            child_ownership(feature, status, &selected_child_ids, &performance_features)?;
        items.push(RawChecklistItem {
            id,
            source_line: u32::try_from(line_index + 1).map_err(|_| {
                BenchError::Qualification("feature-checklist line number overflow".to_string())
            })?,
            anchor_digest: sha256_hex(line.as_bytes()),
            section: section.clone(),
            feature: feature.to_string(),
            raw_status: status.to_string(),
            scope,
            deferred_remainder: status.starts_with("Partial"),
            selected_child,
            deferred_child,
            selected_child_ids,
            deferred_child_ids,
            selected_child_ownership,
            performance_features,
        });
    }
    Ok(items)
}

type ChecklistChildren = (Option<String>, Option<String>, Vec<String>, Vec<String>);

fn children(feature: &str, status: &str, row_id: &str) -> Result<ChecklistChildren, BenchError> {
    if status.starts_with("Done") {
        return Ok((
            Some(
                "The implemented row exactly as bounded by its checklist evidence column."
                    .to_string(),
            ),
            None,
            vec![format!("{row_id}-IMPLEMENTED")],
            Vec::new(),
        ));
    }
    if status.starts_with("Deferred") {
        return Ok((
            None,
            Some(feature.to_string()),
            Vec::new(),
            vec![format!("{row_id}-DEFERRED")],
        ));
    }
    let (selected, deferred, selected_ids, deferred_ids): (&str, &str, &[&str], &[&str]) =
        match feature {
            value if value.contains(".stim") && value.contains("result-format compatibility") => (
                "Implemented format, gate, DEM, analyzer, sampler, search, SAT/WCNF, and filter-key surfaces named by the checklist.",
                "Named deferred command, Python, diagram, and provenance products.",
                &[
                    "STIM-FORMAT-SELECTED",
                    "DEM-SELECTED-RUST-SURFACE",
                    "RESULT-FORMATS-01-B8-R8-HITS-DETS-PTB64",
                    "PFM-B2-GATE-MATRIX",
                    "PFM-B3-DEM-TRAVERSAL",
                    "PFM-B4-FLOW-SOLVE",
                    "PFM-B5-ANALYSIS-SEARCH",
                ],
                &[
                    "DEFERRED-PYTHON-API",
                    "DEFERRED-DIAGRAM-PRODUCT",
                    "DEFERRED-EXPLAIN-ERRORS-PROVENANCE",
                    "DEFERRED-NAMED-COMMAND-PRODUCTS",
                ],
            ),
            "Target kinds" => (
                "Parsing and the exact implemented sweep, feedback, accepted-order, and rejection matrices named by the checklist.",
                "Remaining sweep shapes and typed detector-sampler sweep APIs.",
                &[
                    "TARGET-QUBIT",
                    "TARGET-INVERTED-QUBIT",
                    "TARGET-MEASUREMENT-RECORD",
                    "TARGET-SWEEP-BIT",
                    "TARGET-PAULI",
                    "TARGET-INVERTED-PAULI",
                    "TARGET-COMBINER",
                    "PFM3-SAMPLER-SWEEP-ORDER-MATRIX",
                    "PFM3-DETECT-FEEDBACK-MATRIX",
                    "PFM3-ANALYZER-SWEEP-TARGET-KIND-MATRIX",
                ],
                &[
                    "DEFERRED-PYTHON-DETECTOR-SAMPLER-SWEEP",
                    "DEFERRED-UNSELECTED-SWEEP-TARGET-SHAPES",
                ],
            ),
            "Full semantic execution of every legal circuit operation" => (
                "The exact selected gate matrix and engine cells owned by sampler, detection, analyzer, generation, and algebra milestones.",
                "The complement of legal circuit-operation cells not selected by those milestones.",
                &[
                    "PFM-B2-GATE-SURFACE-37-CASES",
                    "ENGINE-MEASUREMENT-SAMPLER-SELECTED",
                    "ENGINE-DETECTION-CONVERTER-SELECTED",
                    "ENGINE-DETECTOR-FRAME-SELECTED",
                    "ENGINE-ERROR-ANALYZER-SELECTED",
                    "ENGINE-FLOW-GENERATOR-SELECTED",
                    "ENGINE-STABILIZER-ALGEBRA-SELECTED",
                ],
                &[
                    "DEFERRED-INTERACTIVE-TABLEAU-SIMULATOR",
                    "DEFERRED-FLIP-SIMULATOR",
                    "DEFERRED-GATE-ENGINE-CELLS-OUTSIDE-PFM-B2",
                ],
            ),
            "Repeat handling" => (
                "Repeat parsing, printing, sampling, analysis, conversion, listed transforms, caps, and existing folded traversals.",
                "Fully folded traversal for remaining transforms.",
                &[
                    "REPEAT-PARSE-PRINT",
                    "REPEAT-SAMPLING",
                    "REPEAT-ANALYSIS",
                    "REPEAT-DETECTION-CONVERSION",
                    "REPEAT-FLATTENED",
                    "REPEAT-WITHOUT-NOISE",
                    "REPEAT-FEEDBACK-INLINE-SELECTED",
                    "REPEAT-TIME-REVERSE-SELECTED",
                ],
                &["DEFERRED-FOLDED-TRAVERSAL-REMAINING-TRANSFORMS"],
            ),
            "Measurement-to-detection conversion" => (
                "Existing compiled, free, streaming, CLI, sweep, and scoped-feedback conversion contracts.",
                "Broader sweep, transform, and repeat-feedback behavior outside the selected cases.",
                &[
                    "M2D-COMPILED",
                    "M2D-FREE-FUNCTION",
                    "M2D-STREAMING",
                    "M2D-CLI",
                    "M2D-SWEEP-SELECTED",
                    "M2D-FEEDBACK-SELECTED",
                    "M2D-RAN-WITHOUT-FEEDBACK-SELECTED",
                ],
                &[
                    "DEFERRED-M2D-BROADER-SWEEP",
                    "DEFERRED-M2D-BROADER-TRANSFORM",
                    "DEFERRED-M2D-BROADER-REPEAT-FEEDBACK",
                ],
            ),
            "Broader sweep-conditioned simulator and analysis parity" => (
                "The exact current m2d, detect, sampler-order, and analyzer sweep subsets named by the checklist.",
                "Python detector-sampler sweep APIs and every remaining sweep target shape.",
                &[
                    "SWEEP-M2D-FORMATS-01-B8-R8-HITS-DETS-PTB64",
                    "SWEEP-DETECT-DEFAULT-FALSE",
                    "SWEEP-DETECT-FRAME-SELECTED",
                    "SWEEP-SAMPLER-TARGET-ORDER-MATRIX",
                    "PFM3-ANALYZER-SWEEP-CONDITIONED-MATRIX",
                ],
                &[
                    "DEFERRED-PYTHON-DETECTOR-SAMPLER-SWEEP",
                    "DEFERRED-REMAINING-SWEEP-TARGET-SHAPES",
                ],
            ),
            "Full feedback-inlining transform parity" => (
                "Listed Pauli/MPP, XCZ/YCZ, bounded-loop, and nested-repeat feedback cases.",
                "Remaining repeat-block feedback behavior.",
                &[
                    "FEEDBACK-PAULI-MPP-SELECTED",
                    "FEEDBACK-XCZ-YCZ-SELECTED",
                    "FEEDBACK-DEMOLITION-PINNED",
                    "FEEDBACK-INTERLEAVED-ORDER-PINNED",
                    "FEEDBACK-BOUNDED-LOOP-REFOLD",
                    "FEEDBACK-NESTED-REPEAT-DETECTOR-PARITY",
                ],
                &["DEFERRED-REMAINING-REPEAT-BLOCK-FEEDBACK"],
            ),
            _ => {
                return Err(BenchError::Qualification(format!(
                    "partial checklist row {feature:?} has no explicit selected/deferred split"
                )));
            }
        };
    Ok((
        Some(selected.to_string()),
        Some(deferred.to_string()),
        selected_ids.iter().map(|id| (*id).to_string()).collect(),
        deferred_ids.iter().map(|id| (*id).to_string()).collect(),
    ))
}

fn child_ownership(
    feature: &str,
    status: &str,
    selected_child_ids: &[String],
    performance_features: &[String],
) -> Result<Vec<ChecklistChildOwnership>, BenchError> {
    if status.starts_with("Deferred") {
        return Ok(Vec::new());
    }
    if status.starts_with("Done") {
        if performance_features.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(selected_child_ids
            .iter()
            .map(|child_id| ChecklistChildOwnership {
                child_id: child_id.clone(),
                performance_features: performance_features.to_vec(),
            })
            .collect());
    }
    let specs = partial_child_domains(feature).ok_or_else(|| {
        BenchError::Qualification(format!(
            "partial checklist row {feature:?} has no child-to-domain ownership"
        ))
    })?;
    let expected_children = selected_child_ids
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let declared_children = specs
        .iter()
        .map(|(child, _)| *child)
        .collect::<BTreeSet<_>>();
    if expected_children != declared_children || declared_children.len() != specs.len() {
        return Err(BenchError::Qualification(format!(
            "partial checklist row {feature:?} child ownership does not match its selected ids"
        )));
    }
    let expected_features = performance_features
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let declared_features = specs
        .iter()
        .flat_map(|(_, features)| features.iter().copied())
        .collect::<BTreeSet<_>>();
    if expected_features != declared_features {
        return Err(BenchError::Qualification(format!(
            "partial checklist row {feature:?} child ownership does not exactly cover its performance domains"
        )));
    }
    selected_child_ids
        .iter()
        .map(|child_id| {
            let domains = specs
                .iter()
                .find_map(|(candidate, domains)| (*candidate == child_id).then_some(*domains))
                .ok_or_else(|| {
                    BenchError::Qualification(format!(
                        "partial checklist row {feature:?} has no ownership for {child_id}"
                    ))
                })?;
            Ok(ChecklistChildOwnership {
                child_id: child_id.clone(),
                performance_features: domains.iter().map(|domain| (*domain).to_string()).collect(),
            })
        })
        .collect::<Result<Vec<_>, BenchError>>()
}

type ChildDomainSpec = (&'static str, &'static [&'static str]);

fn partial_child_domains(feature: &str) -> Option<Vec<ChildDomainSpec>> {
    let specs = if feature.contains(".stim") && feature.contains("result-format compatibility") {
        vec![
            ("STIM-FORMAT-SELECTED", &["PERF-CIRCUIT-MODEL"] as &[_]),
            ("DEM-SELECTED-RUST-SURFACE", &["PERF-DEM-MODEL"] as &[_]),
            (
                "RESULT-FORMATS-01-B8-R8-HITS-DETS-PTB64",
                &["PERF-CONVERT-CLI", "PERF-RESULT-IO"] as &[_],
            ),
            (
                "PFM-B2-GATE-MATRIX",
                &[
                    "PERF-DETECTION",
                    "PERF-GATE-CONTRACT",
                    "PERF-SAMPLING",
                    "PERF-STABILIZER-ALGEBRA",
                ] as &[_],
            ),
            (
                "PFM-B3-DEM-TRAVERSAL",
                &["PERF-DEM-MODEL", "PERF-DEM-SAMPLING"] as &[_],
            ),
            (
                "PFM-B4-FLOW-SOLVE",
                &["PERF-FLOWS-AND-DETECTOR-UTILITIES", "PERF-GENERATION"] as &[_],
            ),
            (
                "PFM-B5-ANALYSIS-SEARCH",
                &["PERF-ERROR-ANALYSIS", "PERF-SEARCH-AND-MATCHING"] as &[_],
            ),
        ]
    } else {
        match feature {
            "Target kinds" => vec![
                ("TARGET-QUBIT", &["PERF-GATE-CONTRACT"] as &[_]),
                ("TARGET-INVERTED-QUBIT", &["PERF-GATE-CONTRACT"] as &[_]),
                (
                    "TARGET-MEASUREMENT-RECORD",
                    &[
                        "PERF-DETECTION",
                        "PERF-ERROR-ANALYSIS",
                        "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                        "PERF-GATE-CONTRACT",
                        "PERF-SAMPLING",
                    ] as &[_],
                ),
                (
                    "TARGET-SWEEP-BIT",
                    &[
                        "PERF-DETECTION",
                        "PERF-ERROR-ANALYSIS",
                        "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                        "PERF-GATE-CONTRACT",
                        "PERF-SAMPLING",
                    ] as &[_],
                ),
                ("TARGET-PAULI", &["PERF-GATE-CONTRACT"] as &[_]),
                ("TARGET-INVERTED-PAULI", &["PERF-GATE-CONTRACT"] as &[_]),
                ("TARGET-COMBINER", &["PERF-GATE-CONTRACT"] as &[_]),
                (
                    "PFM3-SAMPLER-SWEEP-ORDER-MATRIX",
                    &["PERF-SAMPLING"] as &[_],
                ),
                (
                    "PFM3-DETECT-FEEDBACK-MATRIX",
                    &["PERF-DETECTION", "PERF-FLOWS-AND-DETECTOR-UTILITIES"] as &[_],
                ),
                (
                    "PFM3-ANALYZER-SWEEP-TARGET-KIND-MATRIX",
                    &["PERF-ERROR-ANALYSIS"] as &[_],
                ),
            ],
            "Full semantic execution of every legal circuit operation" => vec![
                (
                    "PFM-B2-GATE-SURFACE-37-CASES",
                    &["PERF-CIRCUIT-MODEL", "PERF-GATE-CONTRACT"] as &[_],
                ),
                (
                    "ENGINE-MEASUREMENT-SAMPLER-SELECTED",
                    &["PERF-SAMPLING"] as &[_],
                ),
                (
                    "ENGINE-DETECTION-CONVERTER-SELECTED",
                    &["PERF-DETECTION"] as &[_],
                ),
                (
                    "ENGINE-DETECTOR-FRAME-SELECTED",
                    &["PERF-DETECTION"] as &[_],
                ),
                (
                    "ENGINE-ERROR-ANALYZER-SELECTED",
                    &["PERF-ERROR-ANALYSIS"] as &[_],
                ),
                (
                    "ENGINE-FLOW-GENERATOR-SELECTED",
                    &["PERF-FLOWS-AND-DETECTOR-UTILITIES", "PERF-GENERATION"] as &[_],
                ),
                (
                    "ENGINE-STABILIZER-ALGEBRA-SELECTED",
                    &["PERF-STABILIZER-ALGEBRA"] as &[_],
                ),
            ],
            "Repeat handling" => vec![
                (
                    "REPEAT-PARSE-PRINT",
                    &["PERF-CIRCUIT-MODEL", "PERF-DEM-MODEL"] as &[_],
                ),
                (
                    "REPEAT-SAMPLING",
                    &["PERF-DEM-SAMPLING", "PERF-SAMPLING"] as &[_],
                ),
                ("REPEAT-ANALYSIS", &["PERF-ERROR-ANALYSIS"] as &[_]),
                ("REPEAT-DETECTION-CONVERSION", &["PERF-DETECTION"] as &[_]),
                (
                    "REPEAT-FLATTENED",
                    &["PERF-CIRCUIT-MODEL", "PERF-DEM-MODEL"] as &[_],
                ),
                ("REPEAT-WITHOUT-NOISE", &["PERF-CIRCUIT-MODEL"] as &[_]),
                (
                    "REPEAT-FEEDBACK-INLINE-SELECTED",
                    &["PERF-DETECTION", "PERF-FLOWS-AND-DETECTOR-UTILITIES"] as &[_],
                ),
                (
                    "REPEAT-TIME-REVERSE-SELECTED",
                    &["PERF-FLOWS-AND-DETECTOR-UTILITIES"] as &[_],
                ),
            ],
            "Measurement-to-detection conversion" => vec![
                ("M2D-COMPILED", &["PERF-DETECTION"] as &[_]),
                ("M2D-FREE-FUNCTION", &["PERF-DETECTION"] as &[_]),
                (
                    "M2D-STREAMING",
                    &["PERF-DETECTION", "PERF-RESULT-IO"] as &[_],
                ),
                ("M2D-CLI", &["PERF-DETECTION", "PERF-RESULT-IO"] as &[_]),
                ("M2D-SWEEP-SELECTED", &["PERF-DETECTION"] as &[_]),
                (
                    "M2D-FEEDBACK-SELECTED",
                    &["PERF-DETECTION", "PERF-FLOWS-AND-DETECTOR-UTILITIES"] as &[_],
                ),
                (
                    "M2D-RAN-WITHOUT-FEEDBACK-SELECTED",
                    &["PERF-DETECTION", "PERF-FLOWS-AND-DETECTOR-UTILITIES"] as &[_],
                ),
            ],
            "Broader sweep-conditioned simulator and analysis parity" => vec![
                (
                    "SWEEP-M2D-FORMATS-01-B8-R8-HITS-DETS-PTB64",
                    &[
                        "PERF-DETECTION",
                        "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                        "PERF-GATE-CONTRACT",
                    ] as &[_],
                ),
                ("SWEEP-DETECT-DEFAULT-FALSE", &["PERF-DETECTION"] as &[_]),
                ("SWEEP-DETECT-FRAME-SELECTED", &["PERF-DETECTION"] as &[_]),
                (
                    "SWEEP-SAMPLER-TARGET-ORDER-MATRIX",
                    &["PERF-GATE-CONTRACT", "PERF-SAMPLING"] as &[_],
                ),
                (
                    "PFM3-ANALYZER-SWEEP-CONDITIONED-MATRIX",
                    &["PERF-ERROR-ANALYSIS", "PERF-GATE-CONTRACT"] as &[_],
                ),
            ],
            "Full feedback-inlining transform parity" => vec![
                (
                    "FEEDBACK-PAULI-MPP-SELECTED",
                    &[
                        "PERF-CIRCUIT-MODEL",
                        "PERF-ERROR-ANALYSIS",
                        "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                    ] as &[_],
                ),
                (
                    "FEEDBACK-XCZ-YCZ-SELECTED",
                    &[
                        "PERF-CIRCUIT-MODEL",
                        "PERF-ERROR-ANALYSIS",
                        "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                    ] as &[_],
                ),
                (
                    "FEEDBACK-DEMOLITION-PINNED",
                    &[
                        "PERF-CIRCUIT-MODEL",
                        "PERF-ERROR-ANALYSIS",
                        "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                    ] as &[_],
                ),
                (
                    "FEEDBACK-INTERLEAVED-ORDER-PINNED",
                    &[
                        "PERF-CIRCUIT-MODEL",
                        "PERF-ERROR-ANALYSIS",
                        "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                    ] as &[_],
                ),
                (
                    "FEEDBACK-BOUNDED-LOOP-REFOLD",
                    &[
                        "PERF-CIRCUIT-MODEL",
                        "PERF-ERROR-ANALYSIS",
                        "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                    ] as &[_],
                ),
                (
                    "FEEDBACK-NESTED-REPEAT-DETECTOR-PARITY",
                    &[
                        "PERF-CIRCUIT-MODEL",
                        "PERF-DETECTION",
                        "PERF-ERROR-ANALYSIS",
                        "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                    ] as &[_],
                ),
            ],
            _ => return None,
        }
    };
    Some(specs)
}

fn classify(section: &str, feature: &str, scope: ChecklistScope) -> Vec<String> {
    if scope == ChecklistScope::Deferred {
        return Vec::new();
    }
    let text = format!("{} {}", section, feature).to_ascii_lowercase();
    let mut features = Vec::new();
    let mut add = |value: &str| {
        if !features.iter().any(|feature| feature == value) {
            features.push(value.to_string());
        }
    };
    let exact_domains: Option<&[&str]> =
        if feature.contains(".stim") && feature.contains("result-format compatibility") {
            Some(&[
                "PERF-CIRCUIT-MODEL",
                "PERF-DEM-MODEL",
                "PERF-RESULT-IO",
                "PERF-GATE-CONTRACT",
                "PERF-STABILIZER-ALGEBRA",
                "PERF-GENERATION",
                "PERF-CONVERT-CLI",
                "PERF-SAMPLING",
                "PERF-DETECTION",
                "PERF-DEM-SAMPLING",
                "PERF-ERROR-ANALYSIS",
                "PERF-SEARCH-AND-MATCHING",
                "PERF-FLOWS-AND-DETECTOR-UTILITIES",
            ])
        } else {
            match feature {
                "Target kinds" => Some(&[
                    "PERF-GATE-CONTRACT",
                    "PERF-SAMPLING",
                    "PERF-DETECTION",
                    "PERF-ERROR-ANALYSIS",
                    "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                ]),
                "Full semantic execution of every legal circuit operation" => Some(&[
                    "PERF-CIRCUIT-MODEL",
                    "PERF-GATE-CONTRACT",
                    "PERF-STABILIZER-ALGEBRA",
                    "PERF-GENERATION",
                    "PERF-SAMPLING",
                    "PERF-DETECTION",
                    "PERF-ERROR-ANALYSIS",
                    "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                ]),
                "Repeat handling" => Some(&[
                    "PERF-CIRCUIT-MODEL",
                    "PERF-DEM-MODEL",
                    "PERF-SAMPLING",
                    "PERF-DETECTION",
                    "PERF-DEM-SAMPLING",
                    "PERF-ERROR-ANALYSIS",
                    "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                ]),
                "Measurement-to-detection conversion" => Some(&[
                    "PERF-RESULT-IO",
                    "PERF-DETECTION",
                    "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                ]),
                "Broader sweep-conditioned simulator and analysis parity" => Some(&[
                    "PERF-GATE-CONTRACT",
                    "PERF-SAMPLING",
                    "PERF-DETECTION",
                    "PERF-ERROR-ANALYSIS",
                    "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                ]),
                "Full feedback-inlining transform parity" => Some(&[
                    "PERF-CIRCUIT-MODEL",
                    "PERF-DETECTION",
                    "PERF-ERROR-ANALYSIS",
                    "PERF-FLOWS-AND-DETECTOR-UTILITIES",
                ]),
                _ => None,
            }
        };
    if let Some(domains) = exact_domains {
        for domain in domains {
            add(domain);
        }
        features.sort();
        return features;
    }
    if section.starts_with("1.") && feature.contains("Rust core") {
        for feature_id in PERFORMANCE_FEATURE_IDS {
            if feature_id != "PERF-CLI-STARTUP-AND-ERRORS" {
                add(feature_id);
            }
        }
    } else if section.starts_with("15.") || section.starts_with("16.") {
        return Vec::new();
    } else if section.starts_with("2.") {
        if text.contains("dem ") {
            add("PERF-DEM-MODEL");
        } else if text.contains("01`")
            || text.contains("b8`")
            || text.contains("r8`")
            || text.contains("hits`")
            || text.contains("dets`")
            || text.contains("ptb64`")
            || text.contains("format conversion")
            || text.contains("streaming io")
        {
            add("PERF-RESULT-IO");
            if text.contains("conversion") {
                add("PERF-CONVERT-CLI");
            }
        } else {
            add("PERF-CIRCUIT-MODEL");
        }
    } else if section.starts_with("3.") {
        add("PERF-GATE-CONTRACT");
    } else if section.starts_with("4.") {
        if text.contains("generation") {
            add("PERF-GENERATION");
        } else if text.contains("reference sample") {
            add("PERF-SAMPLING");
        } else if text.contains("transform") {
            add("PERF-FLOWS-AND-DETECTOR-UTILITIES");
        } else {
            add("PERF-CIRCUIT-MODEL");
        }
    } else if section.starts_with("5.") {
        if text.contains("sampling") {
            add("PERF-DEM-SAMPLING");
        } else if text.contains("search") || text.contains("sat") || text.contains("wcnf") {
            add("PERF-SEARCH-AND-MATCHING");
        } else {
            add("PERF-DEM-MODEL");
        }
    } else if section.starts_with("6.") {
        if text.contains("dem sampling") {
            add("PERF-DEM-SAMPLING");
        } else if text.contains("detection") || text.contains("detector") || text.contains("m2d") {
            add("PERF-DETECTION");
        } else {
            add("PERF-SAMPLING");
        }
    } else if section.starts_with("7.") {
        if text.contains("search")
            || text.contains("logical error")
            || text.contains("sat")
            || text.contains("wcnf")
        {
            add("PERF-SEARCH-AND-MATCHING");
        } else {
            add("PERF-ERROR-ANALYSIS");
        }
    } else if section.starts_with("8.") {
        if text.contains("flow") {
            add("PERF-FLOWS-AND-DETECTOR-UTILITIES");
        } else {
            add("PERF-STABILIZER-ALGEBRA");
        }
    } else if section.starts_with("10.") {
        add("PERF-GENERATION");
    } else if section.starts_with("11.") {
        add(classify_cli(feature));
        add("PERF-CLI-STARTUP-AND-ERRORS");
    } else if section.starts_with("17.") {
        if text.contains("dem public") {
            add("PERF-DEM-MODEL");
        } else if text.contains("sweep") {
            add("PERF-DETECTION");
        } else if text.contains("feedback") || text.contains("transform") {
            add("PERF-FLOWS-AND-DETECTOR-UTILITIES");
        }
    } else if section.starts_with("1.") {
        if text.contains("cli") {
            add("PERF-CLI-STARTUP-AND-ERRORS");
        } else {
            add("PERF-CIRCUIT-MODEL");
            add("PERF-DEM-MODEL");
            add("PERF-RESULT-IO");
        }
    }
    features.sort();
    features
}

fn classify_cli(feature: &str) -> &'static str {
    if feature.contains("gen`") {
        "PERF-GENERATION"
    } else if feature.contains("convert`") {
        "PERF-CONVERT-CLI"
    } else if feature.contains("sample_dem`") {
        "PERF-DEM-SAMPLING"
    } else if feature.contains("sample`") {
        "PERF-SAMPLING"
    } else if feature.contains("detect`") || feature.contains("m2d`") {
        "PERF-DETECTION"
    } else if feature.contains("analyze_errors`") {
        "PERF-ERROR-ANALYSIS"
    } else {
        "PERF-CLI-STARTUP-AND-ERRORS"
    }
}

fn stable_suffix(value: &str) -> String {
    sha256_hex(value.as_bytes())
        .get(..16)
        .unwrap_or("invalid-digest")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const COUNTS: &str = concat!(
        "<!-- qualification-inventory-counts ",
        r#"{"public_api_items":1974,"algebra_api_items":656}"#,
        " -->\n",
        "Qualification inventory counts: **1,974** default-feature public Rust API items and **656** Algebra API items."
    );

    #[test]
    fn advertised_inventory_counts_are_exact_and_fail_closed() {
        let counts = parse_inventory_counts(COUNTS).expect("parse counts");
        counts.validate(1_974, 656).expect("matching counts");
        counts
            .validate_rendered_summary(COUNTS)
            .expect("matching rendered counts");
        assert!(counts.validate(1_975, 656).is_err());
        assert!(counts.validate(1_974, 655).is_err());
    }

    #[test]
    fn rendered_inventory_counts_reject_visible_only_drift() {
        let counts = parse_inventory_counts(COUNTS).expect("parse counts");
        let stale = COUNTS.replace("**1,974**", "**1,973**");
        assert!(counts.validate_rendered_summary(&stale).is_err());
        assert!(
            counts
                .validate_rendered_summary(&format!("{COUNTS}\n{COUNTS}"))
                .is_err()
        );
    }

    #[test]
    fn inventory_count_metadata_rejects_missing_duplicate_and_unknown_fields() {
        assert!(parse_inventory_counts("").is_err());
        assert!(parse_inventory_counts(&format!("{COUNTS}\n{COUNTS}")).is_err());
        assert!(
            parse_inventory_counts(
                "<!-- qualification-inventory-counts {\"public_api_items\":1974,\"algebra_api_items\":656,\"extra\":1} -->"
            )
            .is_err()
        );
    }
}
