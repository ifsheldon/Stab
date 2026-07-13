# CQ1 Correctness Harness Progress Report

## Status

CQ1 is complete as of 2026-07-14.

The executable harness landed in `f957ac9aa671fa404d6717267f2f9d4dc78bb19d`, the initial dependent PQ0 digest refresh landed in `79763160d2124d7f3dca325860c57a9a2c973e11`, and clean promotable correctness evidence was produced from Stab revision `e7ba513822c26859a2b5c70c94d406e1c6adb6b6` with `local_modifications=false`. Final CQ1 checklist acceptance is synchronized into PQ0 at digest `cc9f6cabfb9a3245d9c52000e82c8f1bec76aed605f3563d1a15244d327c3fbd` in this report change.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Correctness inventory: schema version 3 at digest `b2909c677a66e2b034c8ab26e8dc1b2ad78e63900b2d83f938a8c4e725852141`.

Evidence platform: Linux AArch64, target `aarch64-unknown-linux-gnu`, Rust `1.98.0-nightly`.

CQ1 makes the frozen CQ0 selectors, comparators, statistical plans, property plans, resource contracts, execution receipts, reports, and preflight bindings executable. It does not claim that the 3,662 planned CQ2 through CQ5 atomic owners already pass.

## Delivered Harness

- `stab-oracle qualification correctness run` executes exact source-owned cases at `pr`, `full`, and `soak` tiers with exact feature and case filtering.
- Every run publishes a canonical pre-execution request, one content-bound execution receipt per selected case, a canonical completion receipt, `report.json`, derived `report.md`, and `preflight.json`.
- Correctness preflight requires controller-approved request and completion digests and validates the exact manifest, Stab and Stim commits, selection, selector, executable identities, environment, output framing, artifacts, and result.
- Stab and Stim are rebuilt in private configuration-free environments for each run. Direct and compiler-subordinate executable bytes are sealed and hashed before use.
- Cargo executes from `/` with absolute manifest paths. Git status uses a private config-free index reconstructed from `HEAD`, and the pinned Stim checkout must have no tracked or untracked modifications.
- Child execution has explicit environment allowlists, bounded stdout and stderr, process-group timeouts, sticky cancellation, exact Cargo-test counts, and bounded artifact receipts.
- Fixture side outputs and qualification artifacts use descriptor-relative no-follow operations, identity-checked publication, parent-directory synchronization, bounded cleanup, and quarantine instead of an unbounded cleanup fallback.
- Statistical cases use canonical integer rejection boundaries, exact completion markers, declared comparison and batch multiplicity, deterministic soak seed panels, and suite-wide false-positive accounting.
- Property cases support typed static corpora or deterministic generated workers, bounded shrinking, persisted regressions, replay, output limits, and killable timeouts.

## Clean Evidence

| Tier | Selected | Passed | Failed | Planned visible | Deferred selected | Statistical shots | Result |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| `pr` | 299 | 299 | 0 | 3,662 | 0 | 3,918,400 | Pass |
| `full` | 410 | 410 | 0 | 3,662 | 0 | 3,918,400 | Pass |
| `soak` | 410 | 410 | 0 | 3,662 | 0 | 4,547,200 | Pass |

Every tier reports `selection_complete=true` and `local_modifications=false`.

### Artifact Digests

| Tier | Request SHA-256 | Completion SHA-256 | Report SHA-256 | Preflight SHA-256 |
| --- | --- | --- | --- | --- |
| `pr` | `bf5f5ba809b13ff49852777c158f1eeafcc365f983db087375e6f5ce026d24a0` | `b7e60aa629dc740f2b379ccf985bc48656c04771808051ab7ec883318c4669dd` | `d3974fae5a5ca3f29424f4c8608d1b9863597a552b31efcc02317be3913e8639` | `acabc96ed13a2f56a0195f509c2cf5101ccfa689e1f208ec75016d60fc1d0fdd` |
| `full` | `9dd4f7d8e97d4e1574ebd924290fa79e66f0d992a8a50047d4083b2d4fbaa697` | `8061fc77d26d46d9ae8388f3b428b9324da1efbad1a63809ec1254786fb143be` | `e5376b833838dccb14e95023c64d1a10b82dc8e0325888864cc73b771e335f00` | `f7a80d936cf9be0e7d4ae9ec9ee52b03ab1ee9fc5910db55f6d77f94027ee057` |
| `soak` | `14e7f2e52ecfa94382113a8eb706b1e0426faffb3f9290980b5dfa0b3182debc` | `2e8f35d757599ecb99fcda27cf283d668e66115c2330e529c9ce0a5752546d12` | `648944c267696b9b62c1cadbd3ede89889769d5a191798b1c74f8c5498efa389` | `7a08bc14c6e713183799f577246585a7d9f610e8497a108a59c21a31441541ad` |

Generated evidence remains under `target/qualification/correctness/pr`, `target/qualification/correctness/full`, and `target/qualification/correctness/soak`.

## Full-Tier Domain Results

| Domain | Passed | Failed | Planned for later CQ milestones |
| --- | ---: | ---: | ---: |
| `CQ-STIM-FORMAT` | 3 | 0 | 92 |
| `CQ-DEM-FORMAT` | 12 | 0 | 134 |
| `CQ-RESULT-FORMATS` | 3 | 0 | 211 |
| `CQ-GATE-CONTRACT` | 84 | 0 | 644 |
| `CQ-BIT-KERNELS` | 4 | 0 | 384 |
| `CQ-CIRCUIT-API` | 35 | 0 | 329 |
| `CQ-GENERATION` | 15 | 0 | 75 |
| `CQ-ALGEBRA` | 1 | 0 | 635 |
| `CQ-SAMPLING` | 30 | 0 | 507 |
| `CQ-DETECTION` | 10 | 0 | 102 |
| `CQ-DEM-SAMPLING` | 19 | 0 | 27 |
| `CQ-ANALYZER` | 77 | 0 | 134 |
| `CQ-SEARCH` | 54 | 0 | 87 |
| `CQ-FLOW-UTILS` | 56 | 0 | 227 |
| `CQ-CLI` | 5 | 0 | 61 |
| `CQ-RESOURCE` | 2 | 0 | 13 |

