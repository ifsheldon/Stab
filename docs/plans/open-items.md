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
- A single `Tableau::append` still visits every row, about 4 ns per row at 5000 qubits where the row stride makes it memory-bound; batched callers including `tableau_to_circuit` use `TableauAppender`, and `Tableau::prepend` is the word-parallel left-action route.
- Pauli string multiplication by a non-identity right operand is about 2.4x Stim (207 ns against 82 ns at 10,000 qubits and 19.9 µs against 8.2 µs at 1,000,000); an identity right operand returns early, so a mirror of Stim's identity-string perf rows does not exercise the kernel.
- A `CX` `Tableau::prepend` takes 433 ns at 10,000 qubits against 260 ns for Stim's `prepend_ZCX` and keeps about 45 ns of fixed cost at 100 qubits, where an `H` prepend takes 10 ns.
- An `r8` record that spans a buffer refill is redecoded from its start on every read, so a wide record arriving through a pipe costs work that grows with the square of its frame.
- The x86-64 instruction-level kernel paths and the per-call level check have never been executed or measured on a controlled x86-64 host.

## Memory

- Parse admission charges two target slots per target, so a parsed circuit's hold (859 MB for the 147 MB flattened circuit) is about 1.75 times its resident size, and commands on that circuit are refused under the 1 GiB default although they peak below 1 GiB when admitted.
- `sample` and `detect` stream only requests of 1 to 63 shots: one shot of a 5,000,000-round repetition code (10,000,000 measurements) peaks at about 7 MB against Stim's 6 MB. From 64 shots on they still keep one plane per measurement (and detector) for every lane plus about 16 bytes per record bit of encoder transpose scratch, 240 MB at 64 shots and 475 MB at 256 to 1000 shots on that circuit (786 MB for `detect --shots 1000`), below Stim's 268 MB to 2.36 GB but proportional to the lane width; the streaming run that would keep only the delivered records measured 5 to 15 percent slower once it fills whole batches.
- `m2d` converts whole 256-record batches of input records through full measurement and detector planes, 1.2 GB for one record of that circuit against Stim's 6.9 GB, and PTB64 output always materializes its planes.
- `convert` admits an `r8` reader at the worst-case frame of one byte per record bit, about 3.3 bytes per bit with buffer growth, so `r8` records wider than roughly 175 million bits are refused under the 1 GiB default although a sparse record decodes in an eighth of a byte per bit; an incremental `r8` decoder would remove the frame buffer.
- The sparse reverse tracker meters its live sensitivity sets after they grow and checks the budget once per target group, so one wide Pauli-product target can overshoot before it is refused, and the meter counts lengths, not capacities or recycled scratch.
- Parsing still builds a tag string before charging it, keeps up to four bytes per recovered non-UTF-8 tag byte against the two it charges, and rebuilds the item list after tag restoration without a separate charge; the text-record transport buffer can reach about four times its half-budget cap when the cap is not a power-of-two multiple of 64 KiB.

## Cancellation

- Sampling polls are spaced by estimated work rather than wall-clock time, and session setup outside the reference sample, session teardown, delivering pending lanes, and the caller's sink run without polls.

## Compatibility

- `m2d` with the reference sample differs from Stim on non-deterministic detectors after a Pauli-product measurement, because the reference simulator picks a different outcome for the later random measurement: `MPP X0*X1` then `MY 1 0` with all-zero measurements gives `001` where Stim gives `010`; with `--skip_reference_sample` the outputs agree.
- Seeded `sample_dem` detector output changes when `--err_out` is added, because recording errors assigns random streams by error ordinal while discarding them skips zero-effect repeats; Stim's output does not depend on the flag.

## Structure And Evidence

- `Gate` is an 8-byte handle beside `GateKind`; making it the enum would shrink `CircuitItem` and change every byte-admission number derived from its size.
- The engine's admission counting inside lowering and the detecting-region snapshot walk do not use `FoldedCircuitSummary`.
- The sparse tracker's unitary-repeat fold (`sparse_rev_frame_tracker/unitary_repeat.rs`) still recurses once per nested repeat with a count above one, and the 256-level detecting-region test nests `REPEAT 1` blocks, which skip that fold.
- `registry.global-min-publish-age` is not configured, because a publish-age cooldown conflicts with the rule to add dependencies at their latest crates.io version; that policy choice is open.
