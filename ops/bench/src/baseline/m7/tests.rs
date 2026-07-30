#![allow(
    clippy::expect_used,
    reason = "focused benchmark witness tests use direct assertions"
)]

use super::{LEGACY_DISPATCH_EXPECTED, ensure_legacy_dispatch_witness};
use crate::baseline::batch_sinks::OutputWitness;

#[test]
fn legacy_dispatch_rejects_same_width_wrong_content() {
    let wrong = vec![0_u8; LEGACY_DISPATCH_EXPECTED.bytes];
    let actual = OutputWitness::from_bytes(&wrong);
    assert_eq!(actual.bytes, LEGACY_DISPATCH_EXPECTED.bytes);
    assert_ne!(actual.digest, LEGACY_DISPATCH_EXPECTED.digest);

    let error = ensure_legacy_dispatch_witness("pf7-cli-legacy-dispatch-startup", actual)
        .expect_err("same-width output with the wrong circuit must be rejected");
    assert!(error.to_string().contains("pinned Stim v1.16.0"));
}
