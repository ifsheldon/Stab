# Pinned Stim Qualification Adapter

This directory owns the narrow C++ adapter used by `stab-bench` when pinned Stim v1.16.0 has no faithful public CLI or `stim_perf` phase boundary.

The adapter is benchmark infrastructure only. It does not create a Stab C++ API or C++ header compatibility promise.

`ops/bench` validates the pinned Stim commit, hashes this source and the built Stim library, records descriptor-safe CMake and C++ compiler identities plus exact flags, builds below `target/benchmarks/stim-adapter/`, and rejects stale source, tool, library, binary, or receipt identities before and after accepting output.

The PQ1 adapter and Stab worker share a bounded schema-versioned JSON Lines protocol containing exact workload and measurement ids, evidence mode, iterations, semantic work, elapsed time, semantic output digest, setup and peak RSS, CPU affinity, pinned Stim commit, source digest, and build fingerprint. The parent rejects malformed, oversized, duplicated, stale, non-finite, missing, or unexpected rows before computing any pair.

Timed qualification invocations use a one-byte start barrier. The parent pins the child and installs any regular-file limit before releasing that barrier, and both workers verify their singleton CPU affinity before measured work begins.

Reproduce the process boundary and adapter boundary independently with:

```sh
just bench::qualification-probe --group pq1-process-contract-smoke
just bench::qualification-probe --group pq1-adapter-protocol-smoke
```

The PQ1 protocol-smoke body is deliberately synthetic and report-only. It proves runner symmetry and evidence reconstruction but is not a benchmark of Stim or Stab product behavior; product adapters added in later milestones require exact CQ preflight and source-owned workload contracts.
