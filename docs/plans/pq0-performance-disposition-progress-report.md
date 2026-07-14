# PQ0 Performance Disposition Progress Report

## Status

Completed: 2026-07-13.

Compatibility target: Stim v1.16.0 at commit `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`.

Current correctness dependency after the completed selected CQ2 `.stim`, `.dem`, result-format, gate-contract, and bit-kernel slices: schema-version-3 CQ0 semantic digest `2b2f0456e568b86a973d4b9077b9688ab9f7748af1bd9cd349e2a2abf217d638`.

Current performance inventory digest after dependent CQ2 regeneration: `4e31a348b0c796ae4c4400369c70019eff8fa991592f201c80e7fee7d8718f7a`.

Implementation evidence revision: `abf7cd1bae0de045f62e976a290507238153f976`.

Initial corrected CQ dependency regeneration revision: `79763160d2124d7f3dca325860c57a9a2c973e11`; clean CQ1 evidence revision: `e7ba513822c26859a2b5c70c94d406e1c6adb6b6`; final acceptance-status synchronization revision: `6d4c55fdddf84c90bd3f64c2bf49bf67a9b786ba`, validated with `local_modifications=false`. The final synchronization does not change any PQ0 disposition or unresolved count.

The original PQ0 evidence state was clean at the source-owned revision above.
CQ1's confirmed exact-selector correction changed the CQ0 digest, so the checked PQ0 inventory and frozen performance digest were regenerated without changing any group, disposition, unresolved count, threshold, waiver, or acceptance conclusion.
The completed selected CQ2 `.stim`, `.dem`, result-format, gate-contract, and bit-kernel slices changed correctness owner ids, exact classifications, and acceptance state, so PQ0 was regenerated without changing those performance classifications. The bit refresh preserves all 536 runtime groups and all inherited disposition, threshold, waiver, and unresolved counts while rebinding exact correctness ownership. Clean current-digest PQ1 PR, full, and soak execution from revision `d4301cc1085680ff650f9e0474e075998c14c4bd` passes offline report validation and report-only regression replay.

PQ0 freezes inventory and migration decisions only.
It does not claim that inherited timing ratios satisfy the comprehensive runner, preflight, scaling, memory, or statistical contract.

## Delivered Artifacts

- `benchmarks/stim-qualification-suite.json` is the checked deterministic overlay on `benchmarks/manifest.csv`.
- `ops/bench/src/qualification/` owns checklist extraction, source discovery, schema types, validation, deterministic regeneration, bounded regular-file reads, atomic writes, listing, and summary reporting.
- `just bench::qualification-list` lists all coverage or one exact `PERF-*` domain.
- `just bench::qualification-check` validates the checked inventory, validates all source references, regenerates it, and byte-compares the result.
- `just bench::qualification-regenerate --check` performs the deterministic regeneration check without timing workloads.

## Frozen Inventory

| Inventory | Count |
| --- | ---: |
| Performance domains | 16 |
| Checklist rows | 126 |
| Done checklist rows | 73 |
| Partial checklist rows with explicit selected and deferred children | 7 |
| Deferred checklist rows | 46 |
| Default-feature public Rust API items | 1,922 |
| Public API items covered by a measured parent | 1,013 |
| Public API items classified as not independently performance relevant | 909 |
| Qualification groups | 536 |
| Measured or planned measured groups | 534 |
| Non-performance metadata groups | 2 |
| Inherited benchmark rows | 161 |
| Upstream perf files | 23 |
| Upstream `BENCHMARK(...)` symbols | 74 |
| Current primary waiver rows with adapter retirement mappings | 5 |
| Groups bound to exact CQ0 API owners | 242 |
| Groups with stable planned CQ preflight ids | 294 |
| Exact inherited threshold measurement pairs | 24 |
| Exact checklist row-and-domain parent groups | 132 |
| Exact checklist child claims across those parents | 180 |
| Partial-row child ownership entries | 50 |
| Typed generated fixture families | 375 |
| Typed repository fixture families with SHA-256 | 35 |
| Typed inline fixture families | 126 |
| Scale points with exact input byte counts | 122 |
| Scale points explicitly not byte-sized | 1,164 |

