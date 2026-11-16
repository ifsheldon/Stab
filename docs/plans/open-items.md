# Open Items

This list holds known gaps that no active milestone owns.
It was collected on 2026-11-14 when the fearless SIMD plan and the engine speed and architecture plan were completed and retired to Git history; both are last present at commit `7b0aa4f3` under `docs/plans/`.
An item leaves this list when a plan adopts it or a change closes it.
Timings are shared-host diagnostics from the AArch64 development host and promote no performance claim.

## Speed

- Fixed parse-and-compile cost on large circuits is above Stim and outside the suite: a single-shot `sample` on a 147 MB flat circuit is about 1.3x Stim without the reference sample and 1.7x with it, and 1.5 million short instructions are about 3.2x.
- That remaining cost is per-instruction memory in the parsed circuit and the lowered program, and the SHA-256 plan fingerprint, which is about 20 percent of compile; Stim also streams a single shot without building the circuit.
- A random collapse in the reference simulator scans a strided pivot column per measurement; a batched collapse in a transposed layout was not built, and it is most of a single-shot reference run on a wide circuit with few rounds.
- At about 10 percent density, reading HITS or DETS input costs about 2x (100-bit records) to 14x (1,000,000-bit records) reading the same records as `b8`, whose files are 2 to 6 times smaller, although every measured `convert` and `m2d` row is faster than Stim; at about 0.1 percent density it costs no more than `b8`.
- Variable-width `sample_dem --replay_err_in` inputs (`01`, HITS, DETS, and `r8`) whose retained records do not fit what the replay session leaves of `--memory-budget` still decode their input twice, because validation runs before opening output files.
- A single `Tableau::append` still visits every row, about 4 ns per row at 5000 qubits where the row stride makes it memory-bound, and `tableau_to_circuit` uses it because it reads the remaining Tableau between gates; `TableauAppender` and `Tableau::prepend` are the word-parallel routes.
- `Tableau::prepend` carries about 40 ns of fixed cost per call, which is roughly three times Stim's specialized prepends below 500 qubits, and Pauli string multiplication is 1.05x to 1.23x Stim.
- The x86-64 instruction-level kernel paths and the per-call level check have never been executed or measured on a controlled x86-64 host.

## Memory

- The CLI's command-level `--memory-budget` ledger cannot hold a compiled sampling, detection, or measurement-to-detection plan while its session is admitted, because the plans expose neither their retained bytes nor their session estimates; the session is admitted against the plan's compile-time budget, so a command can still peak near the plan plus that budget.
- `sample_dem` plans are not budget-admitted, feedback inlining reports no output size, and `analyze_errors` grows its error accumulator before its materialized-bytes check: the 147 MB flattened circuit reaches 2.6 GB of resident memory under the 1 GiB default (4.8 GB under 2 GiB) before `circuit_to_detector_error_model` refuses it.
- Parse admission charges two target slots per target, so a parsed circuit's hold (859 MB for the 147 MB flattened circuit) is about 1.75 times its resident size, and commands on that circuit are refused under the 1 GiB default although they peak below 1 GiB when admitted.
- `sample` and `detect` stream only requests of 1 to 63 shots: one shot of a 5,000,000-round repetition code (10,000,000 measurements) peaks at about 7 MB against Stim's 6 MB. From 64 shots on they still keep one plane per measurement (and detector) for every lane plus about 16 bytes per record bit of encoder transpose scratch, 240 MB at 64 shots and 475 MB at 256 to 1000 shots on that circuit (786 MB for `detect --shots 1000`), below Stim's 268 MB to 2.36 GB but proportional to the lane width; the streaming run that would keep only the delivered records measured 5 to 15 percent slower once it fills whole batches.
- `m2d` converts whole 256-record batches of input records through full measurement and detector planes, 1.2 GB for one record of that circuit against Stim's 6.9 GB, and PTB64 output always materializes its planes.
- Sampled flows (`sample_if_circuit_has_stabilizer_flows_with_options`) charge neither the noiseless copy of the input circuit nor each flow's augmented witness circuit against `SampledFlowOptions::memory_budget`; both grow with the input circuit, which the caller already holds.
- The sparse reverse tracker's own sensitivity sets are not admitted against a budget; only the Pauli strings it extracts are, so flow checking and detecting regions bound their output rather than their live tracker state.

## Cancellation

- Sampling polls are spaced by estimated work, and one reference-simulator collapse or fold comparison is not split: about 1.3 ms at 12,000 qubits, growing with the square of the qubit count up to tens of milliseconds for the largest tableau the 1 GiB default admits. Session creation, session teardown, delivering pending lanes, and the caller's sink also run without polls.
- One extracted flow region, one search state's successor list, and one emitted SAT clause are single polling ticks, so a very wide flow, a very high-degree detector, or a very long clause runs between polls.

## Structure And Evidence

- `Gate` is an 8-byte handle beside `GateKind`; making it the enum would shrink `CircuitItem` and change every byte-admission number derived from its size.
- The engine's admission counting inside lowering, the detecting-region snapshot walk, and the reverse-tracker metering do not use `FoldedCircuitSummary`.
- `stab plan` does not report the frame lane width that a shot count selects, and no pinned-Stim statistical fixture row runs at a shot count that selects 64 lanes.
- `registry.global-min-publish-age` is not configured, because a publish-age cooldown conflicts with the rule to add dependencies at their latest crates.io version; that policy choice is open.
