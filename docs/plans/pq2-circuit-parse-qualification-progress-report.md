# PQ2 Circuit Parse Qualification Progress Report

## Status

The first PQ2 product group, `PERFQ-M4-CIRCUIT-PARSE`, has complete current-schema correctness-bound PR, full, and soak evidence on the controlled Linux AArch64 host as of 2026-07-15.

The harness and evidence contracts pass, but the product timing target does not. All six promotable full and soak scale measurements fail the unchanged `1.25x` gate. This is an optimization blocker, not a correctness, comparator, provenance, reproducibility, host-policy, noise, or report-replay blocker.

This report closes only the first proving group. It does not complete `PERF-CIRCUIT-MODEL`, the remaining PQ2 runtime groups, PQ2 on AArch64, or the separately required native x86-64 evidence.

## Frozen Inputs

- Stab evidence revision: `f537b3677d91c1f6ee17b916bb08412691fc3989`, clean and unchanged before and after every promotable report and both rollups.
- Stim baseline: v1.16.0 at `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.
- Correctness inventory: `deb6c025854e0e9dc555b45ee5afda33ac22b31c307d41d01731fa320a399f73`.
- Performance inventory: `ce0c451a9c3123be95d3b9606b96a7ce26e3b26f09f543ae7d2e9e0345e86d54`.
- Runtime group contract: `46d089b64d6e06417a72e9fa1be8fce652cd6860fffef40782d1f8890af5a994`.
- Profiler note: `benchmarks/profiler-notes/qualification/perfq-m4-circuit-parse.md` at `d970e7bb466f9305535cd4a9e66944f4323d26e44a7b992c424a5a47ab7fb588` in the evidence revision.

## Correctness Preflight

The current CQ report at `target/qualification/pq2-circuit-parse-current-full` selected and passed exactly these two cases:

- `cq-evidence-qualification-633fa529edf5f549`
- `cq-evidence-qualification-e660819ae9a223c6`

The report passed canonical offline regeneration and exact dependent preflight with the following bindings:

| Artifact | SHA-256 |
| --- | --- |
| Request | `0dc1ed1bdaaf349d8bf7c0d43ec454aa0f9a70a77be14fc928b1e188bd509479` |
| Report | `f9162c35c34a122a7ab6fcab895a9640f7f1b86a6441f7e49e5d692c2ae4d284` |
| Completion | `44a68dd35c2d181e7293a659e3415082d3c2ac74cc5281d578f226604451583c` |
| Preflight | `9c8719502938dedb57ad185ff3063f696364a7d34cb762ea3d47c48d4f91d666` |

Every performance report reopens these artifacts and reconstructs their canonical receipts before timing.

## Reproducible Workers

`just bench::qualification-worker-reproducibility` rebuilt both private workers twice from the clean unchanged evidence revision, verified each live protocol identity, and proved exact pre-barrier rejection of the first unsupported circuit size. Both builds produced the same six worker identities later bound by every source report and rollup.

| Worker identity | SHA-256 |
| --- | --- |
| Stim source | `1a22bfd87554e0c184f130de45ae89c59786e5d2592ded4ebddc701cde5a0abe` |
| Stim build fingerprint | `b17167a0fd156f37c27bb03ee96f0ceca3a6103ae2c9f6b427bca860d43875fe` |
| Stim binary | `d6d4a654bfb810c73bc1d4b13de744e3cf4c8b4cec59af828c4bc57d50bfb2e1` |
| Stab source | `7568b5a1cd0d53959f5abaea776bfb79b4a26346745a907fc00b6fff71b10e87` |
| Stab build fingerprint | `849fd8cee91d2e81c7d768cb6d48fc7d6f59edc99061668cff4bcc34dbda8988` |
| Stab binary | `78838783638fbebd6e761e975b2679992a20e3ac512b814b34fce7326045bec2` |

## AArch64 Timing Results

All reports used the verified `linux-aarch64-controlled` host, matched exact input bytes and digests, matched the normalized semantic output digest, retained raw paired samples, and completed without a noise rerun.

| Scale | Tier | Pairs | Median ratio | Bootstrap 95% interval | Ratio rMAD | Outcome |
| --- | --- | ---: | ---: | --- | ---: | --- |
| 64 instructions | Full | 9 | 1.290179 | [1.273320, 1.298316] | 0.003069 | Failed |
| 64 instructions | Soak | 15 | 1.281841 | [1.275644, 1.283951] | 0.004835 | Failed |
| 4,096 instructions | Full | 9 | 1.272669 | [1.268562, 1.281227] | 0.003227 | Failed |
| 4,096 instructions | Soak | 15 | 1.273620 | [1.271603, 1.280911] | 0.003526 | Failed |
| 65,536 instructions | Full | 9 | 1.378636 | [1.342293, 1.409809] | 0.011112 | Failed |
| 65,536 instructions | Soak | 15 | 1.351942 | [1.348378, 1.373912] | 0.010439 | Failed |

Both full and soak family outcomes are `failed` with three failed measurements and zero noisy measurements. Regression replay rejects every promotable source report because `parse` has a non-passing outcome. The PR report remains valid diagnostic evidence but is correctly rejected by regression dispatch because a product PR report is nonpromotable.

The medium scale is closest to the gate, but its lower confidence bound remains above `1.25`. The large-scale result is materially worse and proves that fixed setup cost is not the sole cause. The next optimization must target parser, instruction-construction, replacement, or destruction throughput without changing the workload or acceptance rule.

## AArch64 Memory Results

Memory is evidence, not a substitute for the timing gate. Stab has higher peak RSS at small and medium scales but lower peak RSS at the large scale.

| Scale | Tier | Stim peak RSS | Stab peak RSS | Direction |
| --- | --- | ---: | ---: | --- |
| 64 instructions | Soak | 3,403,776 bytes | 4,440,064 bytes | Stab higher |
| 4,096 instructions | Soak | 4,231,168 bytes | 5,324,800 bytes | Stab higher |
| 65,536 instructions | Soak | 19,456,000 bytes | 18,509,824 bytes | Stab lower |

The large-scale memory advantage means the timing deficit must not be attributed to retained circuit size from RSS alone.

## Authoritative Artifacts

| Evidence | Path | Report SHA-256 |
| --- | --- | --- |
| Small full | `target/benchmarks/qualification/pq2-circuit-parse-current-small-full` | `4a2bcd0bc8355af16ab3d45bc28cb3b9d2d42b4c90c6e02b6d243c5755389df3` |
| Medium full | `target/benchmarks/qualification/pq2-circuit-parse-current-medium-full` | `d4136fc882db97d4809644a6785929b29e98300514e8004968d045100b389ac4` |
| Large full | `target/benchmarks/qualification/pq2-circuit-parse-current-large-full` | `473db23c470642be70be28c97c50bd0cfe1c00dcc0c9901a894a35a74d9dca49` |
| Small soak | `target/benchmarks/qualification/pq2-circuit-parse-current-small-soak` | `19b39612bbc44c5978600ccb0774123b72fb4b904d7c60bef1e65bc40accf35c` |
| Medium soak | `target/benchmarks/qualification/pq2-circuit-parse-current-medium-soak` | `7ceee5231617eed79b0a1ca59c1770e8986a82a787d15827ce9a6d08614bd5c6` |
| Large soak | `target/benchmarks/qualification/pq2-circuit-parse-current-large-soak` | `be868f0200f8a34a1d101aeb2dcace8910bea06cd1d08e6ecc47ac71dbec9d52` |
| AArch64 full rollup | `target/benchmarks/qualification/pq2-circuit-parse-current-aarch64-full-rollup` | `f9202702c217432a5f8acf25cc4e581805e8c2e60c1a0fb25d4391f07f6b5b85` |
| AArch64 soak rollup | `target/benchmarks/qualification/pq2-circuit-parse-current-aarch64-soak-rollup` | `ddc35c7e871266ae249a95e6d8e3012318e52c0f1e05d233aeeaaa1fed8945b1` |

Both rollups were produced while the checkout still exactly matched the source-report commit and passed `just bench::qualification-rollup-report`. They bind all three required scales, one architecture, one tier, one correctness preflight, one runtime contract, and one exact six-digest worker identity.

## Review Closure

The GPT-5.6/max full-code-review found that the private Stab receipt originally hashed the live source instead of the materialized source actually compiled. The implementation now hashes and rechecks the materialized worker source and requires the sealed worker to confirm its source and build identities. The post-fix review found no remaining confirmed issue.

The milestone audit found that the first-unsupported C++ adapter probe did not prove exact subprocess rejection and that the Stab diagnostic contract differed from the assumed one-line error. The check now requires implementation-specific exit status, empty stdout, and exact stderr bytes for both sealed workers. The post-fix audit found no remaining implementation issue or new under-specification.

Clean integration exposed one additional canonical-report defect: default `serde_json` float parsing changed a valid report decimal by one ULP and made immediate offline replay reject newly generated bytes. Workspace `serde_json` now enables `float_roundtrip`, and a regression preserves the exact failing decimal and first-byte mismatch diagnostic. Immediate replay of the final PR, full, soak, and rollup artifacts passes.

## Remaining Blockers

1. Optimize `stab-core` circuit parsing until both median ratio and confidence-interval upper bound are at most `1.25` on all three AArch64 scales, then regenerate correctness-bound full and soak reports and both AArch64 rollups from one new clean commit.
2. Capture permitted release stack profiles for both workers at medium and large scales. This host currently has `perf_event_paranoid=4`, so fresh stack attribution requires an authorized host configuration change or another controlled AArch64 host. Do not infer a line-level owner from source inspection alone.
3. Produce the same clean full and soak scale families and rollups on a controlled native Linux x86-64 host. No x86-64 timing conclusion is currently claimed.
4. Implement and qualify the remaining PQ2 runtime groups. This first group does not close the broader deterministic performance inventory.

Do not waive the comparable slowdown, change the fixtures, shrink the large scale, loosen the `1.25x` threshold, or cite the large-scale memory result as a timing pass.

## Verification

The implementation revision passed workspace format, clippy, tests, correctness and performance inventory checks, benchmark smoke, worker reproducibility, exact CQ report regeneration and preflight, immediate offline replay for every report, and full and soak rollup replay. `qualification-regression` failed closed for the expected reasons: incompatible nonpromotable PR disposition for the PR diagnostic and non-passing `parse` outcomes for all six promotable reports.

Milestone audit and GPT-5.6/max full-code-review were completed before final evidence generation. No required process remains running.