The API inventory preserves the CQ0 primary performance ownership counts: 274 bit-kernel, 202 circuit, 1 CLI, 128 DEM, 15 DEM-sampling, 62 detection, 105 error-analysis, 150 flow and detector-utility, 178 gate, 107 generation, 97 result-IO, 49 sampling, 4 search, and 550 stabilizer-algebra items.
Declaration-only kinds and marker or diagnostic trait implementations do not receive independent runtime claims.
Each function, method, and behavioral trait implementation is assigned to one or more of 242 planned measured API parents grouped only by canonical owner, phase, and performance domain, with every exact API path and CQ0 `owner_case_id` listed as required evidence, three concrete scale ids, a work unit, and a no-aggregation output contract; the validator rejects absent, non-measured, cross-domain, path-omitting, or owner-omitting parents.
All 460 multi-domain API items preserve their secondary performance domains instead of silently retaining only the first CQ0 domain.
The 72 performance-relevant selected checklist rows map to 132 exact row-and-domain parent groups, while inherited and API groups carry no checklist ownership, so a shared domain cannot make an unrelated benchmark claim the row.
The seven partial rows carry explicit stable selected and deferred child ids plus 50 machine-readable child-to-domain ownership entries; their parent groups carry only the 180 exact child claims selected for that domain, and no `(child_id, performance_feature)` pair has more than one primary owner.
The broad `.stim`, `.dem`, and result-format row, for example, assigns only the result-format child to `PERF-CONVERT-CLI`; gate, DEM, flow, and analysis children remain with their own domains.

Every workload fixture is a typed generated, repository-file, or inline locator.
The 35 repository fixtures use bounded nonsymlink reads and store SHA-256 corpus digests, so same-length content drift changes the semantic digest.
Every scale point stores either an exact input-byte count or an explicit not-applicable state; generated workloads also use a registered group-kind generator, an exact seed, and exact `small`, `medium`, and `large` parameters.

## Inherited Row Decisions

| Decision | Count | Meaning |
| --- | ---: | --- |
| Retained | 15 | The upstream operation shape is faithful, but comprehensive preflight, output, scaling, and statistical evidence is still missing. |
| Reworked | 135 | The row needs a faithful runner, exact phase split, scale family, output contract, or another material correction. |
| Diagnostic | 4 | The row remains useful for investigation but cannot produce a comprehensive ratio in its current form. |
| Superseded | 5 | A more specific row owns the behavior and the duplicate identity must be removed during manifest migration. |
| Removed | 2 | `m7-perf-harness` and `m12-primary-performance-matrix` are metadata rather than timed workloads. |

The five superseded identities are `m10-analyze-errors-fold-cli`, `m9-feedback-inline-mpp-batch`, `pf3-m2d-sweep-b8`, `pf7-cli-m2d-sweep-b8`, and `pf7-cli-m2d-feedback-inline`.
The four diagnostic identities are `m7-cli-dispatch`, `m7-convert-stim-canonical`, `m7-convert-01-to-ptb64`, and `pf3-gate-semantic-wide`.

## Unresolved Work Frozen For Later Milestones

| Classification | Rows | Owning follow-up |
| --- | ---: | --- |
| Proxy workload | 10 | PQ2 through PQ5 exact workload replacement |
| Stale metadata row | 2 | PQ1 manifest migration |
| Duplicate workload | 5 | PQ1 manifest migration |
| Missing scale family | 124 | PQ2 through PQ6 |
| Missing qualification correctness preflight | 159 | PQ1 plus dependent CQ milestones |
| Missing semantic output digest | 159 | PQ1 |
| Missing current comparator | 73 | PQ1 adapter and process runners |
| In-process Stab versus Stim process mismatch | 58 | PQ1 symmetric process CLI runner |
| Heterogeneous upstream measurement selector | 21 | PQ1 exact submeasurement pairing or row split |
| Heterogeneous selector without current exact threshold pairs | 15 | PQ1 row split or exact pair inventory |

All 58 current `stim-cli` rows are explicitly marked asymmetric and cannot enter the comprehensive primary gate until PQ1 runs built Stab and Stim processes symmetrically.
Five aggregate upstream rows, including SIMD table, tableau simulator, graphlike search, error analyzer, and DEM sampler coverage, remain visible for rework because their current Stab workload is not the same operation or scale as the selected Stim measurements.
No inherited or proposed row is granted a `no-faithful-stim-comparator` conclusion in PQ0.
The five current M12 no-ratio waivers instead name concrete pinned-Stim adapter retirement symbols for circuit serialization, DEM serialization, `01` to `ptb64`, and `ptb64` reading.

## Qualification Outcomes

| Outcome | Count | PQ0 meaning |
| --- | ---: | --- |
| Timing groups qualified | 0 | PQ1 must first provide symmetric runners, executable correctness preflights, output digests, host policy, and paired statistics. |
| Memory groups qualified | 0 | PQ1 and PQ6 own process RSS and Stab allocation evidence. |
| Comparable 1.25x passes | 0 | Existing M12 thresholds remain active but are inherited evidence, not comprehensive-suite qualification. |
| Comparable 1.25x failures | 0 | No comprehensive timing run occurred in PQ0. |
| Noisy timing groups | 0 | No comprehensive timing run occurred in PQ0. |
| No-faithful-comparator groups | 0 | PQ0 found adapter or public-command paths for every selected group; later runner work may prove otherwise only with validator-backed evidence. |

