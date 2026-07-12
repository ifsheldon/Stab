use sha2::{Digest, Sha256};

use super::{BlockerLedger, OracleEvidenceSignature};

pub(super) fn computed_semantic_digest(ledger: &BlockerLedger) -> sha2::digest::Output<Sha256> {
    let mut hasher = Sha256::new();
    let mut blockers = ledger.blockers.iter().collect::<Vec<_>>();
    blockers.sort_by_key(|blocker| blocker.id.as_str());
    for blocker in blockers {
        digest_field(&mut hasher, "blocker.id", &blocker.id);
        digest_field(&mut hasher, "blocker.milestone", blocker.milestone.as_str());
        digest_field(
            &mut hasher,
            "blocker.disposition",
            blocker.disposition.as_str(),
        );
        digest_field(&mut hasher, "blocker.title", &blocker.title);

        let mut supporting_oracles = blocker.supporting_oracles.iter().collect::<Vec<_>>();
        supporting_oracles.sort_by_key(|reference| reference.value.as_str());
        for reference in supporting_oracles {
            digest_field(
                &mut hasher,
                "supporting_oracle.classification",
                &format!("{:?}", reference.classification),
            );
            digest_field(
                &mut hasher,
                "supporting_oracle.value",
                reference.value.as_str(),
            );
            digest_oracle_signature(
                &mut hasher,
                "supporting_oracle.signature",
                &reference.signature,
            );
        }

        let mut supporting_benchmarks = blocker.supporting_benchmarks.iter().collect::<Vec<_>>();
        supporting_benchmarks.sort_by_key(|reference| reference.value.as_str());
        for reference in supporting_benchmarks {
            digest_field(
                &mut hasher,
                "supporting_benchmark.classification",
                &format!("{:?}", reference.classification),
            );
            digest_field(
                &mut hasher,
                "supporting_benchmark.value",
                reference.value.as_str(),
            );
        }

        let mut cases = blocker.cases.iter().collect::<Vec<_>>();
        cases.sort_by_key(|case| case.id.as_str());
        for case in cases {
            digest_field(&mut hasher, "case.id", &case.id);
            digest_field(&mut hasher, "case.surface", &case.surface);
            let mut gate_surfaces = case.gate_surfaces.clone();
            gate_surfaces.sort();
            for surface in gate_surfaces {
                digest_field(&mut hasher, "case.gate_surface", surface.as_str());
            }
            let mut gate_families = case.gate_families.clone();
            gate_families.sort();
            for family in gate_families {
                digest_field(&mut hasher, "case.gate_family", family.as_str());
            }
            digest_field(
                &mut hasher,
                "case.upstream.path",
                &case.upstream.path.0.to_string_lossy(),
            );
            digest_field(
                &mut hasher,
                "case.upstream.kind",
                &format!("{:?}", case.upstream.kind),
            );
            digest_field(&mut hasher, "case.upstream.test", &case.upstream.test);
            digest_field(&mut hasher, "case.upstream.subcase", &case.upstream.subcase);
            let mut gate_markers = case.upstream.gate_markers.iter().collect::<Vec<_>>();
            gate_markers.sort();
            for marker in gate_markers {
                digest_field(&mut hasher, "case.upstream.gate_marker", marker.as_str());
            }
            for anchor in &case.upstream.anchors {
                digest_field(&mut hasher, "case.upstream.anchor", anchor);
            }
            digest_field(
                &mut hasher,
                "case.comparator",
                &format!("{:?}", case.comparator),
            );
            match &case.statistical_plan {
                Some(plan) => {
                    digest_field(
                        &mut hasher,
                        "case.statistical_plan.shots",
                        &plan.shots.to_string(),
                    );
                    digest_field(
                        &mut hasher,
                        "case.statistical_plan.seed",
                        &plan.seed.to_string(),
                    );
                    digest_field(
                        &mut hasher,
                        "case.statistical_plan.sigma_multiplier",
                        &plan.sigma_multiplier.to_string(),
                    );
                    digest_field(
                        &mut hasher,
                        "case.statistical_plan.absolute_probability_floor",
                        &plan.absolute_probability_floor.to_string(),
                    );
                    digest_field(
                        &mut hasher,
                        "case.statistical_plan.familywise_false_positive_budget",
                        &plan.familywise_false_positive_budget.to_string(),
                    );
                    for bucket in &plan.buckets {
                        digest_field(
                            &mut hasher,
                            "case.statistical_plan.bucket.name",
                            &bucket.name,
                        );
                        digest_field(
                            &mut hasher,
                            "case.statistical_plan.bucket.expected_probability",
                            &bucket.expected_probability.to_string(),
                        );
                    }
                }
                None => digest_field(&mut hasher, "case.statistical_plan", "none"),
            }
            digest_field(&mut hasher, "case.status", case.status.as_str());
            digest_field(
                &mut hasher,
                "case.test.state",
                &format!("{:?}", case.test.state),
            );
            for part in &case.test.selector {
                digest_field(&mut hasher, "case.test.selector", part);
            }
            digest_field(
                &mut hasher,
                "case.oracle.state",
                &format!("{:?}", case.oracle.state),
            );
            digest_field(
                &mut hasher,
                "case.oracle.classification",
                &format!("{:?}", case.oracle.classification),
            );
            digest_field(&mut hasher, "case.oracle.value", case.oracle.value.as_str());
            match &case.oracle.signature {
                Some(signature) => {
                    digest_oracle_signature(&mut hasher, "case.oracle.signature", signature);
                }
                None => digest_field(&mut hasher, "case.oracle.signature", "none"),
            }
            digest_field(
                &mut hasher,
                "case.benchmark.state",
                &format!("{:?}", case.benchmark.state),
            );
            digest_field(&mut hasher, "case.benchmark.value", &case.benchmark.value);
            digest_field(
                &mut hasher,
                "case.benchmark.classification",
                &format!("{:?}", case.benchmark.classification),
            );
            digest_field(
                &mut hasher,
                "case.resource_contract",
                &case.resource_contract,
            );
        }
    }
    hasher.finalize()
}

