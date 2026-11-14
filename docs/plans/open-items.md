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
- `sample_dem --replay_err_in` is up to about 2x Stim for dense HITS and DETS and up to about 5x for `r8` and PTB64 on models whose errors sit in one `repeat` block, where locating each replayed error dominates; the replay input is also decoded twice, because a validation pass keeps a malformed replay from writing any output.
- A single `Tableau::append` still visits every row, about 4 ns per row at 5000 qubits where the row stride makes it memory-bound, and `tableau_to_circuit` uses it because it reads the remaining Tableau between gates; `TableauAppender` and `Tableau::prepend` are the word-parallel routes.
- `Tableau::prepend` carries about 40 ns of fixed cost per call, which is roughly three times Stim's specialized prepends below 500 qubits, and Pauli string multiplication is 1.05x to 1.23x Stim.
- The x86-64 instruction-level kernel paths and the per-call level check have never been executed or measured on a controlled x86-64 host.

## Memory

- The CLI applies `--memory-budget` per stage, not per command, so one command can retain a small multiple of the budget; a 147 MB flattened circuit peaks at 1.35 GB under the 1 GiB default.
- Peak memory at large record widths is far above Stim's for shapes that Stim streams, such as 415 MB against 6 MB for a 5,000,000-round repetition-code `sample`.
- Time reversal, missing-detector discovery, flow generation, detecting regions, sampled flows, and Pauli and Clifford strings admit against the default budget and take no caller budget.

## Cancellation

- A circuit-sampling or detection-sampling session that observes cancellation inside a batch is poisoned, while the same request observed between batches is resumable.
- Polls are spaced by work units, not time, so one very wide instruction can run long between polls.
- Circuit passes, logical-error search, SAT construction, and the flow checker do not poll.
- `stab_decoder::DecodeCancellation` is a separate signal type from `stab_model::Cancellation`.

## Structure And Evidence

- Detecting regions keep a 128-level repeat-nesting bound instead of the shared 256, because the traversal is recursive and overflowed a debug test stack at 256.
- `Gate` is an 8-byte handle beside `GateKind`; making it the enum would shrink `CircuitItem` and change every byte-admission number derived from its size.
- The engine's admission counting inside lowering, the detecting-region snapshot walk, and the reverse-tracker metering do not use `FoldedCircuitSummary`.
- `stab plan` does not report the frame lane width that a shot count selects, and no pinned-Stim statistical fixture row runs at a shot count that selects 64 lanes.
- `PAULI_CHANNEL_1` probabilities differ from Stim in the last printed digit in some analyses.
- `registry.global-min-publish-age` is not configured, because a publish-age cooldown conflicts with the rule to add dependencies at their latest crates.io version; that policy choice is open.