These zeroes are expected milestone results rather than missing report fields.
PQ0 classifies the finite work and rejects premature qualification; it does not run timing, memory, scaling, or 1.25x acceptance measurements.

## Validation Contract

The schema denies unknown JSON fields and validates exact frozen counts, unique ids and API paths, the CQ0 digest, exact API owner cases and all primary or supporting domains, source line and SHA-256 anchors for all checklist rows, exact child-to-domain ownership for every selected child, measured parents, inherited or planned primary row ids, registered generators, exact scale ids and parameters, typed input-byte states, typed fixture locators and static corpus digests, every manifest primary owner, complete 1.25 threshold values and exact measurement pairs, complete waiver reasons, safe repository-relative fixture and pinned-source paths, all 74 upstream perf symbols and matching benchmark filters, adapter retirement mappings, and the frozen semantic digest.
Group-to-group parent cycles are eliminated by construction because only checklist and API dispositions can reference measured groups, while qualification groups cannot reference other groups as parents.
Inputs and the checked inventory must be bounded regular nonsymlink files with nonsymlink repository ancestors.
On Unix, regeneration creates and renames the temporary output relative to one held parent-directory descriptor with `NOFOLLOW`, preventing an ancestor swap from redirecting the write; the resulting source-owned file mode is `0644`.
Bounded source reads also traverse with descriptor-relative `NOFOLLOW`, Stim CLI stdin is capped at `64 MiB` at the execution boundary, and non-Unix benchmark operations fail closed until equivalent handle-relative primitives are available.

Targeted negative tests cover unknown correctness, fixture, measurement, feature, manifest, threshold, and waiver ids; absent or non-measured parents; false no-comparator groups and waivers; asymmetric primary CLI gates; stale and mixed exact or wildcard Stim filters; dropped API owner or secondary-domain bindings; Cartesian or duplicate global checklist ownership; changed threshold ratios and waiver reasons; unknown mapping fields; missing primary rows and correctness dependencies; unregistered generators, wrong seeds, scale drift, fake source-backed API fixture ids, extra parameter keys, and placeholder parameters; missing typed byte counts or static corpus digests; same-length fixture mutation; checked-in inventory symlinks; symlink input and output ancestors; nonregular output destinations; bounded JSON shape limits; and source-owned output permissions.

## Audit And Review

The final `milestone-audit` found no remaining confirmed finding and approved PQ0's dispositions; final CQ1 acceptance-status synchronization now freezes those same dispositions at digest `cc9f6cabfb9a3245d9c52000e82c8f1bec76aed605f3563d1a15244d327c3fbd`.
The final `full-code-review` found no remaining confirmed finding across correctness, schema integrity, benchmark ownership, hostile inputs, filesystem containment, operational commands, tests, module boundaries, and documentation.

Earlier audit and review passes found and drove fixes for truncated and Cartesian checklist ownership, duplicate global child/domain ownership, weak API owner binding, unhashed static fixtures, open-ended generator parameters, incomplete threshold and waiver digests, stale filter handling, asymmetric CLI claims, unsafe pinned-source and repository paths, unbounded manifest and CLI stdin reads, raceable atomic replacement, non-Unix path fallbacks, oversized modules, and stale benchmark-local documentation.
The resolved checklist-child and generator-schema under-specification is recorded in `docs/plans/milestone-spec-gaps.md`.

Residual risk is limited to intentionally unexecuted work: non-Unix benchmark operations are statically reviewed but fail closed, long timing and soak runs belong to later PQ milestones, and the unrelated long-running M4 parser fuzz test remains ignored in the standard workspace suite.

## Verification

The following commands passed from the PQ0 worktree:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --quiet
just oracle::version
just qualification::correctness-check
just bench::smoke
just bench::qualification-regenerate --check
just bench::qualification-check
just bench::qualification-list --feature PERF-RESULT-IO
```

## Next Milestone

PQ1 must build the symmetric process runner, pinned-Stim adapter, Stab worker protocol, correctness and output preflight, calibrated paired statistics, exact submeasurement pairing, host policy, and report commands.
CQ1 now makes the referenced correctness selectors executable and supplies machine-readable preflight evidence. PQ1 must consume those exact prerequisites before any dependent benchmark becomes qualified or enters a new comprehensive 1.25x claim.