## Full-Tier Comparator Results

| Comparator | Passed | Failed | Planned for later CQ milestones |
| --- | ---: | ---: | ---: |
| `exact-bytes` | 126 | 0 | 333 |
| `exact-value` | 97 | 0 | 0 |
| `error-class` | 19 | 0 | 0 |
| `structural` | 105 | 0 | 225 |
| `state-equivalence` | 10 | 0 | 619 |
| `semantic-invariant` | 14 | 0 | 418 |
| `statistical` | 32 | 0 | 528 |
| `property` | 6 | 0 | 1,315 |
| `resource` | 1 | 0 | 13 |
| `canonical` | 0 | 0 | 211 |

The zero executable canonical cases and all other planned counts remain visible because CQ1 owns harness execution, not domain expansion. CQ2 through CQ5 must replace those planned owners with atomic evidence or an explicit valid disposition.

## Statistical, Property, And Resource Evidence

The full tier executed all 3,918,400 planned statistical shots. Its declared suite bound was `3.20000000000000053e-5`, and its consumed bound was `2.67062845963454362e-6`.

The soak tier expanded eligible oracle plans across their deterministic seed panels, executed all 4,547,200 planned shots, and consumed `5.98047030092843113e-6`, still below the declared suite bound.

Six independently selectable property corpora passed: five source-owned static bit, SIMD, Pauli, and twiddle corpora plus the generated CQ1 worker lifecycle contract. The implemented resource case proved descriptor-relative rejection of symlink-parent and traversal attacks under bounded output and artifact limits. Thirteen additional public-boundary resource families remain explicit planned CQ2 through CQ5 work.

## Audit And Review

The milestone audit initially found three evidence-quality defects: a descriptor-lifetime test that could observe a reused file descriptor under parallel execution, a Cargo isolation test that bypassed the production helper and omitted hostile scratch-ancestor configuration, and a nested-directory durability test that was not regression-sensitive to removal of the parent synchronization.

The fixes resolve the descriptor target before releasing ownership, route both production and adversarial Cargo builds through the fixed-root helper, and count successful parent synchronizations for every newly created nested directory. The normal parallel `stab-oracle` suite then passed 312 tests with two intentionally ignored long-running tests.

The security re-audit found one final bounded-cleanup defect: automatic `TempDir` destruction could fall through to unbounded path cleanup after CQ1's bounded descriptor cleanup refused an over-depth runtime. `PrivateRuntime` now persists the temporary directory, retains root and parent descriptors, performs only bounded descriptor cleanup, and quarantines over-budget trees. Its focused regression passed.

Final milestone-audit and full-code-review closure found no remaining actionable P0 through P3 implementation or specification issue. The resolved CQ1 specification gaps for executable provenance, statistical multiplicity, descriptor ownership, property-plan ownership, deferred diagnostics, report axes, and Linux process/publication semantics remain recorded in `docs/plans/milestone-spec-gaps.md`.

## Controlled-Host Trust Root

Promotable CQ1 evidence assumes a controlled Linux host. The outer `cargo run` bootstrap, kernel, procfs, process-group and dynamic-loader behavior, system shared libraries, and dependency-cache contents remain trusted. Recorded SHA-256 identities provide reproducibility and tamper evidence, not third-party authenticity.

Promotable runs also require no malicious concurrent same-UID process to transiently mutate and restore the live checkout, linked Git refs or objects, or compiler and CMake support aliases during execution. CQ1 detects persistent changes before execution and publication but does not claim authenticated isolation from a hostile local operator.

## Verification

The following checks passed:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
cargo test -p stab-oracle --quiet
just oracle::version
just oracle::matrix --check
just oracle::blockers --check-selectors
just oracle::run --implemented-only
just qualification::correctness-regenerate --check
just qualification::correctness-check
just qualification::correctness-provenance-probe
just qualification::correctness-run --tier pr
just qualification::correctness-run --tier full --out target/qualification/correctness/full
just qualification::correctness-run --tier soak --out target/qualification/correctness/soak
just qualification::correctness-report --out target/qualification/correctness/latest
just qualification::correctness-report --out target/qualification/correctness/full
just qualification::correctness-report --out target/qualification/correctness/soak
just bench::smoke
just bench::qualification-regenerate --check
just bench::qualification-check
just maintenance::pre-commit
```

The PR report was validated at the default `latest` path and then preserved byte-for-byte under `target/qualification/correctness/pr`; its exact-case preflight remained valid at the preserved path. An exact-case preflight passed for each tier using `cq-evidence-blocker-018716cd2ac39dbd` and that tier's controller-approved request and completion digests.

## Next Milestone

PQ1 is now active. It must build the symmetric paired benchmark runner, pinned-Stim adapter, bounded Stab worker, correctness and output preflight, host policy, calibrated paired statistics, exact submeasurement pairing, memory evidence, and report commands against the completed CQ1 machine-readable contracts.

CQ2 through CQ6 remain later correctness milestones. Intentionally deferred products remain outside this program slice and are not counted as passes.
