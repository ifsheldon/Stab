#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "M4 compatibility tests use direct assertions for compact diagnostics"
)]

use std::collections::BTreeSet;

use stab_core::Gate;

#[test]
fn gate_name_hash_matches_stim_v116_examples_and_canonical_names_do_not_collide() {
    // Adapted from Stim v1.16.0 src/stim/gates/gates.h gate_name_to_hash.
    for (name, expected_hash) in [
        ("H", 169),
        ("CX", 208),
        ("MXX", 5),
        ("DETECTOR", 381),
        ("PAULI_CHANNEL_1", 288),
        ("ELSE_CORRELATED_ERROR", 364),
        ("MPAD", 192),
    ] {
        assert_eq!(Gate::stim_name_hash(name), expected_hash, "{name}");
        assert_eq!(
            Gate::stim_name_hash(&name.to_ascii_lowercase()),
            expected_hash,
            "{name}"
        );
    }

    let hashes = Gate::all()
        .map(|gate| Gate::stim_name_hash(gate.canonical_name()))
        .collect::<BTreeSet<_>>();
    assert_eq!(hashes.len(), Gate::all().count());
}
