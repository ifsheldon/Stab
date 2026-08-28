# Instructions For Performance Work

- [suite.toml](suite.toml) is the only active workload and policy source. Update [SUITE.md](SUITE.md) with `just bench::e2e-check --write-docs` in the same change.
- Keep the release matrix at or below 30 cases and persistent diagnostics at or below 15. A diagnostic needs a profile showing at least 10 percent of a release workflow.
- Benchmark complete user workflows. Do not add rows for getters, labels, derives, protocol plumbing, or isolated helpers without demonstrated user-visible cost.
- Compare CLI workflows process to process using release binaries, identical inputs, equivalent sinks, and fully consumed output. Keep reusable Rust workflows on stable component APIs.
- Validate correctness before timing. Exact outputs must match exactly; stochastic outputs need format, shape, count, and source-owned semantic witnesses.
- Preserve paired alternating samples, fixed-seed bootstrap intervals, all retained observations, raw semantic work, and kernel-reported child peak RSS.
- Start CLI timing immediately before spawn and stop it at pidfd-signaled child completion after `wait4`; keep monitor polling, final pipe drain, and output validation outside the interval. Sum those child intervals for CLI pipelines.
- Keep the Stim parity ceiling exactly `1.25x` and the seeded Stab self-regression ceiling exactly `1.15x`. Missing baselines are `unseeded`. Do not add waivers or shrink work to pass.
- Bump the explicit timing-boundary identity whenever controller work moves into or out of a timed region. Baselines must also match architecture, CPU, Rust target, Rust toolchain, and case digest.
- Use the shared bounded process supervisor. Preserve concurrent I/O, process-group termination, cancellation, output limits, timeouts, and tested child-RSS accounting.
- Formal evidence requires a clean committed source, exact pinned Stim identity, fixed release builds, CPU affinity, no competing benchmark process, temperature below `100 C`, and no swap I/O.
- Write every bundle to a unique, previously absent child under `target/benchmarks/`. Preserve failed runs and never reuse their paths.
- Run `just bench::e2e-check`, targeted `stab-bench` tests, formatting, warnings-denied Clippy, and the relevant smoke workflow before a requested commit.