fn digest_oracle_signature(hasher: &mut Sha256, label: &str, signature: &OracleEvidenceSignature) {
    digest_field(
        hasher,
        &format!("{label}.parity_mode"),
        &format!("{:?}", signature.parity_mode),
    );
    digest_field(
        hasher,
        &format!("{label}.comparator"),
        &format!("{:?}", signature.comparator),
    );
    digest_field(hasher, &format!("{label}.argv"), &signature.argv);
    digest_field(
        hasher,
        &format!("{label}.upstream_source"),
        &signature.upstream_source.0.to_string_lossy(),
    );
    digest_optional_path(
        hasher,
        &format!("{label}.stdin_path"),
        signature.stdin_path.as_ref(),
    );
    digest_optional_path(
        hasher,
        &format!("{label}.expected_stdout_path"),
        signature.expected_stdout_path.as_ref(),
    );
    digest_field(
        hasher,
        &format!("{label}.stdin_sha256"),
        signature
            .stdin_sha256
            .as_ref()
            .map_or("none", |digest| digest.0.as_str()),
    );
    digest_field(
        hasher,
        &format!("{label}.expected_stdout_sha256"),
        signature
            .expected_stdout_sha256
            .as_ref()
            .map_or("none", |digest| digest.0.as_str()),
    );
}

fn digest_optional_path(
    hasher: &mut Sha256,
    label: &str,
    path: Option<&super::FixtureRelativeEvidencePath>,
) {
    let value = path.map_or_else(|| "none".into(), |path| path.0.to_string_lossy());
    digest_field(hasher, label, &value);
}

pub(super) fn digest_hex(digest: &[u8]) -> String {
    let mut result = String::with_capacity(digest.len() * 2);
    for byte in digest {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}

fn digest_field(hasher: &mut Sha256, label: &str, value: &str) {
    for bytes in [label.as_bytes(), value.as_bytes()] {
        hasher.update(bytes.len().to_string().as_bytes());
        hasher.update(b":");
        hasher.update(bytes);
    }
}
