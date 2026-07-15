pub(super) const PROTOCOL_SMOKE_ITERATIONS: u64 = 1;
pub(super) const PROTOCOL_SMOKE_WORK_ITEMS: u64 = 1;
pub(super) const PROTOCOL_SMOKE_INPUT_DIGEST: &str =
    "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1";
pub(super) const PROTOCOL_SMOKE_OUTPUT_LANES: [u64; 4] = [
    0x656c_7d8a_03ff_449d,
    0x0c24_8bde_f4c3_140b,
    0x0225_2abf_fcd7_61d6,
    0x68e9_bc4c_63e0_059d,
];

pub(super) fn protocol_smoke_output_digest() -> String {
    let [lane_0, lane_1, lane_2, lane_3] = PROTOCOL_SMOKE_OUTPUT_LANES;
    format!("{lane_0:016x}{lane_1:016x}{lane_2:016x}{lane_3:016x}")
}
