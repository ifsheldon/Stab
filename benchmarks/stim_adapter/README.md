# Pinned Stim Qualification Adapter

This directory owns the narrow C++ adapter used by `stab-bench` when pinned Stim v1.16.0 has no faithful public CLI or `stim_perf` phase boundary.

The adapter is benchmark infrastructure only. It does not create a Stab C++ API or C++ header compatibility promise.

`ops/bench` validates the pinned Stim commit through a config-free Git view, materializes both committed Stab source and pinned Stim source into a fresh private `/tmp` runtime, and never reuses a prior CMake cache or library. It records canonical `cmake`, `cc`, `c++`, and `make` identities plus exact configure, library-build, compile, and environment contracts; hashes this source and the resulting Stim library; recomputes the build fingerprint from those inputs; seals the final adapter executable; and rejects stale or internally inconsistent source, tool, library, binary, argument, environment, or receipt identities before and after accepting output.

The adapter and Stab worker share a bounded schema-versioned JSON Lines protocol containing exact workload and measurement ids, evidence mode, iterations, semantic work, elapsed time, semantic output digest, setup and peak RSS, CPU affinity, pinned Stim commit, source digest, and build fingerprint. The parent derives expected work from the selected source-owned scale, performs semantic preflight at the exact retained batch shape, and rejects malformed, oversized, duplicated, stale, non-finite, missing, unexpected, work-mismatched, or post-preflight digest-mismatched rows before computing any pair.

Timed qualification invocations use a one-byte start barrier. The parent pins the child and installs any regular-file limit before releasing that barrier, and both workers verify their singleton CPU affinity before measured work begins.

Reproduce the process boundary and adapter boundary independently with:

```sh
just bench::qualification-probe --group pq1-process-contract-smoke
just bench::qualification-probe --group pq1-adapter-protocol-smoke
just bench::qualification-probe --group pq2-circuit-parse-adapter-smoke --iterations 2 --work-items 64
just bench::qualification-probe --group pq2-circuit-canonical-print-adapter-smoke --iterations 2 --work-items 64
```

The PQ1 protocol-smoke body is deliberately synthetic and report-only. It proves runner symmetry and evidence reconstruction but is not a benchmark of Stim or Stab product behavior.

The first product adapter workload is `circuit-parse` with measurement `parse`. Both workers build the same bounded deterministic instruction fixture before timing, parse it for the calibrated iteration count, and consume an untimed digest of the resulting canonical circuit. The digest removes Stab's single terminal newline before comparison because pinned Stim's `Circuit::str()` omits that newline; it does not otherwise normalize circuit text. The probe proves worker symmetry but remains diagnostic. A promotable ratio additionally requires a source-owned runtime group, exact passing CQ preflight, a clean full or soak run, and a verified host.

The second product adapter workload is `circuit-canonical-print` with measurement `serialize`. Both workers build and parse the same exact fixture before the start barrier, then time only repeated conversion of the prepared circuit into canonical `.stim` text. Every produced string is consumed, the final string is digested outside timing, and only Stab's single terminal newline is removed before comparison. Parsing and fixture construction are setup work, while output allocation and destruction remain part of the serialization measurement.
