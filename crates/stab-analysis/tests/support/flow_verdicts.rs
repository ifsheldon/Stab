//! Reads unsigned flow-check verdicts for tests that only need whether each flow holds.

#![allow(
    clippy::expect_used,
    reason = "a flow check that fails is a test failure reported by value"
)]

use stab_algebra::Flow;
use stab_analysis::{FlowVerdict, UnsignedFlowCheckOptions, check_unsigned_stabilizer_flows};
use stab_model::Circuit;

/// Each flow's verdict under the default options: `true` when it holds and `false` when it is
/// violated. An error fails the test.
pub(crate) fn flow_holds(circuit: &Circuit, flows: &[Flow]) -> Vec<bool> {
    check_unsigned_stabilizer_flows(circuit, flows, &UnsignedFlowCheckOptions::default())
        .expect("default options decide every flow")
        .iter()
        .map(FlowVerdict::holds)
        .collect()
}
