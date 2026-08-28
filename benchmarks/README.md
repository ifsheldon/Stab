# End-to-End Performance

Stab has one active performance system. [suite.toml](suite.toml) owns the release workflows, exact semantic work, correctness prerequisites, output witnesses, memory ceilings, Stim parity policy, and Stab self-regression policy. [SUITE.md](SUITE.md) is generated from that file and is the reviewable matrix.

The suite deliberately benchmarks user experience instead of every public function. Its release matrix covers generation, result conversion, circuit sampling, detection, measurement conversion, circuit analysis, DEM sampling, a complete CLI QEC pipeline, and a reusable Rust QEC pipeline. A persistent diagnostic is allowed only when a profile shows that it explains at least 10 percent of one release workflow.

## Gates

- Comparable CLI cases run release-built Stim and Stab processes against identical deterministic inputs.
- Stim parity requires both the paired median ratio and fixed-seed bootstrap confidence upper bound to be at most `1.25x`.
- Seeded Stab self-regression requires both normalized timing bounds to be at most `1.15x` of the accepted architecture-specific baseline.
- An absent or identity-mismatched baseline is `unseeded`, never passing.
- Each case enforces a Stab peak-RSS ceiling and validates all primary and side output outside the timed region.
- Workloads, thresholds, and semantic work may not be reduced or waived to obtain a pass.
- Accepted baselines are keyed by architecture, CPU, Rust target, Rust toolchain, case digest, and the explicit `e2e-user-workflow-v1` timing boundary.

## Measurement

CLI timing includes process startup, parsing, compilation, execution, codecs, and I/O. The Rust pipeline uses stable component APIs and includes the complete reusable sample, detect, and decode workflow. The runner alternates Stim-first and Stab-first pairs, retains every sample, drains every output, records kernel-reported child peak RSS, and normalizes timing by declared semantic work.

Deterministic outputs are compared exactly. Stochastic outputs are decoded and checked for record count, width, format validity, and source-owned nondegeneracy witnesses. Generated circuits are produced independently by Stim and Stab before timing and must agree byte for byte.

## Commands

```text
just bench::e2e-check
just bench::e2e-check --list
just bench::e2e-run --tier smoke --out target/benchmarks/<unique-name>
just bench::e2e-run --tier full --affinity-cpu <cpu> --out target/benchmarks/<unique-name>
just bench::e2e-run --tier soak --affinity-cpu <cpu> --out target/benchmarks/<unique-name>
just bench::e2e-replay --input target/benchmarks/<bundle>
just bench::e2e-baseline-candidate --full target/benchmarks/<full> --soak target/benchmarks/<soak> --out target/benchmarks/<candidate>
just bench::e2e-release-check
```

`e2e-check` is untimed and suitable for pull-request CI. Smoke timing is diagnostic. Formal evidence requires a clean commit, the pinned Stim source, fixed release builds, one CPU affinity, no competing benchmark process, host temperature below `100 C`, and no swap I/O during measured samples.

Every run writes to a previously absent child under `target/benchmarks/`. A bundle contains the exact suite, run identity, correctness witnesses, raw samples, derived JSON and Markdown reports, and a digest manifest. `e2e-replay` rejects extra files or changed bytes and reconstructs the report deterministically without executing a workload.

`e2e-release-check` fails closed until P9 records reviewed full and soak bundles under `benchmarks/evidence/aarch64/` and writes `benchmarks/current-aarch64-evidence.toml`. It permits only evidence and named status-document changes after the measured source revision, and it requires either a passing seeded self-regression result or the exact first baseline derived from an unseeded full-and-soak pair.

The removed milestone and qualification benchmark systems remain visible through Git history and [archive/](archive/) only. They are not active gates or alternate sources of truth.
