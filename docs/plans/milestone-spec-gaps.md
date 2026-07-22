# Milestone Under-Specification Log

This log records milestone loopholes, ambiguous acceptance criteria, and under-specified scope discovered during milestone implementation or milestone audit.
Use this file for specification gaps only.
Implementation defects, missing tests, benchmark failures, documentation omissions, and code-review findings should be fixed in the milestone work unless a separate follow-up is explicitly accepted.

## Entry Format

```text
## YYYY-MM-DD - Mx: Milestone Title

Status: Open | Resolved | Superseded
Revealed by: implementation, test, benchmark, audit, or review evidence
Current text: the milestone wording that was too weak or ambiguous
Gap: what the milestone failed to specify
Proposed amendment: concrete replacement text or additional done criterion
Resolution: link or note for the plan update that resolved the gap
```

## Open Entries

No open entries.

## Resolved Entries

## 2026-07-20 - PQ1/PQ2: Live Repository Root During Publication

Status: Resolved
Revealed by: milestone audit of the Clifford-string review-fix artifact lifecycle.
Current text: the qualification contract required repository-anchored descriptor-owned publication and repeated clean Git-state checks through hierarchy durability.
Gap: the contract did not require the path used by Git and the descriptor used for publication to continue naming the same live repository directory. An exchanged repository root could let Git validate a replacement checkout while publication completed through retained descriptors into a detached old tree.
Proposed amendment: open the absolute repository path component by component with nofollow semantics, retain its descriptor from publication admission, require each Git-state check to be bracketed by proof that the live path still names that descriptor, and repeat the identity check through rollback, replay cleanup, final hierarchy synchronization, rollup, and completion publication. Add adversarial root-exchange tests and keep a root mismatch nonpromotable even if the replacement checkout has byte-identical state.
Resolution: each producer or replay controller now opens and retains the admitted root descriptor before its first artifact, Git, source, build, or subprocess access; all production Git checks and nested worker, probe, regression, report, rollup, and completion actions consume the Linux descriptor-root view of that exact directory; and publication independently requires the admitted path to keep naming the descriptor before exchange, during rollback, after cleanup, and at final synchronization. One regression exchanges the repository root during the pre-exchange source checkpoint and proves no artifact is published into either tree, while another swaps in a replacement checkout, proves the nested action reads and validates Git from the retained original, restores it before the outer check, and completes without adopting the replacement. `GOAL.md`, the performance plan, agent instructions, and progress reports now state the descriptor-root and live-root identity contract.

## 2026-07-19 - PQ2: Independent Timing Sample Floor

Status: Resolved
Revealed by: milestone audit of the source-owned independent-throughput qualification mode.
Current text: the plan required each independent implementation to calibrate into a 350-millisecond through 2-second range and also prohibited accepting a sub-floor timed side, without saying whether that prohibition covered only the selected calibration receipt or every later warmup and retained sample.
Gap: applying a hard 250-millisecond floor to every retained sample would turn ordinary runtime jitter into structural report rejection after calibration had already selected an appropriate batch. Applying no floor to the selected calibration receipts would permit unstable tiny batches. The ambiguous wording left both interpretations possible.
Proposed amendment: require each independently selected calibration batch to replay between 350 milliseconds and 2 seconds. Permit later warmup and retained sample durations to jitter outside that range only when they stay positive, finish within the fixed invocation timeout, repeat the frozen iteration count and selected output, and remain subject to the existing paired-noise and `1.25x` rules.
Resolution: `docs/plans/GOAL.md` and the eleventh-slice contract now apply the calibration range only to selected calibration receipts and state the positive-duration, timeout, receipt, noise, and threshold obligations for later samples. The report validator replays both selected calibrations and validates every later receipt without using a per-sample floor that could bias retention.

## 2026-07-19 - PQ2: Clifford Identity Common-Iteration Timing

Status: Resolved
Revealed by: the first clean full-tier diagnostic run of `PERFQ-M6-CLIFFORD-STRING/right-multiply-identity/small` after the reviewed constant-time identity metadata path was restored.
Current text: the qualification program requires one identical iteration count and equal total work for each timed Stim/Stab pair, with standard common batches bounded by 250 milliseconds through 2 seconds and wide-ratio common batches bounded by 250 milliseconds through 20 seconds.
Gap: the identity contract intentionally compares pinned Stim's width-proportional `CliffordString::operator*=` against Stab's semantically equivalent O(1) identity-right fast path. Independent calibration selected a shared maximum of 28,209,671 iterations, at which pinned Stim took 24.145730805 seconds even for the 10,000-qubit small scale and exceeded the hard 20-second ceiling. Using fewer shared iterations leaves Stab below the 250-millisecond measurement floor, increasing the ceiling becomes unbounded at larger widths, and slowing or padding Stab would invalidate the public-lifecycle performance claim. The plan did not define how to qualify a legitimate algorithmic-complexity improvement whose sustained timing ranges do not overlap.
Proposed amendment: add a source-owned `independent-throughput` timing policy only for an exact group whose non-overlap is structurally expected and reviewed. Require independently replayable calibration of each implementation to the existing 350-millisecond target and 250-millisecond through 2-second acceptance range, a common exact semantic preflight at the smaller selected iteration count, per-implementation output validation at each frozen selected count, alternating paired execution, and a normalized ratio of Stab seconds per declared work item divided by Stim seconds per declared work item. Bind both selected iteration and work counts, both output digests, the common semantic receipt, the source-owned policy, and every sample into report replay, regression, rollup, and completion evidence. Keep the `1.25x` threshold, no-waiver rule, exact workload and work-unit identity, and common-iteration policy for every other group.
Resolution: Runtime-group schema version 5 and performance inventory digest `a47866ba5eab70392dd2754391d3d7d8588567a7cbfc1f81a569be813804ce51` select `independent-throughput` only for `PERFQ-M6-CLIFFORD-STRING`; inventory/runtime drift validation rejects substitution. Qualification report schema version 31 preserves the schema-version-30 independent-throughput contract: both independent calibrations, the smaller-count common semantic batch, implementation-specific timing counts and output digests, split work totals, and normalized rates. Runtime and offline replay preserve common-iteration equality for every other group, require a selected calibration output to equal the common semantic output whenever their iteration counts match, reconstruct the selected policy and normalized ratio, and reject zero or over-ceiling common semantic batches, unequal work units, equal-count output mismatch, stale selected receipts, and altered derived evidence. The immutable full-tier diagnostic at `target/benchmarks/qualification/perfq-m6-clifford-identity-independent-schema30-review-final-20260719` remains historical and non-promotable because the checkout was dirty and diagnostic-mode `--allow-unverified-host` was used. Clean pre-migration revision `127d6661a9e00872fc4aa4c0b0d27171e005afa5` completed both exact group chains and authorized the focused timing migration. Clean post-migration revision `91f62d0a78659da2e8e264a6968b3c6cd32456de` then completed and replayed all 12 historical schema-version-30 reports, four rollups, and two completion receipts; identity medians ranged from `0.000145x` to `0.014381x`, with worst identity upper bound `0.014461x`. Clean revision `da7c787d1e9f49110d7054868b146b5fb7d7bda4` later completed a historical schema-version-31 chain under performance inventory `c238dc4e2500192f310ef3d2378ecaafc9744662b5127784dd4eeb6c60726176`, with identity medians from `0.000146x` to `0.014535x` and worst identity upper bound `0.019823x`. Revisions `29a29d5f68767e4ab131b051c88f6b77417e0338` and `0b86f07881198c57df1237b23a7d7c0084f2a272` produced later review-rejected chains. Clean revision `859bf202bdd4bdfbca07e9b1d647afb1b0542846` regenerated and replayed the complete source-current schema-version-31 chain after all confirmed admission, retained-root, artifact-binding, dispatch, cleanup, session-error, and checklist-count fixes; identity medians range from `0.000179x` to `0.017874x`, with worst identity upper bound `0.017927x`.

## 2026-07-18 - CQ2/PQ2: Source-Current Evidence Versus Checkout Replay

Status: Resolved
Revealed by: final milestone audit of the hardened all-domain CQ2 evidence and its performance handoff.
Current text: the execution contract said that a correctness-runner or inventory change makes the all-domain refresh historical, without defining what source-current means after an unrelated commit.
Gap: the milestone did not distinguish scientific acceptance at the artifact's recorded clean producer revision from machine replay in the current checkout. The controller correctly rejects a dirty checkout or different `HEAD`, so the old wording could lead a later performance milestone to reuse broad `3f2f382` receipts directly instead of producing focused exact CQ prerequisites at the same clean revision as its workers. It also did not state whether a documentation-only commit that leaves both inventories and the producer contract unchanged changes the checkpoint's scientific status.
Proposed amendment: define source-current as accepted for the producer and inventory contract at the recorded clean revision, define current-checkout replay as requiring that exact clean `HEAD`, state that documentation-only changes may preserve scientific acceptance while still preventing direct replay, and require every promotable performance run to generate and consume focused exact CQ evidence from its own clean worker revision.
Resolution: `docs/plans/GOAL.md`, `docs/plans/comprehensive-correctness-qualification-plan.md`, and `docs/plans/comprehensive-stim-performance-qualification-plan.md` now define both meanings and the same-revision handoff rule. `docs/plans/cq2-deterministic-qualification-progress-report.md` records a clean schema-7/4 CQ-to-product-performance handoff probe at revision `7d0c07eb62bd455d826714c0c572a586a7e2c548` after schema, passing-receipt, and global statistical-ledger consumer hardening.

## 2026-07-18 - CQ2: Expanded Owner Admission Before Allocation

Status: Resolved
Revealed by: full code review of the first qualification owner-cap hardening change.
Current text: the controller counted expanded word-size owners against the 2,048-owner cap and reused the expanded set during assignment.
Gap: the first fix constructed the expanded vector before enforcing the aggregate cap, so a hostile or accidentally oversized source ledger could still force materialization of the data that admission was intended to reject.
Proposed amendment: validate family shape and use checked arithmetic to compute the complete explicit, family-expanded, public-API, and oracle-fixture owner count before allocating an expansion; allocate at most the admitted upstream count and reuse it during assignment.
Resolution: owner expansion now lives in a bounded module that validates every family and enforces the aggregate limit before `Vec::with_capacity`, while direct 2,049-owner aggregate and oversized-family regressions prove both admission paths fail before expansion.

## 2026-07-18 - PQ2: Clifford Identity Execution Witness

Status: Resolved
Revealed by: milestone audit of the planned eleventh executable performance slice.
Current text: the plan required the identity callback to call public in-place multiplication and black-box the unchanged left operand after the call.
Gap: because both operands and the result are identity and Stab has an identity-right early return, a release optimizer could erase or specialize the call while preserving every planned output field, leaving a timing result dominated by harness overhead.
Proposed amendment: require symmetric per-iteration compiler fences and optimizer-opaque mutable-left and immutable-right references in both workers, retain a post-loop state witness, and freeze the call shape in worker source tests.
Resolution: items 7 through 9 of the eleventh executable slice require the exact fence, opaque-reference, public-call, fence, per-success callback count, result-derived rolling witness, and final-state-witness sequence for both identity and non-identity workers. Callback count and witness reset to zero after setup and immediately before each invocation barrier, so calibration and warmup cannot contribute. The plan freezes literal identity witnesses `0x9e3779b97f4a7c16` after one call and `0x8d6ea9a2cecd4fdd` after two calls; the sixteen-field output and checked vector file freeze all remaining values and forbid a post-call-only barrier, request-derived substitute, or cross-invocation state.

## 2026-07-18 - PQ2: Clifford Canonical Vectors And Rejection Matrix

Status: Resolved
Revealed by: milestone audit of the planned eleventh executable performance slice.
Current text: the plan referred to a canonical 24-gate order, gate-sequence digests, malformed input, and overflow classes without freezing the order, byte encoding, digest framing, exact rejected vectors, or receipt cardinality.
Gap: different valid Clifford permutations or encodings could produce self-consistent but incomparable fixtures, and workers could omit zero-width, unknown-marker, wrong-measurement, malformed-cycle, reserved-field, or semantic-work-overflow branches while still claiming the broad matrix.
Proposed amendment: name the exact pinned Stim order and code assignment, define digest framing and scale tails, require an independently cross-checked checked-in vector file before worker implementation, and enumerate every accepted and rejected vector with an exact receipt total.
Resolution: items 4 and 6 through 10 pin the 24 names and one-byte codes, exact marker and schema constants, SHA-256 framing, scale tails, checked vector file, all 36 complete per-worker requests with exact accepted iteration counts, every descriptor-field rejection trigger, opposite valid workload markers, width-to-work mismatch, malformed descriptor hex, and the precise Stab-then-Stim 72-receipt nesting order. Each field-specific rejection is a checked single-field mutation of one named accepted baseline; the opposite-marker rows deliberately retain a complete valid descriptor for the other workload, and the vector file binds all complete request bytes, accepted outputs or rejection classes, and unconsumed-barrier outcomes so an earlier guard cannot impersonate branch coverage. The former output-field-overflow requirement remains removed because no independently overflowing output field exists after the semantic-work and width bounds.

## 2026-07-18 - CQ1/CQ2: Correctness Artifact Relocation Contract

Status: Resolved
Revealed by: milestone audit of the 271-parent CQ2 report replay and exact preflight.
Current text: the correctness plan required repository-anchored publication and said preflight validates output bindings, but it did not distinguish semantic child-output identity from the report directory's filesystem name.
Gap: documentation could imply that request, report, completion, or preflight bytes bind one canonical `target/qualification/` directory even though a byte-identical artifact can be relocated to another allowed directory and replayed with unchanged digests.
Proposed amendment: define correctness artifact identity as content-addressed and path-independent within the allowed qualification root, keep atomic repository-anchored publication as a write-safety contract, and use `output binding` only for semantic child output and retained artifact content rather than directory provenance.
Resolution: the correctness plan states that byte-identical report trees are intentionally relocatable beneath `target/qualification/`, that repository-anchored publication protects the destination without making it provenance, and that preflight acceptance binds semantic output and result identity. Execution-receipt schema version 4 and report and preflight schema version 7 store retained failure-artifact paths relative to the report root, validate them beneath `cases/<case-id>/`, and include a report-tree relocation regression with retained failure content. The CQ2 progress report no longer claims output-directory binding.

## 2026-07-18 - PQ2: Pauli Iterator Accepted-Maximum State Coverage

Status: Resolved
Revealed by: GPT-5.6/max follow-up review of the tenth PQ2 Pauli-string iterator slice.
Current text: the tenth slice required an independent exact-order owner over three singleton runtime widths and separately required exercise of the 1,048,576-qubit accepted constructor boundary.
Gap: `accepted constructor boundary` permitted the exact owner to construct the maximum-width iterator without traversing its 3,145,728 yielded states. Complete state validation at the 1,000,000-qubit large runtime scale therefore did not cover defects confined to the final 48,576 accepted positions.
Proposed amendment: require complete all-output traversal at 1,048,576 qubits, compare both complete yielded bit planes against independently advanced expected planes at every output, freeze the exact count and sparse sequence digest, and retain typed first rejection at 1,048,577 qubits.
Resolution: the tenth-slice task now states the full accepted-maximum traversal requirement explicitly. `cq2_algebra_pauli_iterator_runtime_contract_matches_independent_reference` includes the accepted maximum in its frozen singleton corpus, validates every X/Y/Z yielded state across all 3,145,728 outputs, and retains the typed first-rejection assertion.

## 2026-07-16 - PQ1/PQ2: Wide-Ratio Common Calibration Batches

Status: Resolved
Revealed by: the first clean calibrated run of `PERFQ-M5-SIMD-BITS-NOT-ZERO-EARLY` after both workers were protected against immutable-input scan elision.
Current text: PQ1 independently calibrates each implementation to a 350-millisecond target within a 250-millisecond to 2-second retained range, then requires one equal-work common iteration count whose measured duration also stays within that same range for both implementations.
Gap: a genuine speed ratio greater than eight has no common iteration count that can keep both implementations between 250 milliseconds and 2 seconds. The early-hit `not_zero` contract exposed this because Stab short-circuits after the bit-600 hit while pinned Stim OR-reduces the complete vector. The 10,000-bit shared batch made Stim take 4.90 seconds while keeping Stab above the noise floor, and the 640,000-bit batch later took 10.17 seconds. A provisional 10-second cap therefore encoded the first observation instead of the workload's plausible ratio range.
Proposed amendment: retain the 350-millisecond target and 2-second ceiling for each independent calibration. Add a source-owned wide-ratio common-batch mode that preserves identical iterations, fixture identity, work count, output digest, and paired statistics; requires both measured durations to remain at least 250 milliseconds; permits only the implementation whose independent calibration selected fewer iterations to exceed 2 seconds; requires the other implementation to remain at or below 2 seconds at its selected common iteration count; and rejects either side above a hard 20-second common ceiling under the existing 30-second invocation timeout. The 20-second cap covers the approximately 40x ratio implied by the 6-percent early-hit position combined with the measured full-scan implementation advantage, plus margin, instead of fitting one observed scale. Record and offline-rederive the mode instead of trusting report-owned classification.
Resolution: qualification report schema version 23 records the standard or wide-ratio common-batch mode and both the 2-second independent and 20-second wide-ratio ceilings. Runtime and offline report validation derive the mode from the two independently replayed calibration decisions and common-validation receipts, reject floor, cap, equal-iteration, wrong-owner, and both-over-standard violations, and retain identical semantic work for every timed and memory pair. `GOAL.md`, the performance qualification plan, and benchmark operations documentation now state this bounded exception explicitly.

## 2026-07-16 - PQ2: Completion-Command Receipt Boundary

Status: Resolved
Revealed by: milestone audit of the fifth PQ2 dense-XOR qualification slice.
Current text: the fifth slice requires standalone worker reproducibility, an adapter probe, immediate source-report replay and regression checks, rollup replay, milestone audit, and GPT-5.6/max full code review before completion; `GOAL.md` requires a prose progress report that records the commands and outcomes.
Gap: neither plan requires one machine-readable completion receipt that binds each standalone command, exact revision, input artifact, result, and output digest. The prose report can record execution, but the normal report and rollup schemas cannot independently attest the complete closure sequence.
Proposed amendment: before the next executable PQ2 slice, define a canonical milestone-completion receipt that records each required standalone command as a typed step with the clean revision, canonical arguments, input identities, exit status, and produced artifact digests; validate the receipt during progress-report or rollup publication without making human review mechanically self-certifying.
Resolution: `just bench::qualification-completion` now executes typed worker-reproducibility, source-owned adapter-probe, report-replay, regression, and rollup-replay handlers from one clean unchanged revision. Completion receipt schema version 1 records canonical standalone argument vectors, exact input and output artifact identities, command-equivalent zero exit status only after successful handler return, and typed results; it rejects non-idempotent repair, mixed exact CPU or worker identities, incomplete gates, failed rollups, and source replacement before atomic publication. Shared artifact publication retains descriptors for every validated source directory and replay target, rechecks their inode identities at the commit boundary together with the parent, staging, target, and replaced directories, makes the new directory durable before treating bounded old-directory cleanup as best effort, and has direct production-path regression coverage for byte-identical directory replacement and cleanup failure. A controllable workflow harness proves exact handler order, first-error termination, and non-idempotent live replay rejection. `just bench::qualification-completion-report` reruns every machine-checkable operation and requires byte-identical receipt and preflight reconstruction. The performance plan and benchmark operations documentation explicitly keep milestone audit and independent code review outside the machine-certified receipt. The requirement applies to closure claims made after schema version 1 was introduced; historical dense-XOR evidence predates this receipt and is not retroactively relabeled.

## 2026-07-16 - PQ2: Allocation-Free Comparator Proof Standard

Status: Resolved
Revealed by: milestone audit of the fifth PQ2 dense-XOR qualification slice.
Current text: the fifth slice says that the timed mutation must allocate zero bytes and requires an allocation-counter test, while the source-owned performance reports record process RSS for both workers.
Gap: the plan does not state whether zero-allocation proof must cover Stab only or both implementations, whether it must execute at every scale, or whether pinned Stim source inspection is an acceptable proof for the C++ comparator. The implemented executable check instruments Stab at the medium scale, while Stim's no-allocation body is supported by the pinned comparator source.
Proposed amendment: state explicitly which implementations and scales require allocator instrumentation, which source-inspection evidence is acceptable for a comparator implemented by in-place operators, and whether the zero-allocation invariant belongs to correctness preflight, report replay, or PQ6 allocation qualification.
Resolution: `docs/plans/comprehensive-stim-performance-qualification-plan.md` and `docs/plans/GOAL.md` now classify zero-allocation evidence explicitly. Stab timed-body claims require allocator instrumentation at every source-owned runtime scale and the accepted maximum, with setup and inspection excluded. Pinned Stim source inspection establishes only the isolated comparator shape and cannot produce a Stim allocation claim; a future cross-implementation claim requires C++ allocator instrumentation. Process RSS stays separate and PQ6-owned. `dense_xor_timed_mutation_allocates_nothing` now checks small, medium, large, and accepted-maximum widths under `count-allocations`.

## 2026-07-16 - PQ2: Pinned Stim Header Warning Boundary

Status: Resolved
Revealed by: direct compilation of the exact `PERFQ-M5-SIMD-BITS` adapter comparator under the source-owned warning policy.
Current text: the fifth PQ2 slice required the literal pinned `destination ^= source` operation and required the standalone adapter to retain strict warning enforcement, but it did not specify how warnings originating inside pinned Stim headers should interact with adapter-owned `-Werror`.
Gap: GCC emits `-Wdeprecated-copy` while instantiating Stim v1.16.0's `simd_bits::operator^=` from the pinned header. A call-site pragma cannot suppress a diagnostic attached to the template definition, globally downgrading that warning would also weaken adapter-owned code, and replacing the operator would violate the exact comparator contract.
Proposed amendment: compile the pinned Stim include root as an external system-header path while preserving CMake's resolved `libstim` flags and retaining `-Wextra -Werror` for adapter-owned code; bind the exact compile arguments into the adapter receipt and build fingerprint.
Resolution: `ops/bench` now emits `-isystem $STIM_SOURCE/src`, retains `-Wextra -Werror`, and tests both properties alongside preservation of CMake-resolved machine flags. The exact isolated comparator still executes `destination ^= source`; direct compilation against pinned `libstim` produces the frozen 4,096-bit input and output digests. Adapter receipt schema version 5 binds the ordered comparator-source collection and compile arguments, contract-preflight schema version 4 records 30 worker receipts, and qualification report schema version 19 rejects the prior embedded receipt shape.

## 2026-07-16 - PQ1/PQ2: Adapter Machine Flags And Mandatory Contract Preflight

Status: Resolved
Revealed by: GPT-5.6/max full code review of the corrected `PERFQ-M5-SIMD-WORD` comparator and evidence path.
Current text: private-worker reproducibility bound adapter tools and canonical compiler arguments and exercised popcount boundaries, but the standalone adapter reconstructed generic compiler flags instead of inheriting CMake's resolved `libstim` machine flags. Canonical vectors and boundary probes ran only in the separate reproducibility command, so a normal promotable qualification run did not record proof that they ran.
Gap: on x86-64, pinned Stim CMake can compile `libstim` and `stim_perf` with `-march=native`, selecting a wider `MAX_BITWORD_WIDTH`, while the adapter headers could compile under the default SSE2 width. Two workers with a shared encoding defect could also pass pairwise semantic preflight when canonical independent probes were omitted from the normal run path.
Proposed amendment: derive the adapter's compile flags from CMake's generated `libstim` target flags, record and fingerprint the exact ordered list, require the complete canonical worker preflight while preparing every worker pair, and bind the actual probe receipts and their recomputed digest into report and rollup identity.
Resolution: adapter receipt schema version 4 records CMake's generated `libstim` compile flags, the isolated comparator-source digest, and the derived standalone compile command and fingerprint. Qualification report schema version 18 records all 18 actual worker-contract probe receipts and includes both workers' exact source, build-fingerprint, and binary identities in their recomputed preflight digest; rollup schema version 4 reopens and validates every source report before accepting that shared digest. Every `PreparedWorkers::prepare` executes the shared frozen protocol vector, odd and even popcount vectors, actual accepted-maximum popcount, circuit and gate rejection probes, and all three popcount rejection classes before calibration. Offline replay reconstructs the exact source-owned receipt list, cross-checks the six preflight worker identities against the report workers, and rejects missing, reordered, altered, refingerprinted, stale, or cross-worker-transplanted evidence.

## 2026-07-16 - PQ2: Popcount Comparator And Output Contract Precision

Status: Resolved
Revealed by: milestone audit of the first `PERFQ-M5-SIMD-WORD` implementation and clean full-tier evidence.
Current text: the fourth PQ2 slice said that the workload was derived from `simd_compat_popcnt`, timed a toggle and complete-vector popcount, bound the checksum and fixture fingerprint into an output digest, and tested the accepted maximum.
Gap: the milestone did not name the exact architecture-dependent Stim loop, so the adapter could call `simd_bits::popcnt()` instead of iterating `ptr_simd[k].popcount()` and still appear compliant on AArch64 where `MAX_BITWORD_WIDTH` is 64. It did not define field order, byte order, digest algorithm, or fixed vectors for the output identity, so three of four fingerprint lanes could be omitted without a failing test. It also did not say that accepted-maximum evidence required real construction and execution in both sealed workers, and its generic adapter test accidentally called every workload output a circuit digest.
Proposed amendment: require the literal pinned `simd_compat_popcnt` SIMD-word loop, define an eight-field little-endian `u64` output encoding followed by the shared four-lane byte digest, add fixed odd and even output vectors, execute both sealed workers at the accepted maximum, exercise below-minimum, unaligned, and over-cap pre-barrier rejections, and use workload-neutral semantic-output wording.
Resolution: the fourth executable slice and tests in `docs/plans/comprehensive-stim-performance-qualification-plan.md` now define those contracts explicitly. The C++ adapter uses an isolated `ptr_simd[k].popcount()` implementation, and runtime-group schema version 4 binds both its call site and implementation path and SHA-256 through the generated inventory, materialized adapter receipt, invocation, and report replay. Both workers construct the complete output identity after timing, Rust tests bind fixed odd and even vectors and execute the accepted maximum, private-worker reproducibility exercises both sealed workers at that maximum and at all three rejection classes, and `benchmarks/stim_adapter/README.md` documents the exact encoding and timing boundary.

## 2026-07-15 - PQ2/PQ6: Programmatic Circuit Serialization Depth

Status: Resolved
Revealed by: GPT-5.6/max review of the canonical-printer qualification slice.
Current text: the PQ2 canonical-print group owns a parsed flat fixture family, while the shared one-million-instruction accepted boundary and first rejected input are assigned to PQ6. The parser separately rejects repeat nesting beyond 256 levels.
Gap: the plans do not define a resource contract for public circuits constructed programmatically with more than 256 nested `RepeatBlock` values. Those values bypass parser admission and currently recurse during capacity calculation, string or file emission, and destruction, so sufficiently deep values can exhaust the process stack. The flat PQ2 timing result does not exercise or claim this behavior.
Proposed amendment: make CQ6/PQ6 select one explicit public contract before deep programmatic circuits are qualified: either implement iterative serialization and destruction with a declared bounded-work policy, or introduce a fallible depth-checked construction or serialization boundary. Add maximum-accepted and first-rejected depth tests for string and file output, early writer failure, and drop behavior, and keep the flat PQ2 performance claim separate from this resource evidence.
Resolution: `docs/plans/comprehensive-correctness-qualification-plan.md` now requires an independently selectable CQ6 programmatic-repeat resource case, and `docs/plans/comprehensive-stim-performance-qualification-plan.md` assigns the implementation choice, depth boundary, string and file output, writer failure, destruction, and scaling evidence to PQ6. The work remains planned; the under-specification is resolved without extending the flat PQ2 timing claim.

## 2026-07-15 - PQ2: Worker Identity Across Scale Families

Status: Resolved
Revealed by: GPT-5.6/max performance review of the first scale-family rollup implementation.
Current text: PQ2 required one commit, toolchain, host, correctness preflight, runtime contract, and complete scale set across a full or soak rollup.
Gap: the milestone did not require every scale report to execute the exact same Stim and Stab worker binaries. Reports built from different source, build recipes, or executable bytes could therefore be aggregated even though their repository and toolchain identities matched.
Proposed amendment: bind all Stim and Stab worker source digests, build fingerprints, and binary digests into the shared rollup identity; require private builds to be byte-reproducible; and add an operational reproducibility check plus mixed-worker rejection tests.
Resolution: rollup schema version 3 carries the complete six-digest worker identity and rejects any cross-scale mismatch. The private Stab receipt hashes the worker source from the materialized commit, removes embedded temporary repository paths and nondeterministic symbol and linker build-id sections, and rechecks the source after building. `just bench::qualification-worker-reproducibility` requires a clean unchanged commit before rebuilding both private workers twice, requires each binary to confirm its receipt source and build identity through a bounded protocol handshake, and rejects any identity difference between builds. Rollup tests reject a changed worker binary digest. The PQ2 plan, benchmark documentation, and active goal now require exact worker identity across every accepted scale family.

## 2026-07-15 - PQ2: Rollup Producer And Replay Identity

Status: Resolved
Revealed by: GPT-5.6/max security review of the first scale-family rollup implementation.
Current text: PQ2 required architecture-scoped full and soak rollups to reopen every scale report, bind their shared identity, and fail closed on incomplete, stale, mixed, or nonpromotable source evidence.
Gap: the milestone did not require the rollup producer checkout itself to be clean, unchanged, and equal to the source reports' Stab commit, and it did not define an offline validator that reconstructs the rollup from its exact source artifacts. A rollup could therefore be derived by uncommitted code or remain superficially valid after its report and preflight were edited together.
Proposed amendment: record a separate clean producer repository tuple, require its commit to equal every source report, restrict rollup artifacts to bounded canonical direct-sibling paths, and add offline replay that reloads current source contracts and exact source report and preflight digests, reconstructs all canonical bytes, rejects hostile mutations, and republishes only through compare-and-swap checks.
Resolution: rollup schema version 3 retains the producer repository state and output binding introduced by schema version 2 and adds exact worker identity; `just bench::qualification-rollup` rejects dirty, changed, or source-mismatched producer revisions and bounds source reads; `just bench::qualification-rollup-report` reconstructs the complete rollup and preflight from current source-owned evidence before atomic refresh; adversarial tests cover noncanonical and oversized bytes, stale preflight, altered source paths and digests, altered timing and memory outcomes, altered aggregate and producer fields, unsafe artifact names, source replacement, and producer-state drift. The PQ2 plan and active goal now require successful replay for every accepted full and soak family.

## 2026-07-15 - PQ2: Timing Scales Versus Materialization Boundary

Status: Resolved
Revealed by: milestone audit of the first `PERFQ-M4-CIRCUIT-PARSE` implementation and three-scale evidence.
Current text: the first PQ2 slice required 64, 4,096, and 65,536-instruction timing scales, capped both workers at 1,000,000 instructions, and required rejection of the first unsupported count.
Gap: the milestone did not say whether the first timing slice also had to materialize and measure the maximum accepted 1,000,000-instruction fixture and the 1,000,001-instruction rejection. Treating the 65,536-instruction timing scale as cap evidence would leave the resource contract unproved, while adding the cap boundary to the paired timing family would mix resource admission with representative throughput.
Proposed amendment: keep the three representative scales as PQ2 timing and process-RSS evidence, retain worker-level preallocation rejection tests, require the clean private-worker check to invoke the first unsupported scale through both sealed binaries with no start-barrier input, and assign maximum-accepted materialization plus first-rejected resource evidence to PQ6 outside the timing gate.
Resolution: `docs/plans/comprehensive-stim-performance-qualification-plan.md` now makes that split explicit. The first slice binds exact inputs and memory evidence at 64, 4,096, and 65,536 instructions and makes its clean private-worker check prove that both sealed binaries reject 1,000,001 instructions before the start barrier. PQ6 owns the 1,000,000-instruction accepted materialization plus complete resource-report evidence for the accepted and first-rejected boundaries.

## 2026-07-15 - PQ2: Scale-Family Aggregate Evidence

Status: Resolved
Revealed by: milestone audit of independently published small, medium, and large circuit-parse reports.
Current text: PQ2 required three scales where applicable and clean full and soak evidence, but accepted separate report artifacts without defining one aggregate completeness check.
Gap: three individually valid reports do not prove that a reviewer selected exactly one current, promotable report for every required scale on one architecture. A stale, missing, duplicated, cross-commit, cross-inventory, or cross-architecture scale could be overlooked while each remaining report still validates independently.
Proposed amendment: require separate source-owned full and soak architecture-scoped scale-family rollups that reopen every report, bind one commit, Stim revision, correctness and performance inventory, runtime contract, host architecture, tier policy, and complete required scale set, and fail closed on missing, duplicate, stale, nonpromotable, or foreign evidence. Keep AArch64 and x86-64 rollups separate and combine only their conclusions in PQ7.
Resolution: the PQ2 tests and acceptance criteria in `docs/plans/comprehensive-stim-performance-qualification-plan.md` now require both rollups before PQ2 completion. `just bench::qualification-rollup` implements the fail-closed report and preflight binding, the active goal requires the first AArch64 full and soak rollups for the circuit-parse slice, and native x86-64 evidence remains explicitly unclaimed until a controlled x86-64 host produces it.

## 2026-07-15 - PQ2: Product PR Evidence Versus Promotion

Status: Resolved
Revealed by: implementation of the first correctness-bound product runtime group.
Current text: the performance plan requires PR, full, and soak qualification tiers, exact correctness preflight before product timing, and clean verified full evidence for promotable claims.
Gap: the PQ1 report validator required every `promotable-performance` group report to have `promotable=true`. A PR-tier product report can never satisfy the full-or-soak promotion rule, so the validator made the required PR product tier structurally impossible even when its correctness, work, output, and host evidence were valid. The plan did not explicitly distinguish a product-class diagnostic report from a promoted product claim.
Proposed amendment: derive promotion from claim class, tier, repository cleanliness, host verification, and exact correctness preflight. Permit a product PR report to validate with `promotable=false`, require clean verified full or soak reports to record `promotable=true`, and keep regression dispatch fail-closed for nonpromotable product reports.
Resolution: report schema version 14 derives promotion from the complete evidence tuple. Product PR, dirty, and unverified-host reports remain nonpromotable; exact correctness preflight remains mandatory for all product tiers; and `qualification-regression` continues to reject nonpromotable product evidence. The active goal and performance plan now state the distinction.

## 2026-07-15 - CQ2 Algebra Deterministic Materialization Admission

Status: Resolved
Revealed by: exact `CQ-ALGEBRA` exported-API ownership reconciliation.
Current text: the qualification goal requires explicit cap acceptance and first-rejection evidence for materialized and bounded-search APIs, while M6 declares owned Pauli, Clifford, Tableau, iterator, stabilizer-solving, and scoped unitary-conversion APIs complete.
Gap: `PauliString::identity`, `PauliString::from_bases`, `FlexPauliString::identity`, `CliffordString::identity`, `CliffordString::from_gates`, `Tableau::identity`, `PauliStringIterator::new`, and `Flow::new` were infallible size-driven or iterator-driven constructors. Several paths allocated or performed quadratic or exponential work from caller-supplied sizes, so the APIs could not provide a deterministic typed first-rejection contract. `CliffordString::repeat` also accepted an empty string with an arbitrarily large repetition count and performed a pointless unbounded loop because its projected length remained zero. `Circuit::to_tableau` expanded compact repeat counts linearly and did not convert valid `SPP` or `SPP_DAG` instructions.
Proposed amendment: define separate source-owned limits for Pauli and Clifford materialization, aggregate Flow classical terms, dense Tableau materialization, lazy iterator state, stabilizer solving, circuit-to-Tableau conversion, random Tableau construction, and unitary-matrix conversion. Make public materializing constructors fallible or introduce validated typed counts that eliminate invalid states. Explicit-size requests must reject before allocation or output mutation; unknown-length iterable requests must stop at the first excess item, keep local allocation within the cap, and return no partial output. Preserve all pinned selected cases including the 500-qubit Tableau regression and 65,536-qubit sparse Pauli use, and add exact last-accepted or representative accepted plus first-rejected tests for every limit family. Fix zero-length repetition so work is constant regardless of repetition count, and require compact circuit repeats to use logarithmic Tableau composition instead of expansion.
Resolution: `StabilizerResource` now owns separate Pauli, Clifford, aggregate Flow classical-term, dense Tableau, compact-repeat Tableau work, random-Tableau, stabilizer-solver, and unitary-dimension limits. Explicit-size constructors and projected growth reject before allocation, iterable construction stops at the first excess item without growing beyond the cap or returning partial output, and random construction rejects before RNG use. Flow construction and parsing share a 65,536 aggregate measurement-plus-observable-term cap, and Flow multiplication performs bounded symmetric-difference cancellation before rejecting an oversized result while preserving Stim's signed-input representation convention. Annotation-only identity-flow generation has an aggregate output-bit budget, empty Clifford repetition is constant-work, and `Circuit::to_tableau` handles `SPP`, `SPP_DAG`, and compact repeats through one-body conversion plus identity-aware binary exponentiation under a 16,777,216-unit width-squared work budget. `crates/stab-core/tests/cq2_algebra_resources.rs` proves accepted representatives, first rejection, first-extra-item consumption, overflow, circuit-derived admission, logarithmic and bounded repeat work, RNG non-consumption, and precedence over later work. The roadmap, checklist, M6 report, CQ2 report, and active goal record the contract.

## 2026-07-14 - CQ2: Mixed Aggregate Count Semantics And Overflow

Status: Resolved
Revealed by: milestone audit and exact-symbol review of `CQ-CIRCUIT-API`.
Current text: CQ2 required exact upstream-symbol dispositions, focused parent ownership, overflow negatives, and splitting aggregated tests at distinct behavioral anchors.
Gap: Pinned C++ aggregate count symbols combine portable ordinary and folded-count behavior with `UINT64_MAX` saturation, while Stab's selected Rust count methods return checked `Result` errors. The inventory schema addresses complete upstream symbols rather than individual assertions, so mapping the aggregate to a Rust parent would falsely claim saturation parity, but dropping the aggregate would hide a reviewed incompatibility and could also obscure the shared portable behavior.
Proposed amendment: When a complete upstream symbol mixes selected portable semantics with incompatible language-specific or API-specific behavior and has no independently addressable subcase, disposition the complete symbol according to its complete contract and own the shared portable semantics through an independent exact Rust parent. Require the selected Rust overflow contract to be explicit in qualification evidence and the feature checklist.
Resolution: The CQ2 plan now states that rule. The mixed C++ count aggregates are `not-applicable` with precise checked-overflow reasons, focused Circuit count and repetition parents prove ordinary, folded, huge-count, cap, and first-overflow behavior, and `docs/stab-feature-checklist.md` records checked `Result` overflow as an intentional Rust API divergence instead of an unfinished Stim parity surface.

## 2026-07-14 - CQ2: Portable Bit Semantics Versus C++ Storage Helpers

Status: Resolved
Revealed by: implementation and exact-symbol audit of `CQ-BIT-KERNELS`.
Current text: CQ2 named dense bits, bit tables, transposition, parity, random fill, and sparse XOR, and required every relevant upstream case to receive an exact disposition.
Gap: The pinned memory tests mix portable Boolean, tail, row, transpose, and sparse-XOR semantics with C++ moved-from state, mutable aliasing, destructive and preserving resize, padded lane layout, arithmetic, shifts, raw random fill, table concatenation, triangular inverse, and predicates that Stab neither exposes nor uses. The plan did not say whether qualification must invent public Rust APIs for those C++ internals, mark them deferred, or classify them as language-specific implementation details. Treating the whole files as selected would recreate C++ storage design in Rust and inflate parity claims; dropping the rows silently would make the inventory incomplete.
Proposed amendment: Select only portable upstream semantics that map to an existing Stab Rust API or selected engine contract, give every remaining exact symbol a reviewed `not-applicable` reason, and keep typed caller-owned randomization with the Pauli, Clifford, and Tableau APIs that expose it. Require deterministic scalar and dense-versus-sparse corpora to span zero width, unaligned tails, word and portable-SIMD lane boundaries, large widths, overlap policy, and allocation or mutation boundaries.
Resolution: Exact memory classification now selects 82 portable upstream records and marks 168 C++-specific records not applicable. Eight focused qualification parents assign every selected record and all 274 exported Rust API items, four exact M5 fixture parents remain independent evidence, and four broad M5 fixtures attach as supporting provenance. Deterministic scalar, dense-versus-sparse, self-overlap, and allocation-counter checks cover the selected Rust mutation contracts, including a fixed self-masked-row allocation defect. `docs/plans/comprehensive-correctness-qualification-plan.md` and the performance plan now state the same boundary; typed randomization remains in `CQ-ALGEBRA` and `PERF-STABILIZER-ALGEBRA`.

## 2026-07-14 - CQ2: Gate Ownership, Comparator Refinement, And Word-Size Families

Status: Resolved
Revealed by: implementation and exact-owner audit of the gate-contract qualification slice.
Current text: CQ2 required exact upstream provenance, one independently selectable exact parent, same-feature and same-comparator mappings, and semantic review instead of file-level aggregation.
Gap: Broad simulator-file classification assigned graph, vector, frame, tableau, and analyzer implementation tests to the gate domain even when they owned deferred simulator APIs or non-gate behavior. The generated feature-level comparator was only a provisional discovery hint, but the ledger could not map noisy gate rows to their canonical statistical parents. The ledger also required duplicate qualification parents instead of reusing implemented blocker, oracle, or regression parents, and it had no exact source-owned shorthand for the `_64`, `_128`, and `_256` expansions of one upstream family. Those loopholes inflated the gate inventory, encouraged duplicate terminal selectors, and could let one nearby semantic test claim a differently shaped contract.
Proposed amendment: Classify mixed simulator sources by exact symbol; defer public graph, vector, and Python products with their product boundary; let the reviewed exact parent own the final comparator; allow same-feature mappings only to implemented or evidence-close canonical blocker, oracle, or Rust-regression parents; and expand word-size families only from explicit validated 64-bit, 128-bit, and 256-bit member lists.
Resolution: Schema-version-2 `oracle/qualification-cases.json`, exact-symbol classification, and cross-reference validation now enforce those rules. The gate domain closes with 37 reused blocker-ledger parents, 14 direct oracle-fixture parents, and eight focused qualification parents; all 178 selected APIs and 340 non-deferred upstream records are assigned, 200 simulator or Python records remain explicitly deferred, and focused regressions separately exercise noisy measurement-only and measure-reset gates plus Pauli-target observables.

## 2026-07-14 - CQ2: Mixed Language-Specific Upstream Contracts

Status: Resolved
Revealed by: exact-symbol review of the selected `.dem` upstream tests.
Current text: CQ2 required exact upstream provenance and selected Rust API ownership, but it did not state how to classify one upstream symbol that combines portable semantic behavior with Python object shape or C++ convenience behavior outside Stab's selected Rust surface.
Gap: Treating the entire symbol as selected would silently add Python copying, indexing, operators, file helpers, or C++ moved-from and convenience-operator behavior to the Rust contract. Deferring the entire symbol would also hide portable target, instruction, model, coordinate, traversal, or transform semantics that Stab does select.
Proposed amendment: Disposition each exact upstream symbol according to the contract actually expressed by that symbol, defer or mark non-applicable language-specific symbols honestly, and give the selected Rust API an independent qualification parent when the portable semantic contract is not independently selectable upstream.
Resolution: `.dem` classification now defers Python-only object-shape and file-helper symbols with Python bindings, marks C++ convenience and moved-from behavior not applicable, moves shortest-graphlike Python symbols to `CQ-SEARCH`, and maps selected Rust target, instruction, model, traversal, coordinate, and transform APIs through exact independent parents.

## 2026-07-14 - CQ2: Broad Imported Fixtures As Supporting Provenance

Status: Resolved
Revealed by: exact-parent ownership review for imported `.dem` structural fixtures.
Current text: CQ2 allowed qualification parents to replace planned owners and CQ0 retained broad fixture filters as supporting evidence, but it did not explicitly define how an imported broad fixture row should attach to a new exact qualification parent.
Gap: Promoting the broad fixture row would make a file-level or multi-case filter look atomic. Keeping both the fixture and qualification parent as terminal primaries could duplicate completion credit or violate the shared-primary prohibition.
Proposed amendment: Require broad imported fixture rows to remain supporting-only provenance and name their exact qualification owner through `oracle_fixture_owners`; reject missing fixture ownership, shared terminal primaries, and broad selectors promoted as atomic evidence.
Resolution: The `.dem` qualification ledger claims the affected fixture ids through `oracle_fixture_owners`, generation removes their standalone planned evidence rows, and inventory tests prove that the broad folded-traversal fixture remains supporting provenance rather than an atomic primary. An imported exact fixture may use the same path only when its normalized exact Cargo selector is identical to the qualification parent's primary selector; the fixture then remains supporting provenance and cannot become a duplicate terminal primary.

## 2026-07-14 - CQ2: Exact-Symbol Classification And Complete Parent Credit

Status: Resolved
Revealed by: semantic review of the first `.stim` exact-parent slice after its initial source mappings regenerated cleanly.
Current text: CQ2 had an exact-parent ledger, but file-level relevance still assigned all of mixed `circuit.test.cc` to both format and circuit-API domains, and nearby or name-similar tests could be proposed as evidence for broader upstream symbols.
Gap: File-level relevance and test-name proximity could promote contracts that the selected test did not prove. The review found examples involving a nominal `qubit_coords` case that actually tests `TICK` rejection, broad `from_text` ownership, tag escaping through nested repeats, probability-list validation, target text and accessor matrices, gate-target ordering, Python-only instruction construction, and circuit measurement-count overflow behavior that differs from Stim.
Proposed amendment: Classify mixed source files and overloaded semantic families by exact upstream symbol, require focused tests to exercise every claimed contract edge, split gate equality and instruction value/count ownership into their actual domains, and leave incompatible or untested API contracts planned.
Resolution: Exact-symbol classification now distinguishes circuit format, gate-target equality, instruction value/count, and Python-only constructor cases. Focused `.stim` tests cover full target, tag, validation, fusion, repeat, coordinate, and instruction-print contracts; two focused instruction tests cover only the exact value and per-instruction count methods they exercise. `Circuit::count_measurements` and untested derived traits remain planned, and semantic review fixed inverted Pauli-target admission for `CORRELATED_ERROR` and `ELSE_CORRELATED_ERROR` instead of granting evidence over the defect.

## 2026-07-14 - CQ2: Exact Parent Mapping Ownership

Status: Resolved
Revealed by: CQ2 inventory audit before deterministic format implementation.
Current text: CQ2 required every deterministic upstream case and exported API item to have an exact selector or parent-contract mapping, but CQ0 generated one planned evidence owner per source anchor and provided promotion paths only through blocker-ledger and oracle-fixture records.
Gap: The plan did not define a source-owned, reviewable way to map several exact upstream or public-API owners onto one independently selectable Rust test, nor did it require validation against stale owners, duplicate claims, cross-feature claims, comparator mismatches, broad selectors, or reused terminal primaries. Implementing thousands of wrapper tests would obscure contracts, while silently deleting generated owners would make the inventory unauditable.
Proposed amendment: Add a bounded exact-parent ledger whose entries name every claimed source owner, one feature and comparator, and one exact primary selector; require deterministic generation to replace only matching planned owners and fail closed on stale, duplicated, cross-domain, comparator-mismatched, or shared-primary mappings.
Resolution: `oracle/qualification-cases.json` and its validator now own this contract, the CQ2 plan requires semantic review and focused test splitting before promotion, and the completed selected `.stim` slice uses 24 qualification parents to map 44 exact upstream owners and nine exported-API owners, with eight direct oracle-fixture parents completing the domain. A duplicate selector and an under-proven typed-offset mapping were caught and corrected during the first use of the mechanism.

## 2026-07-14 - PQ1: Legacy M12 Backward Compatibility

Status: Resolved
Revealed by: PQ1 milestone audit and fresh execution of the 89-row M12 beta, timing-regression, and memory-regression commands.
Current text: PQ1 required `just bench::smoke` and existing M12 commands to remain backward compatible.
Gap: The criterion did not distinguish command, schema, and gate compatibility from an expectation that every inherited product row must pass before the independent diagnostic harness could be accepted. Fresh evidence had six non-comparable timing blockers, four unconfigured timing rows, four missing memory baselines, and two memory failures even though PQ1 did not change their product runners or gates.
Proposed amendment: Define PQ1 compatibility as preserved command parsing, execution, report shapes, threshold and waiver files, and failure semantics. Require inherited failures to remain visible and assign their replacement or graduation to PQ2 through PQ6 instead of treating them as either PQ1 harness defects or ignorable successes.
Resolution: The PQ1 acceptance criteria and `docs/plans/pq1-performance-harness-progress-report.md` now define that boundary and record the exact 89-row outcomes. No inherited threshold, waiver, missing baseline, or memory failure was removed to make PQ1 pass.

## 2026-07-13 - CQ1: Executable Provenance And Build Isolation

Status: Resolved
Revealed by: GPT-5.6/max full-code review of CQ1 execution receipts and fixture runners.
Current text: CQ1 required exact selectors, exact commits, canonical receipts, and pinned Stim compatibility but did not define which executable bytes were trusted or whether shared build outputs could be reused.
Gap: A mutable cached Stim or Stab binary, a replaced worker path, or a different Cargo or host-tool executable could produce internally consistent request and result receipts without appearing in the evidence contract.
Proposed amendment: Require a fresh private Stab and Stim build for each qualification run, a canonical role/hash/size identity ledger for every direct build and execution tool, immutable execution through sealed Linux descriptors, the same ledger and hashed environment in every case receipt, and private config-free homes instead of unconstrained inheritance.
Resolution: CQ1 now validates Stim source metadata before building, creates private Release binaries, copies direct and compiler-subordinate executable bytes into sealed memory files before use, freezes the canonical direct-tool ledger and execution-environment digest in `request.json`, repeats both in every schema-version-3 execution receipt, allocates fixed private runtime state under `/tmp`, invokes Cargo from `/` with absolute manifests and private configuration, reconstructs a config-free private Git index from `HEAD`, reuses only dependency caches, snapshots compiler and CMake support trees into read-only content-bound directories, revalidates those snapshots after compiler use and before publication, binds their digests into the explicit environment, and performs bounded descriptor-owned cleanup when the private runtime is dropped.

## 2026-07-13 - CQ1: Statistical Multiplicity And Completion Credit

Status: Resolved
Revealed by: GPT-5.6/max statistical review of blocker-ledger and oracle-fixture qualification accounting.
Current text: CQ1 required fixed shots, exact boundaries, familywise budgets, and per-seed attempt outcomes but did not define how one source plan accounts for several independent bucket checks, two compared implementations, or several shot-producing calls.
Gap: A plan could understate its union bound and planned shot total, while a nonzero process exit or malformed short output could be mistaken for proof that all declared shots completed.
Proposed amendment: Store independent comparisons and shot batches per attempt in source-owned plans, multiply exact tail bounds and shot totals by those counts, credit each compared side only from exact structurally valid records, retain exactly completed sides on failed attempts, and require every declared side and batch before an attempt can pass.
Resolution: Blocker-ledger schema version 3 and the shared gate statistical plans now carry both multiplicities and reject shot batches that do not divide evenly across comparisons, fixture plans explicitly account for Stim and Stab, canonical integer boundaries are shared by validation and execution, fixed Cargo tests emit plan, seed, comparison, and exact per-comparison shot completion markers after structural completion but before probabilistic acceptance, malformed marker suffixes fail while retaining only a validated prefix, two-sided fixtures run both sides and retain exact partial completion, and report schema version 6 plus execution-receipt schema version 3 cross-check the same batch shape and per-attempt work. Probability bounds use a canonical 17-digit string representation so exact consumed-bound validation remains stable after JSON round trips.

## 2026-07-13 - CQ1: Descriptor Ownership And Derived-Report Transactions

Status: Resolved
Revealed by: GPT-5.6/max filesystem review of staged evidence, report regeneration, and previous-run cleanup.
Current text: CQ1 required safe artifact paths and atomic directory publication but did not state whether artifact operations had to remain bound to the originally opened directory or whether derived-report regeneration shared the publication transaction.
Gap: Reopening staged or published artifacts by path permitted directory-identity swaps, report regeneration could race a concurrent publication, and recursive cleanup under the publication lock had no explicit depth or entry bound.
Proposed amendment: Perform staged and locked report I/O relative to held directory descriptors, anchor the publication lock and output traversal to the retained repository directory, verify every output-parent component before publication and before releasing a report lock, release the publication lock before cleanup, and make cleanup iterative and bounded without turning a post-publication cleanup failure into evidence failure.
Resolution: CQ1 now uses descriptor-relative atomic artifact I/O, a repository-anchored publication lock, identity-checked parent-chain traversal for publication and report or preflight operations, post-lock quarantine cleanup, and explicit depth and entry bounds with target, parent-chain, symlink, and deep-tree regressions.

## 2026-07-13 - CQ1: Static Property Corpus Exemption

Status: Resolved
Revealed by: independent milestone audit of the CQ1 generated-property worker.
Current text: The manifest allowed implemented property evidence to use either a generated seed panel or a frozen static corpus, while the property policy said every property case required a deterministic seed and persisted minimization.
Gap: A valid exact Cargo regression corpus could not satisfy generated-seed and shrinking requirements, but the plan did not state whether it was exempt or incomplete.
Proposed amendment: Apply deterministic seeds, reproduction, shrinking, target-bound persistence, and worker replay to generated property cases; allow static corpora only when the manifest freezes their source path, content digest, exact selector, and existing-focused-regression policy.
Resolution: `docs/plans/comprehensive-correctness-qualification-plan.md` now states the exemption and its exact admission requirements, while generated targets retain the complete worker lifecycle.

## 2026-07-13 - CQ1: Disposition And Comparator Report Axes

Status: Resolved
Revealed by: independent milestone audit of CQ1 report generation.
Current text: The report contract requested passed, failed, deferred, semantic-mining, and not-applicable counts by domain and comparator.
Gap: Semantic-mining and not-applicable are upstream dispositions that may intentionally own no executable comparator, so assigning them comparator counts would fabricate evidence semantics; deferred evidence also lacked explicit product ownership.
Proposed amendment: Report executable evidence pass, fail, planned, and deferred counts by domain and comparator; report upstream dispositions by domain without a comparator; require every deferred evidence case to name its deferred product explicitly.
Resolution: The plan and manifest model now separate those axes, `EvidenceCase` owns an optional product that is mandatory only for deferred status, and run reports derive deferred-product counts only from selected deferred evidence.

## 2026-07-13 - CQ1: Process-Tree And Atomic-Publication Platform

Status: Resolved
Revealed by: independent milestone audit of CQ1 timeout and artifact publication behavior.
Current text: CQ1 required killable worker timeouts and reproducible bounded reports but did not name a supported operating-system contract.
Gap: The implementation could terminate descendant process groups only on Unix and could not atomically replace a nonempty run directory on every supported host, so a generic portability claim would permit weaker evidence behavior.
Proposed amendment: Either implement equivalent process-tree termination and atomic directory publication on each supported platform or name a controlled platform and fail closed before execution elsewhere; controller cancellation must remain sticky across direct-child completion and prevent subsequent case execution or publication.
Resolution: CQ1 qualification execution is explicitly Linux-only, installs one process-wide sticky cancellation state, checks it before and after every child, after acquiring the repository publication lock, and immediately before report publication and output commit, uses process-group termination plus atomic directory exchange, and fails before case execution on unsupported hosts; inventory, report reading, and other non-execution operations remain independently portable where their own contracts allow.

## 2026-07-13 - PQ0: Checklist Child And Generator Ownership Schema

Status: Resolved
Revealed by: milestone audit of the first frozen performance disposition ledger.
Current text: PQ0 required explicit selected and deferred checklist children, concrete planned generators and scales, and failure on missing or duplicate ownership, but it did not define child-to-domain identity, global primary-owner uniqueness, registered generator parameter schemas, or source-backed API fixture identity.
Gap: a domain-wide checklist join let unrelated inherited and API workloads claim broad rows, partial checklist groups received every selected child as a Cartesian product, one analyzer sweep child/domain pair had two primary owners, and token-based generator validation accepted extra keys or a fake `cq-api-item-*` fixture id.
Proposed amendment: store exact child-to-domain ownership in the checked ledger, allow checklist ownership only on exact row-and-domain parent groups, require global `(child_id, performance_feature)` primary-owner uniqueness, use row-specific child ids for semantically distinct scopes, and validate exact parameter key sets plus canonical source-backed fixture identities for every registered generator.
Resolution: `docs/plans/comprehensive-stim-performance-qualification-plan.md` now defines exact checklist-child ownership, typed fixture and input-byte states, registered generator schemas, global child/domain uniqueness, and exact drift rejection. `ops/bench/src/qualification/checklist.rs`, `validation.rs`, and `validation/planned.rs` implement those contracts with negative tests for Cartesian ownership, duplicate global ownership, extra generator keys, fake API fixture ids, and same-length static fixture drift.

## 2026-07-08 - PFM2: Broader QEC Inverse And Measurement-Rich Transform Boundary

Status: Resolved
Revealed by: PFM0 evidence-lock refresh after promoting the selected reset-measure-detector, two-to-one, `m_det`, MPP, direct MPAD, noisy MZZ, observable include, noisy measurement, noisy measure-reset, noisy measure-reset detector-flow, pass-through, and selected measurement-rich `time_reversed_for_flows` slices; refreshed by the selected MPAD observable-flow follow-up.
Current text: PFM2 says broader detector-flow rewrites, broader observable-aware QEC inverse rewrites, feedback, repeats, and multi-instruction QEC inverse behavior remain active after listing the selected implemented packets, and the MPAD scope note says broader MPAD observable-flow semantics remain under this entry.
Gap: the selected evidence is now a collection of exact pinned or source-owned packets with tests, oracle rows, and report-only benchmark coverage where relevant, but the remaining broad phrase does not name exact circuits, detector counts, observable shapes, feedback shapes, noise models, repeat structures, comparator class, resource boundaries, oracle metadata, or benchmark policy. For MPAD specifically, the promoted selected record-tail observable-flow and duplicate observable-id record-tail packets still leave observable terms outside selected record-only tails, Pauli-observable tails, duplicate observable-id merging with non-record targets, interleaved operations, repeats, feedback, and multi-instruction MPAD rewrites unselected. Treating either broad phrase as implementation-ready would require inventing new QEC inverse or measurement-rich transform scope after the fact.
Proposed amendment: keep the currently promoted exact packets as the complete PFM2 QEC inverse and selected measurement-rich time-reversal evidence until a future plan names additional exact circuits, positive and negative tests, comparator behavior, oracle rows, benchmark or no-benchmark rationale, and resource-boundary behavior. Do not implement broader QEC inverse, detector-flow, observable-aware, feedback, repeat, multi-instruction transform behavior, or additional MPAD observable-flow families from broad checklist wording alone.
Resolution: Closed by PFM-B1 for the selected Rust transform scope. The nineteen named C++ and Python semantic subcases, bounded MPAD matrix, generated-surface reversal, shared sparse reverse-flow implementation, exact and structural oracle evidence, executable resource contracts, clean committed-HEAD allocation reports, milestone audit, and GPT-5.6/max review are complete in `docs/plans/pfm-b1-reverse-flow-progress-report.md`. Python bindings, exports, broader feedback, heralded-record reversal, duplicate-target compatibility decisions, and any behavior outside the finite ledger require a new exact plan and do not keep this entry open.

## 2026-07-08 - PFM4: Broader DEM Folded Traversal And Coordinate Boundary

Status: Resolved
Revealed by: PFM0 evidence-lock refresh after promoting selected DEM coordinate, graphlike, hypergraph, SAT/WCNF, ErrorMatcher filter, DEM sampler, sparse high-detector search, and analyzer fallback repeat-handling slices.
Current text: PFM4 says true folded traversal remains active for every other consumer that currently expands within a cap, and nearby inventory text says broader coordinate traversal and true folded generated-loop analyzer output remain active.
Gap: the selected PFM4 and PFM6 evidence now names exact repeat structures, consumers, caps, and folded behavior for specific cases, but the remaining broad phrase does not name which DEM consumer, repeat-body family, coordinate overlap pattern, comparator, resource boundary, oracle row, benchmark row, or fallback cap should be implemented next. Treating "every other consumer" as implementation-ready would require guessing the next correctness and performance contract.
Proposed amendment: keep the promoted DEM folded traversal, coordinate, sampler, search, SAT/WCNF, ErrorMatcher filter, sparse high-detector, and analyzer fallback slices as the current PFM4 evidence. Do not add broader DEM folded traversal or coordinate rows until a future plan names exact models, consumer paths, expected folded or capped behavior, positive and negative tests, resource boundaries, oracle metadata, and benchmark or no-benchmark rationale.
Implementation clarification: Pinned Stim's unfiltered detector-coordinate map returns every detector index below `count_detectors()`, including empty vectors for detectors without coordinate declarations. The PFM-B3 phrase about avoiding sparse nonexistent-id scans therefore applies to selected lookup; the full map remains inherently materialized and capped by returned entry count.
Audit clarification: "complete small-model differential corpus" did not define a generator domain, case count, seed, or consumer matrix, and the coordinate row's exact comparator was incompatible with algebraic repeat folding because pinned Stim accumulates fractional shifts sequentially. PFM-B3 now defines a 96-case deterministic Proptest corpus with seed `[0xB3; 32]` and uses absolute tolerance `1e-12` for fractional coordinate parity while retaining exact text checks where byte order is contractual.
Audit tooling follow-up: Rust-test proxy rows freeze command and metadata signatures but do not independently enforce the semantic comparator named by their blocker case, and statistical-plan schema validation does not encode expected bucket probabilities or calculate exact familywise tails. PFM-B3 closes this locally with literal assertions, explicit probability-bearing bucket names, and a documented exact-tail check; a future ledger schema revision should make those guarantees machine-readable before another statistical proxy relies on them.
Resolution: PFM-B3 is complete for the selected Rust surface. The shared folded visitor, seven named consumer migrations, materialization rationales, differential and resource tests, focused oracle rows, clean committed-HEAD allocation evidence, milestone audit, and GPT-5.6/max full-code-review closure are recorded in `docs/plans/pfm-b3-folded-dem-traversal-progress-report.md`. Broader unselected models require a new exact plan instead of reopening this resolved entry. The Rust-test proxy and statistical-schema tooling follow-up remains non-blocking and may be addressed by a future ledger schema revision.

## 2026-07-08 - PFM5: Broader Detecting-Region Utility Boundary

Status: Resolved
Revealed by: PFM0 evidence-lock refresh after promoting selected detecting-region repeat traversal, filters, generated repetition-code and surface-code regions, Clifford propagation, feedback placements, sweep-controlled no-op groups, ordinary noise, inverted targets, MPAD, MPP, SPP/SPP_DAG, heralded record-producing noise, ignored anticommutation, selected gauge behavior, and product-measurement gauge cancellation.
Current text: PFM5 says broader detecting-region target shapes, broader generated-code regions, and broader gauge behavior remain active after the selected utility evidence landed.
Gap: the current positive and fail-closed set is broad but still enumerated. The remaining phrase does not name exact target shapes, generated circuit families, detector or observable filters, tick windows, gauge modes, anticommutation behavior, comparator class, resource boundaries, oracle metadata, or benchmark policy. Treating it as implementation-ready would require guessing which utility family is next.
Proposed amendment: keep the promoted detecting-region subset as the current PFM5 detecting-region evidence. Do not add another detecting-region row until a future plan names exact circuits or target shapes, expected regions, positive and negative tests, comparator behavior, resource-boundary behavior, oracle metadata, and benchmark or no-benchmark rationale.
Resolution: PFM-B4 selects evidence closure instead of speculative feature growth. The two named pinned upstream cases and existing promoted target, generated-code, gauge, repeat, and resource evidence have focused executable evidence; the milestone audit, GPT-5.6/max review, synchronized documentation, and clean committed-HEAD reports are complete in `docs/plans/pfm-b4-detector-flow-progress-report.md`. Unselected utility families require a new exact plan.

## 2026-07-08 - PFM5: Broader Missing-Detector Utility Boundary

Status: Resolved
Revealed by: PFM0 evidence-lock refresh after promoting selected `missing_detectors` row-reduction, observable, MPAD, Clifford-propagation, Pauli-product, repeat-traversal, folded final-repeat, and generated-code suffix slices.
Current text: PFM5 says to extend `missing_detectors` for any remaining MPP, pair-measurement, observable, gauge, Clifford propagation, repeat traversal, and row-reduction cases, and says broader folded large-repeat traversal must name exact deterministic-loop families before implementation.
Gap: the promoted `missing_detectors` evidence now names exact row-reduction, repeated MPP, pair-measurement, record-only observable, ignored Pauli observable, `MPAD` measurement-pad, tableau-backed Clifford, `SPP`, `SPP_DAG`, bounded repeat, selected folded final-repeat, honeycomb, and toric suffix cases. The remaining phrase does not name exact circuits, unknown-input modes, gauge modes, stabilizer products, row-reduction rank patterns, folded repeat bodies, deterministic-loop families, comparator class, oracle metadata, resource limits, or benchmark policy. Treating the broad phrase as implementation-ready would require inferring a whole upstream utility-matrix target from the already promoted examples.
Proposed amendment: keep the promoted `missing_detectors` subset as the current PFM5 evidence. Do not add broader MPP, pair-measurement, observable, gauge, Clifford-propagation, repeat-traversal, folded large-repeat, row-reduction, unknown-input, or generated-code suffix rows until a future plan names exact circuits or generated families, expected missing-detector suffixes or source-owned invariants, positive and negative tests, comparator behavior, resource-boundary behavior, oracle metadata, and benchmark or no-benchmark rationale.
Resolution: PFM-B4 selects evidence closure instead of an unbounded generated-code or stabilizer-rank matrix. The three named pinned upstream cases and existing promoted row-reduction, observable, gate, repeat, and resource evidence have focused executable evidence; the milestone audit, GPT-5.6/max review, synchronized documentation, and clean committed-HEAD reports are complete in `docs/plans/pfm-b4-detector-flow-progress-report.md`. Unselected utility families require a new exact plan.

## 2026-07-08 - PFM5: Broader Flow-Generator Solver And Transform-Integration Boundary

Status: Resolved
Revealed by: PFM0 evidence-lock refresh after promoting the selected measurement-rich flow-generator, solve-for-measurements, diagnostics, signed sampled-flow, repeat-contained, sweep-controlled, heralded MPP, and selected `time_reversed_for_flows` slices.
Current text: PFM5 says broader all-operation composed measurement-rich flow generators, folded repeat traversal, full generator-table measurement solving, broader solver or generator diagnostics, and transform integration remain active.
Gap: the current evidence already names many promoted flow families, but the remaining broad phrases do not identify exact circuits, generated flow tables, diagnostic strings, repeat structures, accepted and rejected operations, transform APIs, comparator class, resource limits, oracle metadata, or benchmark policy. The broader heralded-noise generator subcase is already separately logged, but the rest of the flow-solver and transform-integration wording is still too broad to implement safely.
Proposed amendment: keep the promoted flow generator, solver, diagnostic, signed sampled-flow, and selected transform packets as the current PFM5 flow evidence. Do not add broader flow-generator, solver, diagnostic, folded-repeat, or transform-integration rows until a future plan selects exact subcases, expected flows or diagnostics, positive and negative tests, resource boundaries, oracle metadata, and benchmark or no-benchmark rationale.
Resolution: Exact closure scope is selected and complete under PFM-B4. The named C++ and Python semantic subcases have stable selectors, generator and sparse-checker dispatch share typed reverse transitions, exhaustive subset solving is removed in favor of generator-table GF(2) elimination, generated and resource tests pass, and oracle plus clean benchmark evidence is recorded in `docs/plans/pfm-b4-detector-flow-progress-report.md`. Broader measurement-rich transform integration remains separately owned by PFM-B1 instead of reopening this flow-engine entry.

## 2026-07-11 - PFM-B4: Matrix Benchmark Semantic Work

Status: Resolved
Revealed by: GPT-5.6/max milestone review after the first `pfm-b4-flow-solve-matrix-sizes` runner passed its nominal dimension and density checks.
Current text: PFM-B4 required deterministic scrambled dense `32x64` and `128x256` generator bases plus a `512x1024` high-qubit basis with 32 sparse active qubits, query-inclusive input-bit and query rates, allocation evidence, and density checks.
Gap: the contract did not specify measurement-signature width, query composition weight, nonempty solved parity, timing boundaries, active-submatrix density, exact active support, or a production-construction test. A unitary-only runner could copy direct generator rows, return empty parity, and satisfy the nominal matrix wording without exercising measurement-signature elimination or the former fallback boundary.
Proposed amendment: require exact 7-, 24-, and 12-singleton signature sets, a controlled-Pauli instruction mixing classical-feedback and plain two-qubit groups that forced the pre-PFM-B4 fallback, a medium case above the former sixteen-measurement cap, three-row-composed nonempty-parity queries, explicit timing boundaries, density and support guards, and literal production-contract tests.
Resolution: Resolved in the PFM-B4 contract before completion. The amended benchmark requires measurement-rich bases with exact 7-, 24-, and 12-singleton measurement-signature sets; 17, 65, and 33 three-row-composed queries with distinct singleton signatures and nonempty solved parity; the mixed controlled-Pauli shape that the previous generator rejected; a 24-measurement medium case beyond the removed fallback cap; end-to-end public solver timing with fixture construction and validation outside the sample; dense, sparse, and active-submatrix density bands; exact measurement-bearing active support; literal work-value assertions; and production case construction in benchmark tests.

## 2026-07-11 - PFM-B4: Flow Solver Query-Term Projection

Status: Resolved
Revealed by: GPT-5.6/max compatibility review after a duplicate-observable solver test passed without exercising an observable constraint.
Current text: PFM-B4 required solved flows to match requested Pauli and observable terms and named duplicate-observable solver coverage.
Gap: Stim v1.16.0 `solve_for_flow_measurements` copies only query input and output Pauli endpoints into its elimination table. Existing query measurement and observable terms are deliberately ignored, while `Flow::new` canonicalizes duplicate terms before the solver sees them. The old wording assigned observable-aware validity to a solver that neither Stim nor Stab defines that way and allowed a vacuous `[7, 7]` test to count as solver evidence.
Proposed amendment: define `solve_for_flow_measurements` as Pauli-projection solving that ignores measurement and observable terms already present on query flows, require a nonempty single observable and measurement-term compatibility regression, reconstruct solved flows by attaching returned measurements when checking Pauli validity, and keep duplicate term parity evidence in the `Flow` value-object and signed or unsigned checker suites that consume those terms.
Resolution: The PFM-B4 test and documentation contract now match pinned Stim. `pfm_b4_flow_solver_handles_sparse_high_qubits_and_uses_pauli_projection` uses nonempty, non-canceling query terms and receives the same result as the Pauli-only query; `stabilizers_flow_from_str_canonicalizes_duplicate_terms` retains direct duplicate measurement and observable parity evidence; solver Rustdoc, roadmap, checklist, progress report, and oracle metadata state the projection boundary explicitly.

## 2026-07-08 - PFM6: Broader Analyzer Search And Sparse-Tracker Boundary

Status: Resolved
Revealed by: PFM0 evidence-lock refresh after promoting selected generated-QEC analyzer, prefix/repeat/tail folded analyzer, loop-carried observable folded analyzer, period-8 observable folded analyzer, period-127 observable folded analyzer, loop-folded decomposition, folded observable guard, mixed-top-level fallback, direct and generated search, SAT/WCNF, sparse reverse tracker, and matched-error canonicalization slices.
Current text: PFM6 says broader true folded generated-loop analyzer behavior, broader generated search/SAT/WCNF families, tie-sensitive comparators, loop-folded generated search, analyzer/search sparse-tracker consumption, broader variable-target unitary semantics, and future matched-error hardening remain active.
Gap: the promoted PFM6 evidence is exact and source-owned, including the selected huge odd-repeat loop-carried observable case, selected period-8 logical-observable oscillation case, and selected period-127 logical-observable oscillation case, but the remaining broad phrases do not name generated circuits, exact DEM outputs, other logical-observable periods, search tie cases, SAT or WCNF encodings, sparse-tracker consumers, variable-target unitary families, value-object fields, comparator behavior, resource limits, oracle metadata, or benchmark policy. Treating the broad phrases as implementation targets would recreate the original planning failure of selecting work from whole upstream files.
Proposed amendment: keep the current PFM6 slices as the selected analyzer, search, sparse-tracker, and matched-error evidence. Do not add broader generated analyzer, logical-observable period, search, SAT/WCNF, sparse-tracker consumption, variable-target unitary, or value-object hardening rows until a future plan names exact subcases, positive and negative tests, comparator behavior, resource boundaries, oracle metadata, and benchmark or no-benchmark rationale.
Resolution: PFM-B5 replaces fixture-specific period handling with one shared shifted-recurrence engine, completes the named analyzer, graphlike, hypergraph, shared traversal-resource, and selected exact WCNF corpora, closes the selected sparse-tracker and matched-error consumers, and records executable exact selectors, content-bound direct oracle metadata, and independent analyzer-probe, fallback-expansion, per-error and aggregate target-work, graph-construction, persistent graph-payload, search-state-payload, edge-arena, SAT clause, and SAT literal resource limits in `docs/plans/pfm-b5-analyzer-search-progress-report.md`. The conservative SAT output-byte guard is redundant defense in depth behind stricter materialization caps instead of an independently reachable contract. Folded noisy `MPAD`, exact zero-probability diagnostics, generated source-mechanism membership, early trivial UNSAT, and exact finite low-quantization WCNF headers are selected compatibility contracts, while sparse-ID and large folded-repeat compression are deliberate semantic resource-hardening decisions rather than universal byte-exact claims. Fresh benchmark evidence and final audit or review closure remain process gates, not new semantic scope. Full ErrorMatcher provenance remains explicitly deferred, and any unselected analyzer or search family requires a new exact plan instead of reopening this resolved entry.

## 2026-07-07 - PFM3: Remaining Legal Non-Tableau Execution Boundary

Status: Resolved
Revealed by: PFM3 evidence-lock refresh after comparing the implemented fixed-tableau, `SPP` or `SPP_DAG`, selected deterministic `MPP`, selected stochastic `MPP(p)` sampler and detection-sampling, selected deterministic `MPAD`, selected stochastic `MPAD(p)` sampler and detection-sampling, and selected noisy `MPAD(p)` analyzer execution evidence against the broad "remaining legal-gate execution" wording.
Current text: PFM3 says non-tableau legal operations remain active or explicitly rejected after the fixed-tableau gate contract, supported Hermitian `SPP` or `SPP_DAG` execution slices, selected deterministic `MPP` execution slice, selected stochastic `MPP(p)` sampler and detection-sampling slice, selected deterministic `MPAD` execution slice, selected stochastic `MPAD(p)` sampler and detection-sampling slice, and selected noisy `MPAD(p)` analyzer slice landed.
Gap: the selected evidence now covers all 46 fixed-tableau gates across sampler, detection-conversion, and analyzer circuits, supported Hermitian `SPP` or `SPP_DAG` execution and anti-Hermitian rejection across the promoted sampler, detection-conversion, detector-frame, and analyzer surfaces, selected deterministic `MPP` Pauli-product measurement execution across the promoted sampler, detection-conversion, non-frame detection-sampling, frame detection-sampling, and analyzer surfaces, selected stochastic `MPP(p)` sampler distribution, detection-converter reference mapping, non-frame detection-sampling distribution, and frame-path detection-sampling distribution, selected deterministic `MPAD` measurement-pad execution across the promoted sampler, detection-conversion, non-frame detection-sampling, frame detection-sampling, and analyzer surfaces, selected stochastic `MPAD(p)` sampler distribution, detection-converter reference mapping, non-frame detection-sampling distribution, and frame-path detection-sampling distribution, and selected noisy `MPAD(p)` analyzer detector or observable effects. The remaining broad phrase does not name additional legal gate families, execution surfaces, accepted or rejected target shapes, comparator class, resource behavior, oracle metadata, or benchmark policy. Treating it as implementation-ready would require inventing scope after the fact.
Proposed amendment: keep the fixed-tableau plus supported Hermitian `SPP` or `SPP_DAG` plus selected deterministic `MPP`, selected stochastic `MPP(p)` sampler and detection-sampling, selected deterministic `MPAD`, selected stochastic `MPAD(p)` sampler and detection-sampling, and selected noisy `MPAD(p)` analyzer contract as the complete gate semantic evidence currently selected by PFM3. Do not add another legal-gate execution row until a future plan names exact gate families, execution surfaces, accepted and rejected behavior, positive and negative tests, resource-boundary behavior, oracle metadata, and benchmark or no-benchmark rationale.
Resolution: PFM-B2 now classifies all 81 canonical gates across eight implemented surfaces, maps all accepted target groups to 22 typed patterns, and closes the nineteen semantic families through 37 independently selectable exact, structural, rejection, semantic-invariant, state-equivalence, or statistical cases. `just oracle::blockers --check-selectors` validates all 165 ledger cases with no planned row, and `just oracle::run --milestone PF3` passes all implemented PF3 shards. Review remediation strengthened fixed-tableau, pair-measurement, deterministic MPP, controlled-Pauli, general-noise, heralded-record, typed exact provenance, shared statistical boundaries, resource behavior, and independent detector-frame benchmark evidence. Clean timing and allocation reports identify `HEAD=6474a7fb6752ec59448382cff73925eb6f30803b` with `local_modifications=false`; future execution surfaces or unselected gate shapes require an explicit new plan instead of reopening this resolved phrase.

## 2026-07-13 - CQ1: Property Plan Ownership And Execution

Status: Resolved

Revealed by: milestone audit of CQ1 property-runner evidence and future `property-target` promotion.

Current text: CQ1 required deterministic seeds, shrinking, persisted regressions, timeouts, and oversized-case rejection, but the inventory carried only a property comparator and selector without a typed generator or corpus plan.

Gap: a future property owner could have been promoted without freezing its generator domain, case count, seed panel, generated-byte cap, regression persistence policy, corpus identity, or killable execution mode, while a static exact Cargo regression and a generated worker target required different evidence contracts.

Proposed amendment: require every property case to carry a typed `property_plan`; planned cases reference their future qualification owner, implemented static corpora bind a repository path and content digest to an exact Cargo subprocess, and generated targets bind deterministic seeds, limits, persistence, and qualification-worker execution.

Resolution: Schema version 3 adds typed property-plan references and execution plans, validates source and status ownership, binds the four currently implemented static property corpora by path and digest, and rejects promotion of an unregistered or non-worker property target.

## 2026-07-13 - CQ1: Deferred Diagnostic Selection

Status: Resolved

Revealed by: milestone audit of the `correctness-run --allow-deferred` option and report completion semantics.

Current text: CQ1 required a selected deferred case to fail without selection permission, but it did not define whether permission applied to broad tier or feature runs, whether deferred-only runs were valid, or whether their reports could satisfy performance preflight.

Gap: an implementation could silently omit an explicitly selected deferred case, use the flag to hide deferred scope in a broad run, or publish a zero-execution report that appeared promotable because selection completeness counted only executable cases.

Proposed amendment: permit `--allow-deferred` only with explicit `--case` filters, retain permitted deferred cases as visible diagnostic counts without execution, reject planned and out-of-tier explicit cases, and make every preflight with a nonzero deferred count fail.

Resolution: The CQ1 selection policy and report validator implement those rules, targeted tests cover explicit planned, out-of-tier, and unpermitted deferred selections, and the correctness plan now labels deferred-only reports as diagnostic rather than promotable evidence.

## 2026-07-13 - CQ0: Pytest Parameterized Subcase Identity

Status: Resolved

Revealed by: GPT-5.6/max correctness-contract review of the first frozen qualification inventory.

Current text: CQ0 required every relevant Python semantic test and parameter subcase to receive a case-level disposition, but it specified only pytest function discovery.

Gap: a function-level AST record silently collapsed statically enumerable `pytest.mark.parametrize` cases and gave dynamic parameter expressions no finite identity rule. This could hide distinct semantic cases behind one planned selector or pretend an unbounded dynamic family was executable evidence.

Proposed amendment: expand literal collections, literal ranges, supported dictionary keys, `itertools.product`, and stacked parameter decorators into deterministic content-addressed subcases with a bounded Cartesian-product limit; represent every unsupported dynamic expression as one content-addressed `dynamic-family` record and reject it from executable scope until a later milestone selects finite explicit cases.

Resolution: [comprehensive-correctness-qualification-plan.md](comprehensive-correctness-qualification-plan.md) now states the static and dynamic parameterization contract. The CQ0 extractor implements it without importing pinned test modules, the manifest records `none`, `static-subcase`, or `dynamic-family`, and validation rejects executable dynamic families.

## 2026-07-13 - CQ0: Domain Relevance, Evidence Ownership, And Claim Staging

Status: Resolved

Revealed by: GPT-5.6/max architecture and correctness-contract reviews of the first frozen qualification inventory.

Current text: CQ0 required domain ids, primary cases, statistical plans, negative axes, and resource contracts but did not distinguish source relevance from executable proof or define how planned statistical evidence could exist before CQ1.

Gap: one ownership field could either erase deferred cases from domain summaries or let deferred cases count as passing evidence. Planned statistical rows also had no honest plan reference before CQ1, and feature-wide default negative or resource claims allowed an atomic semantic test to appear to prove boundaries it never exercised.

Proposed amendment: store domain relevance separately from executable evidence ownership; require deferred and not-applicable records to remain visible without owning passing evidence; require implemented statistical rows to reference existing source-owned plans while planned rows reference their future qualification-case owner; and assign negative axes or resource contracts only to dedicated evidence that directly measures those claims.

Resolution: [comprehensive-correctness-qualification-plan.md](comprehensive-correctness-qualification-plan.md) now defines the separation and staging rules. The CQ0 schema uses `domain_ids`, executable `ownerships`, independent `behavioral_surface` and `provenance`, typed statistical-plan references, and neutral semantic-only resource contracts; validation rejects stale, missing, or overclaimed combinations.

## 2026-07-13 - CQ0: Finite Cross-Cutting Resource Ownership

Status: Resolved

Revealed by: GPT-5.6/max CQ0 correctness-contract review after the first manifest treated one symlink regression as sufficient to make `CQ-RESOURCE` nonempty.

Current text: the domain matrix required admission, buffering, traversal, allocation, writer and visitor failure, path, and symlink evidence and forbade an umbrella case from closing a domain, while CQ0 acceptance required only one implemented or evidence-close case per feature.

Gap: the first frozen inventory contained one implemented symlink case and no planned resource cases, so CQ2 through CQ5 had no finite source-owned resource ledger to implement and generic feature non-emptiness could incorrectly present the domain as inventoried.

Proposed amendment: freeze independent planned owners for parser admission, checked count arithmetic, result-record admission, materialized expansion, streaming buffer slope, writer failure, visitor failure, replay and side-input admission, folded traversal work, search and solver admission, allocation scaling, typed path boundaries, and output-file lifecycle; retain symlink rejection as an independent implemented owner and validate the exact source-id set.

Resolution: [comprehensive-correctness-qualification-plan.md](comprehensive-correctness-qualification-plan.md) now names the thirteen planned boundary families. `qualification/resource.rs` owns those cases plus the implemented symlink regression, and manifest validation rejects a missing, stale, or incorrectly promoted resource owner.

## 2026-07-12 - PFM-B2: Semantic Rollups Versus Exact Upstream Provenance

Status: Resolved

Revealed by: final PFM-B2 ledger promotion after all 18 planned semantic-family tests passed.

Current text: the PFM-B2 plan and progress report treated 18 gate-family records as the final independently selectable evidence set, while 11 records still used `test-family` provenance with 30 named pinned-Stim test anchors.

Gap: the ledger validator correctly forbids promoting a test-family aggregation as implementation evidence. Relabeling each aggregation as one convenient test case would have discarded 19 selected upstream anchors, while weakening the validator would have violated PFM-B0's one-record-per-owned-subcase rule. The 18-row count therefore described semantic ownership but under-specified final provenance granularity.

Proposed amendment: retain the 18 records as semantic-family rollups, split every additional named pinned anchor into an independently selectable case, and require exact Cargo selectors, direct oracle signatures, honest comparator classes, source-owned statistical plans where the upstream behavior is probabilistic, and no shared selectors.

Resolution: eleven rollups are resolved to exact primary anchors and nineteen additional exact subcases bring PFM-B2 gate execution to 37 implemented cases. All 37 selectors are independent, all 37 PF3 oracle shards pass, thirteen statistical plans are shared between executable core tests and ledger validation, and the complete source ledger now has 165 cases with no planned row.

## 2026-07-07 - PFM3: Analyzer Sweep-Shape Boundary

Status: Resolved
Revealed by: PFM0 evidence-lock refresh after comparing the implemented PF3 analyzer sweep-control matrix against pinned Stim v1.16.0 `ErrorAnalyzer, ignores_sweep_controls`.
Current text: PFM3 said broader analyzer sweep-shape parity remained active after the selected analyzer sweep-control and `CZ` classical-only matrix landed.
Gap: pinned Stim v1.16.0 `src/stim/simulators/error_analyzer.test.cc` names only the `CNOT sweep[0] 0` analyzer no-op case, while current Stab evidence already covers that case plus selected `CY`, `CZ`, `XCZ`, and `YCZ` no-ops, selected `CZ` sweep/sweep, record/sweep, sweep/record, and record/record classical-only no-op groups, public `stab analyze_errors` behavior, and invalid controlled-Pauli target-position rejections. The remaining broad phrase did not name any additional legal gate-target shapes, accepted or rejected behavior, comparator class, CLI or Rust surface, oracle metadata, resource behavior, or benchmark policy.
Proposed amendment: keep the selected PF3 analyzer sweep-control and `CZ` classical-only matrix as the complete analyzer sweep evidence currently selected by PFM3. Do not add another analyzer sweep row until a future plan names exact gate-target shapes, expected no-op or rejection behavior, CLI and Rust surfaces, comparator class, positive and negative tests, oracle metadata, resource behavior, and benchmark or no-benchmark rationale.
Resolution: PFM-B2 records this boundary across all eight gate-contract surfaces, retains independently executable core and CLI evidence, and adds maximum legal sweep-ID semantic and allocation regressions plus low-ID and maximum-ID benchmark submeasurements. The ledger, checklist, inventory, milestone audit, and GPT-5.6/max full code review agree that future analyzer sweep shapes require a new failing pinned oracle, a newly selected public API, or an explicit compatibility-plan revision.

## 2026-07-10 - PFM-B2: Gate-Family Evidence Ownership

Status: Resolved
Revealed by: full-code-review of the PFM-B2 gate-surface contract groundwork.
Current text: PFM-B2 required each planned gate case to list all eight surfaces but did not require the case set to own every canonical semantic family.
Gap: all existing selectors could have been promoted while `I_ERROR`, `II_ERROR`, or circuit-level `REPEAT` behavior remained outside every evidence shard.
Proposed amendment: add typed semantic-family ownership to ledger schema version 2, require at least one family per gate-contract case, and reject an incomplete union across canonical families.
Resolution: `GateContractFamily` validation now requires all nineteen families, checks its wire names against canonical core metadata, and uses dedicated identity-noise and control-flow owners. Independent MPP, anti-Hermitian rejection, and MPAD evidence records bring the gate-contract inventory to eighteen cases.

## 2026-07-10 - PFM-B2: Analyzer Sweep Resource Evidence

Status: Resolved
Revealed by: milestone-audit and full-code-review of the PFM-B2 analyzer sweep evidence-close claim.
Current text: the selected analyzer sweep resource contract prohibited state proportional to sweep-index magnitude but required only a maximum-ID semantic regression.
Gap: a dense allocation indexed by `sweep[16777215]` could still produce the expected DEM and pass the semantic test.
Proposed amendment: compare low and maximum sweep IDs under allocation tracking with explicit count, total-byte, and peak-live-byte deltas, and retain both workloads as benchmark submeasurements.
Resolution: the feature-gated allocation test permits at most two extra allocation calls and 1,024 extra total or peak-live bytes, while the release-profile probe records identical low and maximum measurements of 25 allocation calls, 3,783 total bytes, 11 peak-live allocations, and 1,976 peak-live bytes.

## 2026-07-10 - PFM-B2: Mixed-Contract Benchmark Trigger

Status: Resolved
Revealed by: milestone-audit of the PFM-B2 benchmark disposition.
Current text: a mixed-contract benchmark was required only if generated dispatch introduced "measurable overhead," without defining a probe, baseline, repetition policy, or threshold.
Gap: final implementation could avoid benchmark ownership by declaring an unmeasured change insignificant.
Proposed amendment: require the mixed-contract row whenever production compile or execution dispatch begins consulting the contract, independent of a preliminary timing judgment.
Resolution: PFM-B2 now retains no new row only while the contract is static metadata; any production dispatch integration unconditionally triggers the mixed-contract benchmark requirement.

## 2026-07-07 - PFM2: Broader Repeat-Contained Feedback Boundary

Status: Superseded
Revealed by: PFM2 scope reconciliation after comparing the selected `Circuit::with_inlined_feedback` evidence against pinned Stim v1.16.0 `src/stim/util_top/transform_without_feedback.test.cc`.
Current text: PFM2 and the checklist say broader repeat-contained feedback parity remains open beyond the selected bounded repeat-loop and nested bounded-repeat detector-parity cases.
Gap: pinned Stim v1.16.0 `transform_without_feedback.test.cc` contains `basic`, `demolition_feedback`, `loop`, `mpp`, and interleaved-ordering tests, and current Stab evidence already covers those upstream cases plus selected `XCZ` or `YCZ` measurement-record feedback, bounded repeat-loop refolding, selected nested bounded-repeat `CY` and `CZ` detector-parity preservation, unsupported classical-control rejection, and excessive repeat-work rejection. The remaining broad phrase does not name additional repeat-body shapes, exact input circuits, exact output or semantic comparator, accepted or rejected feedback gates, resource limits, oracle metadata, or benchmark policy. Treating it as implementation-ready would require inventing repeat-contained feedback scope after the fact.
Proposed amendment: keep the pinned loop-refolding case and selected nested bounded-repeat detector-parity case as the complete repeat-contained feedback evidence currently selected by PFM2. Do not add another repeat-contained feedback row until a future plan names exact repeat structures, feedback gate and target shapes, canonical-output or semantic DEM comparator behavior, resource-boundary behavior, oracle metadata, and benchmark or no-benchmark rationale.
Resolution: Superseded by the 2026-07-08 PFM2 broader QEC inverse and measurement-rich transform boundary entry. The selected loop-refolding and nested detector-parity cases remain locked by `docs/plans/pfm2-feedback-repeat-boundary-scope.md`, and future repeat-contained feedback work beyond those selected cases must be named by a new exact-subcase plan.

## 2026-07-07 - PFM5: Broader Heralded-Noise Flow-Generator Boundary

Status: Superseded
Revealed by: PFM5 multi-target heralded flow-generator evidence hardening after promoting selected `HERALDED_ERASE` and `HERALDED_PAULI_CHANNEL_1` MPP cases.
Current text: PFM5 says broader heralded-noise generator synthesis remains active after the pinned single-target heralded-noise MPP fixture landed.
Gap: the selected evidence now covers pinned single-target heralded-noise MPP behavior plus exact multi-target `HERALDED_ERASE`, exact multi-target `HERALDED_PAULI_CHANNEL_1`, and a combined multi-target heralded-noise MPP case against pinned Stim v1.16.0 generator strings and checker satisfaction. The remaining broad phrase does not name additional legal circuits, invalid target shapes, comparator behavior, checker semantics for noisy flows, resource limits, oracle metadata, or benchmark policy. Treating it as implementation-ready would require inventing scope after the fact.
Proposed amendment: keep selected single- and multi-target heralded-noise MPP cases as the complete heralded-noise `circuit_flow_generators` evidence currently selected by PFM5. Do not add another heralded-noise flow-generator row until a future plan names exact circuits, positive and negative target-shape behavior, exact or semantic comparator rules, checker expectations, resource-boundary behavior, oracle metadata, and benchmark or no-benchmark rationale.
Resolution: Superseded by the 2026-07-08 PFM5 broader flow-generator solver and transform-integration boundary entry. The selected single- and multi-target heralded-noise MPP cases remain the current heralded-noise flow-generator evidence, and future heralded-noise generator work beyond those selected cases must be named by a new exact-subcase plan.

## 2026-07-07 - PFM5: Generated Missing-Detector Suffix Boundary

Status: Superseded
Revealed by: PFM0 evidence-lock refresh after promoting the pinned honeycomb and toric `missing_detectors` generated-code suffix cases.
Current text: PFM5 says to extend `missing_detectors` for selected generated honeycomb and toric suffix cases, then leaves "broader generated-code suffix analysis" as active remaining work.
Gap: pinned Stim v1.16.0 `src/stim/util_top/missing_detectors.test.cc` contains the honeycomb and toric generated-code suffix cases already promoted by `pf5-missing-detectors-generated-honeycomb-rust` and `pf5-missing-detectors-generated-toric-rust`, but the current milestone does not name any additional generated families, exact circuits, expected suffixes, known-input mode, comparator class, negative cases, resource boundaries, or benchmark policy. Treating the broad phrase as an implementation target would require inventing acceptance criteria after the fact or implying whole-file parity from already promoted cases.
Proposed amendment: keep the honeycomb and toric suffix cases as the complete generated-code `missing_detectors` evidence currently selected by PFM5. Do not add another generated-code missing-detector row until a future plan names exact generated circuits or semantic-mining fixtures, expected suffix behavior against pinned Stim or source-owned invariants, positive and negative tests, resource-boundary behavior, oracle metadata, and benchmark or no-benchmark rationale.
Resolution: Superseded by the 2026-07-08 PFM5 broader missing-detector utility boundary entry, which now owns any future generated-code missing-detector work beyond the pinned honeycomb and toric suffix cases. The current selected generated-code suffix boundary remains locked in `docs/plans/pfm5-missing-detectors-generated-boundary-scope.md`.

## 2026-07-04 - RPF2: Flow-Time-Reversal Dependency Boundary

Status: Superseded
Revealed by: implementation of the RPF2 circuit transform slices.
Current text: RPF2 asks to implement `time_reversed_for_flows` only after RPF5 defines required flow semantics, while also listing flow-time-reversal under the RPF2 transform objective.
Gap: RPF2 now has a scoped unitary `time_reversed_for_flows` bridge, but it still cannot specify or test the broader measurement-rich form without the RPF5 measurement-rich `has_flow`, flow-generator, solver, diagnostic, folded-traversal, and transform-integration semantics. Basic `Flow` parsing, included-observable terms, measurement-index terms, and multiplication are closed separately by `coverage-stabilizers-flow`, so treating those basics as part of this open dependency would misstate the remaining gap.
Proposed amendment: keep measurement-rich `time_reversed_for_flows` manifest-only under RPF2 until RPF5 closes the measurement-rich flow contract, then add a follow-up transform slice with exact public API shape, flow-semantic tests, and benchmark classification.
Resolution: Superseded by the selected measurement-rich `time_reversed_for_flows` slices, the 2026-07-08 PFM2 broader QEC inverse and measurement-rich transform boundary entry, and the 2026-07-08 PFM5 broader flow-generator solver and transform-integration boundary entry. The selected slices promote pinned `M` and `MZZ` examples with selected measurement-ordering evidence, selected plain `R`, `RX`, and `RY` reset-to-measurement conversion over one or more unique qubit targets, selected single-target `M`, `MX`, and `MY` measurement-to-reset conversion including the selected `dont_turn_measurements_into_resets` single-measurement option, selected `MR`, `MRX`, and `MRY` measure-reset flow reversal over one or more unique qubit targets with inverted result-target support, the selected `MZZ` plus plain-qubit unitary suffix packet matching pinned `flow_through_mzz_h_cx_s`, and the exact pinned `flow_flip` packet using the existing sparse flow verifier. Duplicate reset-only and duplicate measure-reset behavior is governed by `docs/plans/pfm2-time-reverse-duplicate-target-boundary-scope.md`. Detectors, feedback, noise, repeats, and broader multi-instruction QEC inverse behavior are governed by the 2026-07-08 PFM2 broader entry, while broader flow-generator, solver, diagnostic, folded-repeat, and transform-integration work is governed by the 2026-07-08 PFM5 broader entry until exact subcases are selected. The selected pinned feedback loop-refolding and nested bounded-repeat detector-parity cases are covered separately by `Circuit::with_inlined_feedback`.

## 2026-07-06 - PFM2/PFM5: Duplicate Reset-Only Time-Reversal Semantics

Status: Resolved
Revealed by: scope reconciliation after promoting selected plain reset-to-measurement `time_reversed_for_flows` over one or more unique qubit targets.
Current text: PFM2 and PFM5 listed duplicate reset-only operations as remaining active measurement-rich `time_reversed_for_flows` work after the selected plain reset slice landed.
Gap: Stim v1.16.0 accepts duplicate reset-only targets, but direct pinned-version probing with `uv run --with stim==1.16.0 python` shows malformed inverse flows for duplicate reset-only examples. For example, `R 0 0` with flow `1 -> Z0` returns inverse circuit `M 0 0` and flow `Z -> rec[-4] xor rec[-3]`, even though the inverse circuit has only two measurements. Blindly cloning that behavior would violate Stab's contract that returned flows are meaningful for the returned circuit, while silently "fixing" the behavior would no longer be Stim v1.16.0 output parity.
Proposed amendment: keep duplicate reset-only targets fail-closed in Stab's selected measurement-rich `time_reversed_for_flows` subset until a later compatibility decision explicitly chooses bug-compatible invalid flows, corrected semantic flows, or permanent rejection. The selected plain reset support over one or more unique qubit targets remains implemented, and duplicate reset target rejection remains source-owned test evidence.
Resolution: Resolved for the current PFM2 and PFM5 scope by `docs/plans/pfm2-time-reverse-duplicate-target-boundary-scope.md`. Duplicate reset-only targets remain fail-closed and are a named future compatibility decision, not an active current implementation target.

## 2026-07-06 - PFM2/PFM5: Duplicate Measure-Reset Time-Reversal Semantics

Status: Resolved
Revealed by: implementation of inverted result-target measure-reset `time_reversed_for_flows`.
Current text: PFM2 and PFM5 listed duplicate or inverted measure-reset shapes together as active measurement-rich `time_reversed_for_flows` work.
Gap: Stim v1.16.0 gives coherent self-validating inverse flows for inverted measure-reset targets, but direct pinned-version probing with `uv run --with stim==1.16.0 python` shows malformed inverse flows for duplicate measure-reset targets. For example, `MR 0 0` with satisfied flows `1 -> Z0` and `Z0 -> rec[-2]` returns inverse circuit `MR 0 0` and a flow `Z -> rec[-4] xor rec[-3]`, even though the inverse circuit has only two measurements. Blindly cloning that behavior would violate Stab's contract that returned flows are meaningful for the returned circuit, while silently "fixing" it would no longer be Stim v1.16.0 output parity.
Proposed amendment: keep duplicate measure-reset targets fail-closed in Stab's selected measurement-rich `time_reversed_for_flows` subset until a later compatibility decision explicitly chooses bug-compatible invalid flows, corrected semantic flows, or permanent rejection. Inverted result-target measure-reset support remains implemented and source-owned by exact tests.
Resolution: Resolved for the current PFM2 and PFM5 scope by `docs/plans/pfm2-time-reverse-duplicate-target-boundary-scope.md`. Duplicate measure-reset targets remain fail-closed and are a named future compatibility decision, not an active current implementation target.

## 2026-07-04 - PF1: Path-Based Circuit File Helper Streaming Boundary

Status: Resolved
Revealed by: full-code-review of the PF1 circuit file-helper API slice.
Current text: PF1 asks for circuit file constructor and writer helpers where they are useful Rust APIs, but it does not define whether path-based Rust helpers must stream through the parser or may use a bounded string-backed parser until a streaming `.stim` parser exists.
Gap: `Circuit::write_stim_file` can stream canonical output through an `io::Write`, but `Circuit::from_stim_file` still delegates to the existing string-backed parser. The current Rust API rejects files larger than 64 MiB before parsing to avoid unbounded allocation, so it is bounded but not a full replacement for Stim v1.16.0's streaming `FILE*` reader.
Proposed amendment: keep path-based Rust file helpers in PF1 with the documented 64 MiB read cap, and add a later parser milestone before claiming unbounded streaming `.stim` file-read parity for Rust APIs or future bindings.
Resolution: Resolved for the current Rust PF1 scope by the selected file-helper evidence. `Circuit::from_stim_file` keeps the documented 64 MiB path-read cap while the `.stim` parser remains string-backed, `Circuit::write_stim_file` writes canonical text through an IO writer, `pf1_circuit_file_helpers_read_and_write_canonical_stim_text` proves canonical read/write behavior with tags and repeats, `pf1_circuit_file_helpers_report_read_and_write_errors` proves missing-file, parse-error, oversized-file, and write-error reporting, oracle row `pf1-circuit-file-helpers` selects those tests, and `docs/plans/pf1-circuit-api-progress-report.md`, `docs/plans/partial-feature-inventory.md`, and `docs/stab-feature-checklist.md` document bounded path reads. Unbounded streaming `.stim` file-read parity remains future parser work and is not part of the current non-deferred PF1 closure.

## 2026-07-04 - PF1: Rust Coordinate Query Non-Finite Results

Status: Resolved
Revealed by: full-code-review of the PF1 circuit detector-coordinate API slice.
Current text: PF1 requires Rust circuit coordinate query parity for final qubit coordinates and detector coordinates, but it does not define whether Rust APIs should exactly mirror Stim v1.16.0 C++ double-overflow behavior or reject non-finite folded coordinate results.
Gap: Stim v1.16.0's C++ coordinate helpers can return infinities when finite coordinate inputs overflow during folded repeat arithmetic, while Stab's current Rust coordinate APIs reject non-finite folded coordinate results as a deliberate hardening choice.
Proposed amendment: keep the Rust API hardening documented for PF1, and require a later binding-parity decision before claiming exact Python or C++ coordinate-query side-effect parity.
Resolution: Resolved for the current Rust PF1 scope by the documented hardening decision and executable evidence. `pf1_circuit_stats_coordinate_queries_reject_non_finite_folded_shift` proves Rust circuit coordinate queries reject non-finite folded coordinate results, oracle rows `pf1-circuit-stats-coordinates` and `pf1-circuit-rust-api` select the PF1 coordinate tests, and `docs/plans/pf1-circuit-api-progress-report.md`, `docs/plans/partial-feature-inventory.md`, and `docs/stab-feature-checklist.md` document the hardening. Exact Python-style coordinate API shape and exact C++ infinity side-effect parity remain deferred binding-compatibility work, not active current Rust API work.

## 2026-07-05 - PFM4/PFM6: Generated Surface-Code Folded Coordinate Boundary

Status: Resolved
Revealed by: implementation of the PFM4 DEM coordinate-resource slice.
Current text: PFM4 asks to finish selected folded coordinate behavior for large, nested, and ambiguous repeats, while PFM6 tracks broader generated-loop analyzer behavior and loop-folded decomposition families.
Gap: the pinned Stim v1.16.0 `surface_code_coords_dont_infinite_loop` case compares `DetectorErrorModel::get_detector_coordinates` against `Circuit::get_detector_coordinates` after running `ErrorAnalyzer::circuit_to_detector_error_model` with `decompose_errors=true`, `fold_loops=true`, and remnant-edge blocking on a generated rotated surface-code circuit with prefix, repeat, and tail structure. Stab previously rejected this generated circuit before producing a DEM because `fold_loops=true` only supported top-level repeat-only and selected prefixed-repeat shapes. Treating the coordinate API as incomplete would have hidden the real analyzer loop-folding dependency, while treating the older coordinate tests as completion would have overstated generated-loop parity.
Proposed amendment: split the pinned generated surface-code coordinate case into a PFM6 analyzer-loop-handling requirement for generated prefix, repeat, and tail circuits, followed by a PFM4 coordinate equivalence check that compares DEM and circuit coordinate maps once the analyzer can produce the DEM.
Resolution: The generated surface-code coordinate dependency was initially resolved through bounded mixed-top-level fallback, while separate tests retained repeat-count, aggregate repeat-iteration, expanded-instruction, and selected folded-error non-masking guards for genuinely unsupported families. PFM-B5 later moved `pf6_dem_generated_surface_code_fold_loop_coordinates_match_circuit` onto generic reverse folding; the renamed `pf6-analyzer-generated-fold-loop-coordinates-rust` row runs with ops diagnostics and proves fallback is not used. Broader generated-loop behavior is governed by the finite PFM-B5 ledger and requires an explicit new plan for unselected subcases.

## 2026-07-05 - RPF3: SPP Analyzer State Propagation Boundary

Status: Resolved
Revealed by: implementation of sampler and detection-conversion SPP execution support plus focused analyzer probes.
Current text: the non-deferred partial milestone says to keep `SPP` and `SPP_DAG` parser, decomposition metadata, sampler execution, detection-conversion execution, and analyzer execution behavior synchronized, but it did not say what extra analyzer state-propagation evidence was required before analyzer execution could be promoted.
Gap: simply reusing the public `Circuit::decomposed()` SPP lowering is sufficient for sampler and detection-conversion compilation, but it is not a valid analyzer acceptance criterion by itself. During implementation, direct analyzer probes showed that promoting SPP through a naive decomposition hook could accept circuits whose fully decomposed H/S/H form still fails analyzer determinism checks. The milestone therefore needed a stricter analyzer-specific requirement instead of treating sampler, detection conversion, and analyzer promotion as one interchangeable task.
Proposed amendment: require analyzer `SPP` or `SPP_DAG` promotion to prove analyzer-state equivalence against explicit small-circuit expansions, including deterministic and nondeterministic detector cases, single-product and multi-product targets, sign handling, anti-Hermitian rejection, and comparison against pinned Stim v1.16.0 `analyze_errors` behavior where applicable.
Resolution: Resolved by the analyzer SPP slice. `PendingError`, observable sensitivity, and the analyzer gauge tracker now propagate supported Hermitian `SPP` and `SPP_DAG` products as unsigned Pauli-product Clifford updates, anti-Hermitian products are rejected, `dem_analyzer_spp_matches_explicit_phase_product_expansions`, `dem_analyzer_spp_nondeterministic_detector_matches_explicit_expansion`, and `dem_analyzer_spp_nondeterministic_observable_matches_explicit_expansion` prove explicit-expansion parity, and `gate_metadata_api_contract_table_matches_rust_accessors` keeps the support contract synchronized.

## 2026-07-04 - M9: Exact Feedback Loop Refolding Boundary

Status: Resolved
Revealed by: implementation of `docs/plans/m9-m2d-sweep-feedback-parity-plan.md`.
Current text: the M9 sweep and feedback plan asks to implement `--ran_without_feedback` and port transform subcases for `basic`, `demolition_feedback`, `loop`, `mpp`, and interleaved feedback ordering, while allowing unfinished transform subcases to be logged precisely.
Gap: the implemented M9 slice supports the public command-level `m2d --ran_without_feedback` case, exact `basic`, exact `demolition_feedback`, exact MPP feedback-transform parity, interleaved-operation ordering, and sweep-control preservation, but it does not claim full `Circuit.with_inlined_feedback` parity. Exact selected loop refolding was initially not implemented.
Proposed amendment: define this M9 wave as command-level feedback-inlining parity plus the exact transform subcases now covered by source-owned tests. Log broader repeat-contained feedback under-specification and public transform API parity before the checklist can mark full feedback-inlining transform parity done.
Resolution: Resolved by the PF2 bounded loop-refolding and nested bounded-repeat evidence slices. `Circuit::with_inlined_feedback` now covers the pinned `transform_without_feedback.loop` shape with exact output and DEM preservation, selected nested bounded-repeat `CY`/`CZ` detector-parity preservation, plus an excessive-repeat-work rejection. Broader repeat-contained feedback parity beyond these selected cases is under-specified until exact repeat structures, comparator behavior, resource behavior, oracle metadata, and benchmark policy are selected.

## 2026-06-28 - M12: CLI Sampling And Input Resource Boundaries

Status: Resolved
Revealed by: final GOAL full-code-review of public CLI resource handling.
Current text: M12 required allocation tracking, sampler hot-path optimization, memory gates, and future streaming detection conversion, but did not explicitly require the public `sample` CLI to avoid materializing all generated shots or require every implemented public CLI input path to use a bounded reader or streaming parser.
Gap: `stab sample` could build the full output buffer before writing, and some public `sample`, `convert`, `detect`, and `m2d` circuit or result-input reads bypassed the existing bounded input helper. The benchmark and memory-gate criteria did not by themselves prove hostile-input or huge-output CLI behavior.
Proposed amendment: add a M12 task and done criterion requiring public CLI resource-boundary regression tests: generated sample output must stream through the writer in bounded chunks, and implemented public circuit or result-input reads must have documented caps or streaming readers.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now requires CLI resource-boundary hardening in M12. The implementation adds `CompiledSampler::for_each_sample_with_seed_and_reference_mode`, streams `stab sample` output per shot or per 64-shot `ptb64` group, and routes public `sample`, `convert`, `detect`, and `m2d` circuit or result-input reads through documented 64 MiB caps unless the command already has a narrower streaming or bounded reader. Evidence includes `cargo test -p stab-core sampling streaming_samples_match_seeded_record_samples`, `cargo test -p stab-cli sample_streams_output_without_materializing_all_shots`, and `cargo test -p stab-cli oversized`.

## 2026-06-28 - M12: Optimization Log Evidence Strength

Status: Resolved
Revealed by: fresh M12 milestone audit of the source-owned optimization log.
Current text: M12 says rows optimized below the final-current profiler-note threshold must be listed in `benchmarks/profiler-notes/m12/optimization-log.json` with before and after report paths, dominant-cost evidence or a profiler blocker, implementation summary, semantic checks, and follow-up policy.
Gap: the validator checked schema shape, safe ids, safe report paths, non-empty evidence fields, and required row coverage, but it did not prove that the referenced before and after reports contain the row or support the claimed threshold or gate status.
Proposed amendment: either extend the source-owned optimization log with machine-checkable before and after ratios/statuses that do not depend on local `target/` artifacts, or add an ops validator mode that checks referenced reports when the reports are archived alongside completion evidence.
Resolution: `benchmarks/profiler-notes/m12/optimization-log.json` now uses schema version 2 with source-owned before and after ratios, gate statuses, hot-path statuses, and source profiler-note paths for after rows still above 1.5x. `cargo test -p stab-bench m12_optimization_log_validates_source_file` validates the new schema and required row coverage, while `docs/plans/rust-stim-drop-in-rewrite.md` and `benchmarks/README.md` now describe the stronger optimization-log evidence contract.

## 2026-06-28 - M10: ErrorMatcher Repeat-Contained Noise Scope

Status: Resolved
Revealed by: milestone audit and full-code-review of the M10 detector-analysis implementation.
Current text: M10 linked `src/stim/simulators/error_matcher.test.cc` as an analyzer test source and required structural oracle rows, but the roadmap did not say whether the milestone had to port every upstream ErrorMatcher provenance case.
Gap: current Stab error matching covers a staged subset, while upstream repeat-contained noise stack frames, generated surface-code repeat matching, heralded matching, and full sparse reverse tracker consumption require broader detector-analysis provenance work than the M10 done criteria named.
Proposed amendment: define M10 ErrorMatcher acceptance as the implemented staged direct-Rust subset and add a future detector-analysis item for the remaining provenance cases before claiming full ErrorMatcher parity.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now states that M10 accepts the implemented `coverage-simulators-error-matcher` subset only, names repeat-contained noise stack frames and related generated-circuit cases as future detector-analysis work, and adds that work to the Future Plan.

## 2026-06-28 - M10: Initial Analyzer Resource Limits

Status: Resolved
Revealed by: full-code-review of M10 public DEM analysis and `stab analyze_errors` input handling.
Current text: M10 required loop folding without accidental high-repeat flattening and linked graphlike, hypergraph, SAT, and analyzer workflows, but it did not specify temporary resource limits for the first compatible implementation.
Gap: without explicit caps, public APIs could try to flatten huge DEM repeats and `stab analyze_errors` could accept oversized circuit input or deeply nested repeats before the milestone had streaming or folded traversal support.
Proposed amendment: document accepted temporary limits for CLI input, circuit parser nesting, and DEM flattening-heavy analysis paths, require rejection tests for those limits, and require future streaming or folded traversal evidence before relaxing them.
Resolution: M10 now documents a 64 MiB `analyze_errors` input cap, a 1,000,000 line circuit parser cap, a 256 repeat-nesting cap, and DEM analysis flattening caps of 100,000 repeats, 1,000,000 expanded instructions, and 1,000,000 expanded repeat iterations. Evidence includes `analyze_errors_rejects_oversized_input_file_before_reading`, `analyze_errors_rejects_excessive_repeat_nesting`, `parser_rejects_excessive_repeat_nesting`, `dem_counts_large_repeat_detectors_without_unrolling`, `dem_public_flattening_apis_reject_excessive_repeat_expansion`, and `sat_problem_rejects_excessive_repeat_expansion`.

## 2026-06-28 - M10: Benchmark Evidence Reproducibility

Status: Resolved
Revealed by: milestone audit and full-code-review of M10 benchmark completion evidence.
Current text: M10 required `just bench::compare --milestone M10` to report `.dem` parse/print and `analyze_errors` workloads with loop-folding cases included.
Gap: a bare non-strict compare can succeed while rows are missing from the selected pinned-Stim baseline, and a progress report can cite a stale local baseline path after benchmark row ownership changes.
Proposed amendment: treat bare M10 benchmark comparison as reportable Stab-side evidence, but require a current selected pinned-Stim baseline path and matching strict compare report whenever completion evidence claims strict Stab-vs-Stim benchmark comparison.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now requires fresh named baseline and strict compare paths for strict M10 benchmark claims. `docs/plans/m10-progress-report.md` cites `target/benchmarks/m10-goal-baseline/baseline.json` and `target/benchmarks/m10-goal-strict-compare`, regenerated after the audit found the stale baseline.

## 2026-06-28 - M12: Resident Memory Peak Wording

Status: Resolved
Revealed by: fresh M12 milestone audit of memory-gate evidence.
Current text: the M12 done criterion said no primary workload may regress peak allocations or resident memory by more than 25 percent, while the implementation records allocation-counter maxima and samples process resident memory around each Stab-side benchmark measurement.
Gap: the wording could be read as requiring true peak RSS tracking during the operation, but the implementation and reports provide sampled resident-memory evidence.
Proposed amendment: describe the memory gate as peak live allocation evidence plus sampled resident-memory evidence, or replace the sampler with true peak-RSS tracking.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now says completion-style memory runs fail rows missing sampled resident-memory evidence and distinguishes historical schema-version-1 absolute sampled resident-memory checks from schema-version-2 row-local resident-delta checks with a 64 KiB absolute slack for page-granular RSS sampling noise. The done criterion now names sampled resident deltas for schema-version-2 memory evidence instead of unqualified resident memory.

## 2026-06-28 - M12: Profile Evidence Timing

Status: Resolved
Revealed by: milestone audit of M12 profiler-note evidence.
Current text: M12 says to profile every benchmark that is slower than the beta gate before optimizing it, and source-owned compare runs require notes for rows slower than 1.5x pinned Stim.
Gap: the milestone does not say whether completion evidence requires pre-optimization profiler captures, final-current profiler notes for rows still slower than 1.5x, or both.
Proposed amendment: choose a durable rule: either require pre-optimization notes for every row optimized during M12, or require final-current notes only for rows still slower than 1.5x and separate optimization logs for rows that were fixed.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines final-current profiler-note evidence for rows still slower than 1.5x pinned Stim, plus source-owned optimization-log evidence for M12-optimized rows. `benchmarks/profiler-notes/m12/optimization-log.json` records before and after reports, source-owned ratios, gate statuses, hot-path statuses, dominant-cost evidence, implementation summaries, semantic checks, and follow-up policy for optimized rows, and `cargo test -p stab-bench m12_optimization_log_validates_source_file` validates the log shape and required row coverage.

## 2026-06-28 - M12: Memory Gate Metric Scope

Status: Resolved
Revealed by: milestone audit of M12 memory-gate evidence.
Current text: M12 says no primary workload may regress peak allocations or resident memory by more than 25 percent relative to the first complete Stab benchmark report.
Gap: the previous memory gate tracked Stab-side allocation counts and maximum live allocated bytes, but it did not measure resident set size.
Proposed amendment: either narrow the done criterion to allocation counts and maximum live allocated bytes, or add RSS measurement to the benchmark report and memory gate before M12 completion.
Resolution: `stab-bench compare --track-allocations` now samples Stab-side resident memory with `memory-stats`, records both `resident_bytes` and `resident_delta_bytes` on measurements, promotes `stab_resident_bytes_max` and `stab_resident_delta_bytes_max` to compare rows, and `--require-memory-gate` requires allocation evidence plus the schema-selected resident-memory evidence. The historical M12 completion run passed with all 71 rows in `memory_gate_status=pass`, and the then-current post-beta `benchmarks/m12-primary-memory-baseline.json` used schema version 2 with `stab_allocation_bytes_max`, `stab_resident_bytes_max`, and `stab_resident_delta_bytes_max` for the expanded 85-row primary matrix.

## 2026-06-28 - M12: Regression Threshold Automation

Status: Resolved
Revealed by: milestone audit of M12 regression-threshold evidence.
Current text: M12 says workloads already at or below 1.25x Stim have benchmark thresholds checked by CI smoke or scheduled benchmark automation.
Gap: the repository has source-owned threshold files and local `just bench::primary-regression` evidence, but no checked CI or scheduled automation currently runs the full threshold gate.
Proposed amendment: add a CI or scheduled benchmark workflow for the full threshold gate, or revise the done criterion to accept archived local reports plus a lighter CI smoke command.
Resolution: `.github/workflows/m12-benchmarks.yml` now runs weekly and by manual dispatch, records a fresh `just bench::baseline --primary` report, runs `just bench::primary-regression --baseline <fresh-baseline> --report target/benchmarks/m12-scheduled-primary-regression`, and uploads the generated baseline and compare reports. `just bench::primary-regression` now includes `--warmup --measurement-runs 3` so the scheduled threshold gate uses the same warmed median-of-three Stab-side evidence policy as completion-style timing runs.

## 2026-06-28 - M12: Primary Row Comparability Classes

Status: Resolved
Revealed by: milestone audit of M12 beta-gate evidence and full code review of direct-match benchmark rows.
Historical text at the time: M12 said comparable primary rows must pass the 2.0x beta gate, and measured `contract-only` rows may pass only with source-owned waivers.
Gap: the milestone does not define the allowed comparability classes precisely enough for mixed rows such as direct internal perf matches, public CLI baselines, contract-representative in-process measurements, report-only rows, partial matches, and contract proxies.
Proposed amendment: define benchmark comparability classes such as `direct-match`, `cli-baseline`, `contract-representative`, `report-only`, `partial-match`, and `contract-proxy`; state which classes may satisfy beta, which require waivers, and which must remain follow-up evidence only.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` and `benchmarks/README.md` now define the M12 benchmark comparability taxonomy, `stab-bench compare` records `comparability` in compare rows, positional submeasurement pairing is limited to `direct-match`, beta-waiver diagnostics include the class, and `primary_compare_rows_have_machine_readable_comparability_classes` rejects unclassified primary rows.

## 2026-06-28 - M12: Microbenchmark Warmup And Repeated-Run Evidence

Status: Resolved
Revealed by: repeated M12 primary beta runs after adding the M4 sparse parser fast path.
Current text: M12 requires completion-style performance runs to pass the source-owned beta gate, but it does not define warmup, retry, repeated-run, or median-of-runs evidence for sub-microsecond and first-row benchmark cases.
Gap: the focused M4 parser row repeatedly measured around 1.31x pinned Stim and the next full primary beta run passed, but one intervening full primary beta run transiently measured `m4-circuit-parse` above the historical 2.0x gate at the beginning of the report.
Proposed amendment: define a completion evidence policy for tiny benchmark rows, such as one warmup compare pass before gated measurement, a fixed number of repeated compare runs with median or worst-run acceptance, or an explicit instability note and threshold exclusion rule for rows below a configured absolute-duration floor.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now requires completion-style primary beta runs to include `--warmup --measurement-runs 3`, `stab-bench compare --warmup` runs selected Stab-side workloads once before recording report measurements, repeated recorded measurement runs are aggregated by median, compare reports record `command.warmup` and `command.measurement_runs`, and `just bench::primary-beta` includes `--warmup --measurement-runs 3`.

## 2026-06-28 - M12: Beta Gate Scope For Contract-Only Primary Rows

Status: Resolved
Revealed by: M12 primary compare evidence after reclassifying the M8 primary sampling rows, `m8-sample-high-repeat-contract`, `m9-m2d-bitpacked-contract`, `m9-detect-primary-matrix-contract`, `m9-m2d-primary-matrix-contract`, `m10-analyze-errors-high-repeat-contract`, and four M11 sample_dem rows from `contract-only` to faithful public `stim-cli` baselines.
Current text: M12 said the frozen primary matrix is every benchmark contract row from M4 through M11 except baseline metadata anchors, and completion-style performance runs should pass `--require-beta-gate`, which failed when any selected row lacked a proven Stab-vs-Stim ratio or exceeded the 2.0x beta performance gate.
Gap: the primary matrix still included `m4-circuit-canonical-print`, `m7-convert-stim-canonical`, and `m10-dem-print-contract`, whose best current evidence is Stab-only contract timing because pinned Stim v1.16.0 has no matching public CLI or `stim_perf` baseline for the exact workload.
Proposed amendment: define an M12 beta-gate selection rule that separates comparable primary rows from source-owned contract-representative rows, then require `--require-beta-gate` for every comparable primary row and require each remaining contract-representative row to have either a promoted faithful Stim baseline or an explicit follow-up entry explaining why no ratio can be proven before beta.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now requires comparable rows to pass the active 1.25x beta gate while allowing only measured `contract-only` rows with checked source-owned JSON waivers. `benchmarks/m12-primary-beta-waivers.json` records the current no-ratio rows with reasons and follow-up paths, `stab-bench compare --require-beta-gate --beta-waivers` rejects stale or misapplied waivers, and `just bench::primary-beta` dispatches the completion-style checked run.

## 2026-06-28 - M12: Probability Utility Benchmark Gate Comparability

Status: Resolved
Revealed by: M12 primary compare evidence after optimizing the direct noisy Z-measurement sample path.
Current text: M12 says the beta performance gate applies to every primary parser, generator, sampling, detection, DEM parsing, DEM sampling, and analyzer workload, and the primary matrix includes `m8-probability-util` from `src/stim/util_bot/probability_util.perf.cc`.
Gap: `m8-probability-util` compared Stim's internal `biased_random_1024_*` utility benchmark against a Stab sampler-path contract proxy because Stab did not expose a standalone probability utility API.
Proposed amendment: introduce a Stab probability-draw utility API and a direct benchmark matching Stim's `biased_random_1024_*` filters, while keeping the public sampler probability paths covered by `m8-sample-throughput-*` and statistical oracle rows.
Resolution: Stab now exposes `biased_randomize_bits`, `m8-probability-util` measures seven direct 1024-bit biased-random utility cases against the pinned Stim perf filters, and `target/benchmarks/m12-primary-compare-after-probability-util-direct/compare.json` records the row passing the beta gate at 0.96x.

## 2026-06-27 - M9: Structural Oracle Flag Mismatch

Status: Resolved
Revealed by: running M9 done criteria after implementing the first detection workflow slice.
Current text: M9 lists `just oracle::run --milestone M9 --structural` as a done criterion.
Gap: `stab-oracle run` supported `--exact`, `--statistical`, `--implemented-only`, `--all`, and `--milestone`, but did not support a `--structural` filter; structural implemented rows ran under plain `just oracle::run --milestone M9`.
Proposed amendment: either add a `--structural` filter to `stab-oracle run`, or change milestone done criteria to say that structural rows are checked by `just oracle::run --milestone M9` and exact rows can be checked separately with `--exact`.
Resolution: `stab-oracle run` now supports `--structural`; `just oracle::run --milestone M9 --structural` runs implemented structural rows and reports remaining structural manifest-only rows.

## 2026-06-27 - M5: Memory Test Subcase Granularity

Status: Resolved
Revealed by: full code review of the M5 oracle rows.
Current text: the test-porting plan marked the Memory And Portable SIMD files as P0 for M5 without separating subcases that require APIs not introduced by the M5 portable bit core.
Gap: file-level oracle rows could imply full parity for upstream memory tests that include randomization, shifts, addition, table text parsing, table slicing and resizing, lower-triangular inversion, subset/intersection predicates, and custom allocation/storage utilities.
Proposed amendment: state that M5 owns only the subcases corresponding to the initial Stab bit-core API, and require unsupported upstream subcases to remain deferred until Stab introduces equivalent public or simulator-facing APIs.
Resolution: `docs/plans/stim-test-porting-plan.md` now defines the M5-owned memory subcases, and `oracle/fixtures/manifest.csv` labels implemented M5 memory rows as M5-owned subsets rather than full-file parity.

## 2026-06-27 - M5: Benchmark Compare Semantics

Status: Resolved
Revealed by: milestone audit of the M5 benchmark compare output.
Current text: M5 required `just bench::compare --milestone M5` to report row XOR, matrix transpose, bit-packed copy, sparse XOR, and popcount-like workloads against the M3 baseline.
Gap: the milestone did not distinguish exact upstream workload matches from Stab-only M5 contract-smoke workloads, did not require normalized Stab rates, and did not say whether the current simple matrix transpose helper had to match the upstream 10k optimized transpose benchmark.
Proposed amendment: require M5 compare output to report normalized Stab rates and pinned Stim timings, label non-comparable contract-smoke workloads explicitly, and defer exact optimized 10k bit-table transpose parity to M12 performance hardening.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now names the normalized M5 benchmark evidence and the M12 deferral; `stab-bench compare` prints normalized rates and M5 comparability notes.

## 2026-06-27 - M5: Portable SIMD Feature Gate Location

Status: Resolved
Revealed by: implementation of the M5 portable-SIMD bit kernel.
Current text: M5 said to pin Nightly and isolate `#![feature(portable_simd)]` in bit-kernel modules.
Gap: Rust feature gates are crate-level attributes, so `#![feature(portable_simd)]` cannot be placed only inside a module even when direct `std::simd` imports and operations are module-local.
Proposed amendment: state that the crate-level feature gate is allowed at `stab-core` crate root, while direct `std::simd` imports and operations must stay in approved bit-kernel modules.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now distinguishes the crate-level feature gate from direct SIMD usage.

## 2026-06-27 - M4: Canonical Printer Benchmark Baseline

Status: Resolved
Revealed by: milestone audit of the M4 benchmark evidence.
Current text: M4 required `just bench::compare --milestone M4` to report parser and printer throughput against the M3 C++ baseline, while `m4-circuit-canonical-print` was a contract-only row.
Gap: pinned Stim v1.16.0 has parser and gate lookup perf runners but no direct C++ canonical-printer benchmark runner; using public `stim convert` would benchmark result-format conversion, not `.stim` canonical printing.
Proposed amendment: state that M4 reports parser throughput and gate lookup against the C++ baseline, and reports Stab-only canonical-printer timing against an explicit contract-only printer row without claiming a Stab-vs-Stim printer comparison.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now names the narrower M4 benchmark evidence and the general contract-only benchmark rule.

## 2026-06-27 - M3: Benchmark Compare Acceptance

Status: Resolved
Revealed by: milestone audit of the M3 benchmark harness.
Current text: M3 asks for `just bench::compare` to run Stab and Stim on the same benchmark matrix once Stab supports the feature, but the done criteria only require `bench::baseline`, `bench::list`, and `bench::smoke`.
Gap: the milestone does not define what `bench::compare` must accept, read, report, or fail on before implementation milestones start using it as evidence.
Proposed amendment: require `bench::compare` to read an M3 baseline report or use the documented default, distinguish runnable rows from pending Stab runners, and make `--strict` fail until the owning milestone provides the required Stab runner and complete selected baseline evidence.
Resolution: `stab-bench compare` now reads the default or explicit baseline report, runs Stab comparison runners for supported rows, reports pending rows explicitly, and makes `--strict` fail when any selected row is pending or missing from the selected baseline; `benchmarks/README.md` and `docs/plans/rust-stim-drop-in-rewrite.md` document the behavior.

## 2026-06-27 - M1/M4/M7: CLI Convert Ordering

Status: Resolved
Revealed by: milestone audit of the M1 compatibility matrix and `just oracle::matrix --milestone M4`.
Current text: M1 says planned CLI surfaces are covered in implementation order as `gen`, `convert`, `sample`, `detect`, `m2d`, `analyze_errors`, and `sample_dem`; M4 links `src/stim/cmd/command_convert.test.cc` for parse/canonical-print behavior; M7 tasks say to implement both `stim gen` and `stim convert`.
Gap: the plan does not clearly say whether M4 implements a public `stim convert` subset, only internal parse-print oracle fixtures, or test metadata that M7 later turns into CLI compatibility.
Proposed amendment: state that M4 owns the `.stim` parser/printer library contract and may use `command_convert.test.cc` only as oracle evidence for parse/canonical-print semantics, while M7 owns public `stim convert` CLI compatibility unless the plan explicitly promotes a minimal M4 CLI subset.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now limits M4 benchmarks to parser, printer, and gate lookup, and assigns public `stim convert` CLI compatibility and convert throughput to M7.

## 2026-06-27 - M4/M6/M9: Top-Level Algorithm Fixture Ownership

Status: Resolved
Revealed by: implementation of the M4 oracle rows for circuit, gate, and probability coverage.
Current text: the compatibility matrix and oracle fixture manifest assigned `src/stim/util_top/mbqc_decomposition.test.cc`, `src/stim/util_top/simplified_circuit.test.cc`, and `src/stim/util_top/transform_without_feedback.test.cc` to M4 as `stim-format` rows.
Gap: these upstream tests depend on flow, tableau, simulator, or detector-conversion semantics that M4 does not otherwise own.
Proposed amendment: assign MBQC decomposition and simplified-circuit tests to the tableau milestone and assign transform-without-feedback tests to the detector-conversion milestone.
Resolution: `oracle/compatibility-matrix.csv` and `oracle/fixtures/manifest.csv` now assign MBQC decomposition and simplified-circuit fixtures to M6, and transform-without-feedback fixtures to M9.

## 2026-06-27 - M3: Contract-Only Benchmark Rows

Status: Resolved
Revealed by: implementation of the M3 benchmark manifest.
Current text: M3 requires benchmark contracts for surfaces such as bit-packed `m2d` and `.dem` parse/print while also requiring pinned C++ baseline results.
Gap: some required benchmark contracts do not have a direct `stim_perf` filter or Stim CLI command that exercises the exact future Stab performance surface.
Proposed amendment: allow explicit contract-only benchmark rows when no direct pinned C++ executable baseline exists, require those rows to name their upstream source and owning milestone, and require a runnable benchmark before an implementation milestone claims a Stab-vs-Stim performance comparison.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now makes contract-only benchmark rows explicit in M3.

## 2026-06-27 - M2: Comparator Implementation Ownership

Status: Resolved
Revealed by: milestone audit of the M2 oracle corpus.
Current text: M2 said to define structural and statistical comparators, while later milestones own the first runnable uses of many semantic and statistical comparator families.
Gap: the plan did not say whether M2 must implement every comparator executable or only define comparator contracts and fixture metadata before implementation milestones begin.
Proposed amendment: state that M2 defines comparator contracts and manifest metadata, while the owning M4 through M11 milestones must implement runnable structural or statistical comparator code before marking matching rows `implemented`.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now makes comparator implementation ownership explicit in the M2 task list.

## 2026-06-27 - M8: Linked Simulator And Result-Format Subcase Ownership

Status: Resolved
Revealed by: milestone audit of M8 oracle coverage.
Current text: M8 links the C++ Simulators group for frame, tableau, vector, and graph simulation cases that apply to sampling, and links the C++ Input And Output Formats group for measurement record formats and sparse shots.
Gap: the milestone did not enumerate which upstream simulator subcases are required for the public sampler milestone, which are direct Rust API compatibility tests, and which are later simulator or IO-library work.
Proposed amendment: split M8 acceptance into explicit subcase groups for public `stim sample` CLI parity, result writer byte layouts, result reader/parser APIs, frame/tableau sampling semantics, reference-sample behavior, and simulator-only structural utilities; require every M8-owned group to have runnable fixtures or a named deferred owner before milestone completion.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md`, `oracle/compatibility-matrix.csv`, and `oracle/fixtures/manifest.csv` now scope M8 to frame/tableau sampling semantics, move detection-output helpers to M9, move sparse reverse detector-frame tracking to M10, and move graph/vector simulator internals to M12. The M8 frame and tableau simulator coverage rows are runnable through `cargo test -p stab-core sampling`.

## 2026-06-27 - M8: Benchmark Strictness And Baseline Completeness

Status: Resolved
Revealed by: milestone audit of `just bench::compare --milestone M8`.
Current text: M8 requires `just bench::compare --milestone M8` to report compile/analysis time, single-shot latency, and batch throughput for `1`, `1024`, and `1_000_000` shots.
Gap: non-strict benchmark comparison can exit successfully while M8 benchmark rows have missing pinned Stim baselines, and the milestone does not define which report-only rows are acceptable before completion. Pending Stab runners are no longer part of this gap because every M8 benchmark manifest row now has either a Stab comparison runner or an explicit contract-only runner.
Proposed amendment: require `just bench::compare --milestone M8 --strict` for milestone completion, or explicitly list report-only exceptions with their owning follow-up milestone; every required M8 benchmark row should have a Stab runner and selected pinned Stim baseline before completion.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now requires `just bench::compare --milestone M8 --strict` for M8 completion. Strict comparison validates pinned Stim baseline metadata, rejects unmatched milestone filters, fails invalid placeholder baseline rows, fails empty contract-only placeholders, and the M8 benchmark manifest rows now have Stab runners or measured representative contract rows. Regenerating `target/benchmarks/baseline/latest/baseline.json` with `just bench::baseline --only M8` produced selected pinned Stim rows accepted by the strict comparison.

## 2026-06-27 - M8: Multi-Outcome Statistical Evidence

Status: Resolved
Revealed by: milestone audit of M8 statistical oracle rows.
Current text: M8 requires statistical tests for noisy sampling that do not require C++ random-stream compatibility, and the test strategy names binomial and chi-square checks.
Gap: the milestone does not specify which multi-outcome channels require multinomial or chi-square evidence, what bucket definitions should be used, or what sample counts and false-positive budgets are acceptable for channels such as `PAULI_CHANNEL_2` and heralded local noise.
Proposed amendment: require binomial evidence for one-bit marginal fixtures and chi-square or equivalent multi-bucket evidence for multi-outcome noise fixtures, with fixture-specific bucket definitions, sample counts, fixed seeds, and confidence bounds recorded in the oracle manifest.
Resolution: M8 now includes a bucketed statistical oracle comparator and fixture-specific bucketed rows for `PAULI_CHANNEL_2`, correlated errors, independent X/Y/Z errors, depolarizing basis variants, multi-target `X_ERROR`, and measurement-result flip probabilities. Each row records bucket definitions, sample counts, fixed seed 5, and a 5-sigma tolerance in `oracle/fixtures/manifest.csv`; the oracle harness validates that the declared false-positive budget is not tighter than the tolerance can support.

## 2026-06-27 - M11: Sample Dem CLI Flag And Format Scope

Status: Resolved
Revealed by: milestone audit and full code review against pinned Stim `command_sample_dem.cc` and `dem_sampler.inl`.
Current text: M11 requires `stim sample_dem` with supported flags, detector output, observable output, bit-packed formats, seed handling, and deterministic behavior where applicable.
Gap: the milestone does not enumerate the exact Stim v1.16.0 `sample_dem` flag set, and therefore does not say whether `--err_out`, `--err_out_format`, `--replay_err_in`, `--replay_err_in_format`, `ptb64` detector/observable/error streams, or Stab-only observable append/prepend flags are in scope for the initial M11 completion bar.
Proposed amendment: list the required M11 public `sample_dem` flags and formats explicitly. If full Stim parity is required in M11, require independent detector, observable, and error streams; error recording and replay; `01`, `b8`, `r8`, `ptb64`, `hits`, and `dets` where upstream accepts them; and oracle rows for each stream route. If the initial milestone intentionally excludes some of these surfaces, add explicit deferrals with compatibility-matrix rows and require unsupported flags to fail with clear errors.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now lists the required M11 `sample_dem` flags as `--shots`, `--in`, `--out`, `--out_format`, `--seed`, `--obs_out`, `--obs_out_format`, `--err_out`, `--err_out_format`, `--replay_err_in`, and `--replay_err_in_format`; it requires `01`, `b8`, `r8`, `ptb64`, `hits`, and `dets` for detector, observable, error, and replay streams where Stim accepts those formats. Stab-only `--append_observables` and hidden `--prepend_observables` are explicitly excluded from M11 Stim parity evidence and must reject conflicts if retained.

## 2026-06-27 - M11: DEM Sampler Fixture Group Acceptance

Status: Resolved
Revealed by: milestone audit of the M11 oracle manifest, direct Rust tests, and benchmark rows.
Current text: M11 says to add sparse, dense, repeated, and high-detector-count DEM fixture groups, and the done criteria require exact, statistical, structural, and benchmark checks.
Gap: the milestone does not define which fixture groups must be oracle rows, which can be direct Rust tests, which can be benchmark-only representatives, what comparator each group uses, or what sample counts and statistical bounds prove noisy sparse, dense, repeated, high-detector, observable-only, and correlated-error behavior.
Proposed amendment: define an M11 fixture matrix with rows for deterministic exact output, statistical noisy sampling, sparse detector ids, dense detector targets, repeated detector shifts, high detector ids, observable-only errors, detector-observable correlation, and correlated detector combinations. Each row should name its upstream source, comparator mode, sample count or structural assertion, output format, and whether it is acceptance evidence or benchmark-only evidence.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines the M11 fixture acceptance matrix. The matrix requires exact and statistical evidence for basic, sparse, dense, repeated, high-detector, observable-only, detector-observable correlation, and correlated detector-combination groups, exact side-output oracle rows for observable, error, and replay side streams, and a direct Rust structural row for the M11-owned `dem_sampler` subset.

## 2026-06-27 - M11: DEM Sampler Streaming And Scale Limits

Status: Resolved
Revealed by: full code review of `CompiledDemSampler` and `stab sample_dem` resource behavior.
Current text: M11's objective says to implement fast DEM-based sampling, and the tasks require reusable analysis state, per-shot sampling, repeated DEM fixtures, high-detector fixtures, and bit-packed formats.
Gap: the milestone does not specify whether M11 must stream shots like Stim's striped sampler, what maximum supported `--shots`, detector count, observable count, error count, DEM input size, or output byte count is acceptable during the initial implementation, or whether bounded repeat unrolling is an accepted temporary design.
Proposed amendment: require a compiled or streaming DEM sampler API that can write output in bounded chunks without materializing all shots, or explicitly document initial resource limits and add rejection tests for excessive shots, excessive detector/observable widths, excessive error counts, oversized DEM input, and nested repeat expansion. State whether folded repeat sampling is an M11 requirement or an M12 performance-hardening task.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now accepts a bounded materialized M11 sampler and names the required limits. The implementation adds `CompiledDemSampler::validate_sample_buffer_units`, a 64 MiB `sample_dem` DEM input cap, core plus CLI rejection tests for excessive shot counts, high detector widths, optional generated or replayed error-record buffers, and bounded repeat expansion. Replay input reads only the requested `ptb64`, `b8`, and `r8` record prefix, text replay records are capped at 1,048,576 bytes per requested record, and extra replay records after `--shots` are ignored. True streaming output, folded repeat sampling without bounded unrolling, exact output-byte budgeting, and performance thresholds are deferred to M12.

## 2026-06-27 - M11: Benchmark Baseline And Comparability

Status: Resolved
Revealed by: milestone audit and full code review of `just bench::compare --milestone M11`.
Current text: M11 requires `just bench::compare --milestone M11` to report sparse, dense, repeated, and high-detector-count DEM sampling throughput.
Gap: the milestone does not say whether M11 completion requires a selected pinned-Stim baseline artifact, strict comparison, external `stab-cli sample_dem` process timings, in-process Stab core timings, or report-only representative workloads. The current Stab runners print useful in-process rates, but the latest local baseline artifact can omit M11 pinned-Stim rows and the `stim-cli` row is not an external CLI-vs-CLI comparison.
Proposed amendment: define M11 benchmark acceptance as either `just bench::compare --milestone M11 --strict` against a baseline that includes `m11-dem-sampler` and `m11-sample-dem-cli`, or explicitly label the M11 benchmark rows as report-only until M12. If CLI comparability is required, add a Stab subprocess runner using the same argv and stdin path as the Stim CLI baseline, and normalize rates by shots, detector bits, error operations, and output bytes where appropriate.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines M11 benchmark acceptance as report-only Stab-side throughput from `just bench::compare --milestone M11`. Strict pinned-Stim baseline completeness, external CLI-vs-CLI process timing comparability, performance thresholds, and normalized primary-matrix reporting are M12 responsibilities.

## 2026-06-27 - M9: Feedback-Removal Conversion Scope

Status: Resolved
Revealed by: implementing `stab m2d` and inspecting pinned Stim `command_m2d.test.cc` plus `transform_without_feedback.test.cc`.
Current text: M9 requires `stim m2d` with measurement input parsing, detector conversion, observable output, and inconsistent-input errors, and the compatibility matrix assigns `transform_without_feedback.test.cc` to M9.
Gap: the milestone does not explicitly say whether `m2d --ran_without_feedback` and circuit feedback inlining are required for the initial M9 CLI surface, even though pinned Stim tests exercise that path and Stab currently rejects the flag instead of silently returning incorrect output.
Proposed amendment: add an explicit M9 task and fixture group for `--ran_without_feedback` if feedback-removal parity is required now, or defer it to a named later detector-conversion submilestone while requiring the CLI to reject the flag with a clear error.
Resolution: the later M9 sweep and detector-utility follow-ups implemented the scoped `m2d --ran_without_feedback` path and promoted `coverage-util-top-transform-without-feedback` into an executable row for the owned subset. The implementation covers basic measurement feedback, demolition feedback, interleaved ordering, sweep-control preservation, MPP feedback inlining, and the later selected bounded loop-refolding and nested bounded-repeat detector-parity cases while rejecting excessive repeat work and unsupported classical controlled feedback gates. Broader repeat-contained feedback beyond selected cases is under-specified until exact repeat structures, comparator behavior, resource behavior, oracle metadata, and benchmark policy are selected, and full feedback-transform parity remains future work.

## 2026-06-27 - M9: Detector Analysis Utility Row Ownership

Status: Resolved
Revealed by: M9 oracle manifest after implementing detector sampling, measurement-to-detection conversion, observable output, and M9 benchmark runners.
Current text: M9 links detector-conversion workflows and the compatibility matrix assigns `circuit_to_detecting_regions.test.cc`, `missing_detectors.test.cc`, and `transform_without_feedback.test.cc` to M9.
Gap: the milestone objective and tasks describe public `detect` and `m2d` workflows, but they do not define public Rust APIs, fixture subsets, or done criteria for detecting regions, missing-detector analysis, or feedback-removal transforms; those rows remain manifest-only while the public CLI/core conversion rows are implemented.
Proposed amendment: split M9 into explicit public workflow acceptance and detector-analysis utility acceptance, naming the APIs and upstream subcases required for each utility row, or move the utility rows to the DEM/analyzer milestone that introduces the required analysis structures.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now splits M9 public workflow acceptance from detector-analysis utilities. The M9 detector utility closure promoted simple detecting regions, basic single-record missing-detector suggestions, and MPP feedback inlining into explicit Rust APIs and executable rows. PF5 later promoted detecting-region repeat traversal, detector and logical-observable target filters, generated repetition-code all-target/all-tick selection with selected exact D0, D6, and L0 regions, selected generated rotated surface-code all-target/all-tick helper counts plus exact D0, D4, and L0 regions, fixed single-qubit and two-qubit Clifford propagation for plain qubit targets, selected measurement-record feedback target placements, selected gate-order-valid sweep-controlled Pauli sign-only no-op groups including selected `CZ` classical-only bit-bit groups, source-owned fail-closed validation for non-`CZ` sweep/sweep, record/sweep, and record/record groups, inverted targets for promoted measurement and reset-measurement families, `MPP` Pauli-product measurement targets, `SPP` and `SPP_DAG` unitary Pauli-product target shapes, ignored anticommutation mode, selected measurement-gauge ignored-mode behavior, product-measurement gauge-cancellation behavior, missing-detectors Gaussian row reduction, repeated MPP and pair-measurement stabilizer products, record-only observable rows, ignored Pauli observable rows, tableau-backed Clifford propagation for plain qubit target groups, bounded repeat traversal with explicit expansion caps, selected folded final-repeat traversal for covered deterministic measurement loops with flat or bounded nested local bodies, selected observable-neutral final repeats where top-level record-only observable rows are redundant under independent detector evidence, and the pinned honeycomb and toric generated-code suffix cases. The later PF2 feedback slices promoted selected bounded loop refolding and selected nested bounded-repeat detector-parity preservation for `Circuit::with_inlined_feedback`. Broader `missing_detectors` utility families are now scoped by the 2026-07-08 PFM5 broader missing-detector utility boundary entry, while the selected generated-code suffix boundary remains locked by `docs/plans/pfm5-missing-detectors-generated-boundary-scope.md`. Broader detecting-region target-shape support outside the promoted positive set and source-owned fail-closed set, broader generated-code detecting-region extraction beyond the promoted repetition-code and selected rotated surface-code cases, broader gauge handling, broader folded large-repeat traversal beyond the selected final covered deterministic measurement-loop cases with flat or bounded nested local bodies and selected observable-neutral final repeats, public measurement-rich flow solving, and full transform API parity remain future work with explicit manifest or gap entries. Broader repeat-contained feedback beyond selected cases is under-specified until exact repeat structures, comparator behavior, resource behavior, oracle metadata, and benchmark policy are selected.

## 2026-06-27 - M9: Sweep-Conditioned Detection Conversion Scope

Status: Resolved
Revealed by: milestone audit against pinned Stim `measurements_to_detection_events.test.cc`.
Current text: M9 requires measurement-to-detection conversion from measurement records and circuits with detectors, observables, coordinate shifts, and repeats, but it does not mention sweep data or sweep-conditioned detector expectations.
Gap: upstream measurement-to-detection tests include sweep-bit inputs that can alter expected detector parities through sweep-controlled operations, while the current Stab converter and `m2d` CLI accept only measurements, a circuit, and reference-sample options.
Proposed amendment: state whether sweep-conditioned conversion is in M9 scope. If it is, require typed sweep inputs, `--sweep` and `--sweep_format` CLI flags, and fixtures for sweep count mismatches and sweep-controlled parity changes. If not, move those upstream subcases to the first milestone that introduces sweep-aware simulation.
Resolution: M9 follow-up work implemented sweep input data for public `m2d` detection conversion through `--sweep` and `--sweep_format`, while omitted sweep input uses all-false sweep bits. PF3 later allowed the selected `detect` omitted-all-false sweep subset, but pinned Stim v1.16.0 has no `stim detect --sweep` flag, so typed sweep files are not a `detect` CLI parity target. Broader analyzer sweep behavior, broader sweep target shapes, and deferred Python detector-sampler sweep APIs remain outside the resolved M9 scope.

## 2026-06-27 - M9: Benchmark Baseline Completeness

Status: Resolved
Revealed by: milestone audit of `just bench::compare --milestone M9` and `just bench::compare --milestone M9 --strict`.
Current text: M9 requires `just bench::compare --milestone M9` to report `detect` and `m2d` throughput separately for text and bit-packed formats, while the benchmark plan describes comparisons against pinned Stim v1.16.0.
Gap: the non-strict compare command reports Stab-side M9 timings, but the current baseline artifact has no M9 pinned Stim rows, so `--strict` fails and the command is not a complete Stab-vs-Stim comparison.
Proposed amendment: either require M9 to record selected pinned Stim detect and m2d baselines before completion and run the strict comparison, or label M9 benchmark evidence as report-only until M12 freezes the primary performance matrix.
Resolution: M9 benchmark acceptance is explicitly report-only Stab-side timing from `just bench::compare --milestone M9`. Strict pinned-Stim baseline completeness, external CLI-vs-CLI timing comparability, beta-gate ratios, and promoted primary-matrix baseline rows belong to M12, where selected M9 rows can gain faithful public Stim CLI baselines without changing M9 completion. Evidence is `benchmarks/manifest.csv` marking the M9 rows as `report-only`, `cargo test -p stab-bench m9_benchmark_rows_have_stab_compare_runners --quiet`, and the M12 progress note for promoted M9 baseline rows.

## 2026-06-27 - M9: Detection Bit-Packed Format Scope

Status: Resolved
Revealed by: milestone audit against pinned Stim `command_detect.cc`, `command_m2d.cc`, and `measurements_to_detection_events.test.cc`.
Current text: M9 requires `stim detect` with bit-packed modes and `stim m2d` with measurement input parsing, while the benchmark plan names text and bit-packed input.
Gap: the milestone does not say whether M9 bit-packed parity means the `b8` subset needed by current decoder workflows or every Stim v1.16.0 bit-packed format including `ptb64`, nor does it name zero-width bit-packed input behavior as an acceptance case.
Proposed amendment: define the exact M9 bit-packed parity boundary by command and stream: `b8` for public detector and observable streams, `ptb64` for `detect` detector output and `detect --obs_out`, and `ptb64` for `m2d` measurement input only. Require `m2d --out_format=ptb64` and `m2d --obs_out_format=ptb64` to reject like pinned Stim v1.16.0, require zero-width `ptb64` input rejection, and require decoded-record bounds before allocation.
Resolution: M9 now requires `b8` parity for public `detect` and `m2d` detector and observable streams, `ptb64` parity for `detect` detector output, `detect --obs_out`, and `m2d` measurement input, plus explicit rejection for `m2d` `ptb64` detector and observable outputs. Evidence is `cargo test -p stab-cli m9`, including `detect_writes_ptb64_detector_and_observable_outputs`, `detect_rejects_ptb64_shots_that_are_not_multiple_of_64`, `m2d_reads_ptb64_records_and_writes_supported_formats`, `m2d_rejects_ptb64_detector_output_like_stim`, `m2d_rejects_ptb64_observable_output_like_stim`, `m2d_rejects_zero_width_ptb64_input`, and `m2d_rejects_excessive_ptb64_decoded_shots_before_expansion`, plus `cargo test -p stab-core result_formats::tests::ptb64_records_are_measurement_major_over_64_shot_groups`.

## 2026-06-27 - M9: Generated Fixture Round-Trip Coverage

Status: Resolved
Revealed by: milestone audit comparing the M9 task list to current oracle and benchmark evidence.
Current text: M9 says to add round-trip tests for bit-packed input/output and text input/output across circuit fixtures generated in M7.
Gap: current M9 exact oracle rows use hand-authored circuits and measurement records, while generated repetition-code coverage exists in benchmark runners instead of runnable oracle or test acceptance rows; the plan does not define the generated fixture matrix, output formats, round-trip direction, or whether benchmark primary-matrix representatives count as acceptance evidence.
Proposed amendment: add explicit generated-fixture M9 oracle or Rust tests for selected M7 repetition, rotated surface, unrotated surface, and color-code circuits across `01`, `dets`, and `b8` conversion paths, or narrow the task to say generated-fixture coverage is benchmark evidence only until the primary matrix is frozen.
Resolution: M9 now treats generated-fixture acceptance as `sample -> m2d` public-workflow round trips compared with `detect` for M7 repetition, rotated-surface, unrotated-surface, and color-code circuits in `01` text and `b8` bit-packed output with appended observables. Evidence is `cargo test -p stab-cli m2d_round_trips_generated_m7_circuits_in_text_and_bitpacked_formats --quiet` and the oracle manifest row `coverage-simulators-measurements-to-detection-events-generated`. Existing hand-authored M9 oracle rows continue to cover `dets` label formatting.

## 2026-06-27 - M9: Pauli-Target Observable Detection Scope

Status: Resolved
Revealed by: full code review against pinned Stim `frame_simulator` observable handling.
Current text: M9 requires `stim detect` with observables and detector output handling.
Gap: the milestone did not distinguish measurement-record observables from `OBSERVABLE_INCLUDE` Pauli target observables. The prior implementation rejected Pauli-target observables for `detect` to avoid silently returning incorrect logical flips, while `m2d` continued to ignore Pauli targets like pinned Stim's measurement-to-detection converter.
Proposed amendment: either require M9 to implement frame-simulator-style Pauli-target observable flips for `detect`, including deterministic and random observable fixtures, or defer Pauli-target observable detection to the simulator-completeness milestone while requiring an explicit error in the M9 CLI and Rust API.
Resolution: M9 now requires `stim detect` to implement frame-simulator-style Pauli-target observable flips for the documented scalar frame subset while leaving `m2d` conversion behavior unchanged. Evidence is `coverage-simulators-frame-simulator-pauli-observables`, `cargo test -p stab-core detection_sampling`, including RX/RY/RZ Pauli observable parity, product-measurement frame updates, and reference-sample measurement-bit cancellation, plus `cargo test -p stab-cli detect_supports_pauli_target_observable_flips`, `cargo test -p stab-cli detect_supports_product_measurements_with_pauli_observable_flips`, and `cargo test -p stab-cli m2d_ignores_pauli_target_observables_like_stim_conversion`.

## 2026-06-27 - M9: Detection Conversion Streaming And Scale Limits

Status: Resolved
Revealed by: full code review of `detect` and `m2d` resource behavior.
Current text: M9 requires decoder-pipeline detection workflows and benchmark reporting but does not define streaming, batching, loop-folding, or maximum supported record sizes.
Gap: current Stab materializes measurement records and detection records in memory and unrolls detection-conversion repeats within explicit temporary limits. This prevents unbounded CPU or memory use for hostile inputs, but it is not a final decoder-scale streaming design and does not match Stim's ability to process large files and folded repeats efficiently.
Proposed amendment: add a follow-up milestone or M12 task for compiled/streaming detection conversion that processes records in bounded batches, preserves repeat structure where possible, avoids duplicate sampler analysis, documents or removes temporary limits, and includes benchmark rows for large generated-code detector workloads.
Resolution: M9 now explicitly accepts a bounded materialized detection-conversion implementation with documented temporary limits: 1,000,000 bits for measurement, detector, and observable record widths, 64,000,000 buffered bits for materialized measurement samples and detection records, and 100,000 repeat iterations during conversion planning. M12 now owns compiled or streaming detection conversion when benchmark evidence shows the materialized path is the bottleneck, including bounded batches, folded-repeat preservation where possible, duplicate-analysis removal, and large generated-code `detect` and `m2d` benchmark rows. Evidence is `cargo test -p stab-core detection_conversion_rejects_unbounded_record_shapes --quiet` and the M12 task list in `docs/plans/rust-stim-drop-in-rewrite.md`.

## 2026-06-27 - M8: Skip Loop Folding Scope

Status: Resolved
Revealed by: milestone audit of M8 `--skip_loop_folding` evidence.
Current text: M8 requires repeat handling, reference sample behavior, and `stim sample` core flags, but does not say whether `--skip_loop_folding` must change the Rust sampler implementation or only be accepted with output-compatible behavior.
Gap: Stab currently accepts `--skip_loop_folding` and proves output parity on a repeat circuit, while optimized loop-folded reference-sample construction remains deferred by the `coverage-util-top-reference-sample-tree` manifest row. The milestone text does not state whether that is sufficient for M8 completion.
Proposed amendment: state that M8 requires `--skip_loop_folding` to be accepted and output-compatible for repeat circuits, while optimized loop-folded reference-sample construction and performance parity are deferred to M12 unless promoted earlier.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines `--skip_loop_folding` acceptance as output-compatible repeat-circuit behavior for M8 and defers optimized loop-folded reference-sample construction plus performance parity to M12. Evidence is the implemented oracle fixture `m8-sample-skip-loop-folding` and the structural row `coverage-util-top-reference-sample-tree`, whose manifest note records the optimized construction deferral.

## 2026-06-27 - M7: Generated Fixture Matrix Scope

Status: Resolved
Revealed by: milestone audit of M7 generator oracle rows and structural generator tests.
Current text: M7 says to store generated circuit fixture matrices by family, task, distance, rounds, and noise settings for later M8 through M12 reuse, and says `stab-cli gen` output must match Stim v1.16.0 for the compatibility matrix of families, tasks, distances, rounds, and noise settings.
Gap: the milestone does not define the concrete matrix dimensions, required noise settings, fixture artifact format, acceptable storage size, whether every matrix point needs exact CLI golden output or direct Rust structural parity, or how the matrix is reused by later milestones without checking in very large circuit outputs.
Proposed amendment: define a primary M7 generator matrix with explicit family, task, distance, round, and noise tuples; require exact CLI goldens only for a small public-command subset and direct Rust structural or generated-on-demand oracle checks for the larger matrix; name the fixture artifact location and the later milestones that consume each fixture group.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines the M7 generated-circuit acceptance matrix as source-owned oracle, direct-test, and benchmark manifests instead of checked-in generated circuit bodies. Exact CLI goldens cover the public command shape for supported families and tasks, direct Rust structural tests cover representative larger noisy family/task/distance/round/probability cases, and benchmark rows cover generated-on-demand primary matrix circuits reused by M8 through M12.

## 2026-06-27 - M7: Convert Command Circuit Versus Result-Format Scope

Status: Resolved
Revealed by: implementation and upstream test inspection of `src/stim/cmd/command_convert.test.cc`.
Current text: M7 requires `stim convert` for `.stim` parse and canonical print workflows and links `command_convert.test.cc` as a direct CLI command test source.
Gap: pinned Stim v1.16.0 `command_convert.test.cc` primarily tests measurement, detector, and observable result-format conversion among formats such as `01`, `b8`, `hits`, `r8`, and `dets`, often with `--circuit`, `--dem`, `--types`, and observable-output routing, while `.stim` canonical circuit parse-print behavior is already owned by the M4 core parser/printer fixtures and is not an exact upstream `stim convert` command surface.
Proposed amendment: split M7 convert acceptance into two explicit tracks: a Stab-specific `convert --in_format=stim --out_format=stim` canonical circuit workflow backed by M4 parser/printer tests, and pinned-Stim-compatible result-data conversion rows backed by `command_convert.test.cc`; defer full `b8`, `hits`, `r8`, `--circuit`, `--dem`, `--types`, and `--obs_out` support to the first milestone that owns the corresponding measurement-record and detector-error-model APIs if M7 does not introduce those APIs.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now splits M7 convert acceptance into `convert --in_format=stim --out_format=stim` canonical circuit workflows backed by M4 parser/printer behavior and pinned-Stim result-data conversion rows backed by `command_convert.test.cc`. `oracle/fixtures/manifest.csv` includes a Stim-compatible rejection row for `--bits_per_shot` to `dets`, and `README.md` documents supported and deferred conversion behavior.

## 2026-06-27 - M6: Random Generation Hook Ownership

Status: Resolved
Revealed by: milestone audit of the M6 stabilizer algebra implementation and benchmark rows.
Current text: M6 requires random generation hooks and links upstream `tableau_random*`, Clifford random distribution, and stabilizers-to-tableau fuzz and perf coverage.
Gap: the milestone does not define which Rust RNG type, seeding contract, distribution parity, or public random-constructor API must exist before Stab has simulator and sampling consumers.
Proposed amendment: state that M6 must either introduce explicit deterministic random hooks for `CliffordString`, `PauliString`, and `Tableau` with documented seed and distribution contracts, or defer random generation to the first simulator/sampler milestone that consumes those hooks while keeping M6 deterministic algebra and iterator coverage.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines caller-owned `rand::Rng` hooks for `PauliString`, `SingleQubitClifford`, `CliffordString`, and `Tableau`. Seeded Rust RNGs give reproducible Stab output and exact Stim C++ random-stream parity is not required. `PauliString` samples uniformly over sign and `I`/`X`/`Y`/`Z` bases, including zero-length sign sampling; `CliffordString` samples uniformly over the 24 single-qubit Clifford gates; `Tableau::random` samples valid Clifford tableaus from random Clifford-circuit shapes; and exact uniform tableau sampling or random-workload performance parity is deferred to M12 if needed.

## 2026-06-27 - M6: Util-Top Algorithm Subset Boundaries

Status: Resolved
Revealed by: milestone audit of M6 `circuit_flow_generators`, `has_flow`, `circuit_inverse_qec`, `simplified_circuit`, `mbqc_decomposition`, `circuit_vs_tableau`, and `stabilizers_to_tableau` rows.
Current text: M6 links related util-top tests when their dependencies are in scope, but the oracle manifest records several rows as implemented with notes that defer measurement-rich, detector, noise, sampled-flow, full-gate, tableau-to-circuit, and fuzz variants.
Gap: the milestone does not split deterministic unitary/tableau subset parity from full upstream util-top parity, so an implemented row can be misread as full Stim parity for the entire upstream file.
Proposed amendment: split each related util-top row into explicit subcases owned by M6 and deferred subcases owned by the simulator, detector, or performance-hardening milestones; require public APIs for subset helpers to document unsupported semantics until the deferred rows are implemented.
Resolution: M6 now splits util-top ownership by manifest row. The roadmap names the deterministic unitary and tableau-backed subcases owned by `coverage-util-top-circuit-flow-generators`, `coverage-util-top-has-flow`, `coverage-util-top-circuit-inverse-qec`, `coverage-util-top-circuit-vs-tableau`, `coverage-util-top-simplified-circuit`, `coverage-util-top-mbqc-decomposition`, and `coverage-util-top-stabilizers-to-tableau`, and it names deferred measurement-rich, detector, noise, sampled-flow, full-gate, tableau-to-circuit, and unsupported-semantics variants. Evidence is `just oracle::list --milestone M6`, `just oracle::run --milestone M6 --structural`, and the implemented `coverage-util-top-*` manifest notes.

## 2026-06-27 - M7: Generator Benchmark Comparability

Status: Resolved
Revealed by: implementation of Stab-side M7 benchmark runners for `just bench::compare --milestone M7 --strict`.
Current text: M7 requires generator throughput for repetition, rotated surface, unrotated surface, and color code circuits, and the benchmark manifest uses pinned Stim CLI rows for `stim gen` plus a `main_sample*` CLI dispatch perf row.
Gap: the plan does not specify whether Stab-side generator benchmark evidence must measure direct Rust generator construction, `stab-cli gen` end-to-end execution, canonical `.stim` printing cost, process startup cost, or all of these separately.
Proposed amendment: split M7 benchmark acceptance into explicit rows for direct Rust generator construction, `stab-cli gen` in-process dispatch, and external process startup or canonical text emission if those are required; keep the current Stab direct generator rows report-only until an exact CLI-vs-CLI threshold is specified.
Resolution: `docs/plans/rust-stim-drop-in-rewrite.md` now defines M7 benchmark acceptance as report-only direct Rust generator construction, in-process CLI dispatch, and canonical conversion timings. Strict external CLI-vs-CLI thresholds and process-startup comparability are deferred to M12 performance hardening.

## 2026-06-27 - M6: Stabilizers Versus Amplitudes Dependency

Status: Resolved
Revealed by: milestone audit of the M6 linked-test list and compatibility matrix.
Current text: M6 lists `stabilizers_vs_amplitudes` as a related util-top test when dependencies are in scope.
Gap: the plan does not say which amplitude-state or simulator dependency brings this row into scope, and no M6 fixture manifest row currently names the semantic subset that should be proven by the algebra milestone alone.
Proposed amendment: either add a deterministic algebra-only fixture for the subcases that can be checked without an amplitude simulator, or move `stabilizers_vs_amplitudes` to the tableau simulator milestone with a clear dependency note.
Resolution: M6 now owns the deterministic algebra-only `unitary_to_tableau` subset from `stabilizers_vs_amplitudes`, covering all 46 canonical known-unitary gate-data matrices, controlled-gate endian mapping, Stim-style phase smoothing, and malformed or non-Clifford matrix rejection via `coverage-util-top-stabilizers-vs-amplitudes`. `tableau_to_unitary`, random tableau/unitary roundtrips, and amplitude-simulator cross-checks are explicitly deferred.

## 2026-06-28 - M6: Stabilizers Vs Amplitudes Gate-Data Breadth

Status: Resolved
Revealed by: milestone audit of the `coverage-util-top-stabilizers-vs-amplitudes` fixture after implementing `unitary_to_tableau`.
Current text: M6 tracks a selected `unitary_to_tableau` subset from `stabilizers_vs_amplitudes` and the fixture manifest marks that selected subset as implemented.
Gap: upstream Stim's `unitary_to_tableau_vs_gate_data` test iterates every gate with a known unitary matrix, but Stab currently proves only selected single-qubit matrices plus the four upstream endian examples. The plan does not yet assign the exhaustive known-unitary gate-data matrix sweep to a specific milestone or manifest row.
Proposed amendment: add a separate compatibility row for exhaustive known-unitary gate matrix coverage once Stab has centralized gate-unitary data, or explicitly defer that sweep to the matrix/state-vector milestone that also owns `tableau_to_unitary`.
Resolution: `coverage-util-top-stabilizers-vs-amplitudes` now covers the upstream known-unitary gate-data loop directly with 24 canonical single-qubit matrices and 22 canonical paired-gate matrices copied from pinned Stim v1.16.0 gate data, plus a count check tied to that scope. The plan and manifest now treat exhaustive `unitary_to_tableau` gate-data coverage as M6 evidence; only `tableau_to_unitary`, random roundtrips, and amplitude-simulator checks remain deferred.

## 2026-06-27 - M6: Stabilizer Benchmark Exact Workload Parity

Status: Resolved
Revealed by: milestone audit of `just bench::compare --milestone M6`.
Current text: M6 requires `just bench::compare --milestone M6` to report Pauli, Clifford, tableau, tableau-iterator, and stabilizers-to-tableau workloads, while benchmark manifest rows point at upstream random, fuzz-like, and large-tableau perf filters.
Gap: the milestone does not distinguish report-only deterministic Stab benchmark runners from exact parity with upstream random and 10K-qubit perf workloads.
Proposed amendment: require M6 compare output to provide deterministic Stab-side timings and normalized rates for each M6 benchmark row, label non-exact benchmark workloads in compare notes, and defer exact random and large-tableau threshold parity to M12 performance hardening after random hooks and optimized tableau internals are specified.
Resolution: M6 benchmark acceptance is now explicitly report-only deterministic Stab-side timing from `just bench::compare --milestone M6`. The roadmap allows direct operation-shape matches for Pauli, Clifford, and Pauli-iterator rows only when compare notes say so, while tableau, tableau-iterator, and stabilizers-to-tableau workloads remain deterministic substitutes until M12 decides exact random, fuzz-like, signed-tableau, and 10K-qubit threshold parity. Evidence is `cargo test -p stab-bench m6_benchmark_rows_have_stab_compare_runners --quiet` and `just bench::compare --milestone M6`.

## 2026-06-27 - M6: Stabilizer Algebra Public View And Text Scope

Status: Resolved
Revealed by: implementation of the first owned Pauli-string algebra slice and upstream stabilizer scan.
Current text: M6 requires `PauliString`, `CliffordString`, `Tableau`, related iterators or views, sign handling, and text round trips.
Gap: the milestone does not say whether Rust must expose a public borrowed `PauliStringRef` equivalent, does not distinguish real-phase C++ `PauliString` text from phase-general `FlexPauliString` sparse and lowercase text, and does not define which Python-facing phase semantics are required before the Python API milestone.
Proposed amendment: state that M6 starts with owned Pauli, FlexPauli, Clifford, and Tableau APIs; borrowed views may stay internal unless a later M6 task proves a public view is necessary; text parity must separately cover real dense `PauliString` syntax and phase-general `FlexPauliString` dense or sparse syntax; Python-only binding behavior is semantic-mining input but not a public API requirement until the Python milestone.
Resolution: M6 now states that the public Rust API starts with owned `PauliString`, `FlexPauliString`, `CliffordString`, and `Tableau` values, public `PauliStringRef` parity is not required unless later parity or performance work proves it necessary, and Python-only binding behavior remains semantic-mining input until the Python API milestone. `crates/stab-core/tests/stabilizers.rs` now checks that real `PauliString` rejects imaginary, lowercase, and sparse-style text while `FlexPauliString` accepts phase-general dense and sparse text with canonical display; manifest rows `coverage-stabilizers-pauli-string`, `coverage-stabilizers-flex-pauli-string`, and `coverage-stabilizers-pauli-string-ref` track the split.

## 2026-06-27 - M4: Gate Decomposition Utility Scope

Status: Resolved
Revealed by: implementation of `coverage-circuit-gate-decomposition` as a direct Rust oracle row.
Current text: M4 links `src/stim/circuit/gate_decomposition.test.cc` under Circuit Model, Parser, Targets, And Decomposition, but M4's objective is the public `.stim` data model, gate metadata, parser, validator, and canonical printer.
Gap: the upstream file mixes pure circuit-structure helpers, such as target grouping and disjoint segmentation, with semantic MPP/SPP decomposition behavior that later depends on base-gate decomposition, flows, tableaus, and simulator correctness.
Proposed amendment: state that M4 owns structural decomposition prerequisites only, including Pauli-product grouping and disjoint target segmentation; full `decomposed` behavior for MPP, SPP, pair measurements, and base-gate lowering should move to the first milestone that implements the required tableau/simulator semantics or receive its own explicit milestone task.
Resolution: M4 now explicitly owns only structural decomposition prerequisites through `coverage-circuit-gate-decomposition`: target grouping and disjoint segmentation. Full semantic `decomposed` behavior for MPP, SPP, pair measurements, base-gate lowering, and tableau or simulator equivalence is assigned to the first milestone with the required algebra, flow, simulator, or analyzer semantics, including the M6 util-top rows and later detector/analyzer milestones. Evidence is `cargo test -p stab-core gate_decomposition --quiet` and the implemented manifest row.

## 2026-06-27 - M4: Probability Utility Fixture Scope

Status: Resolved
Revealed by: implementation of `coverage-util-bot-probability-util` as a direct Rust oracle row.
Current text: M4 requires gate argument rules and probability validation, while the test-porting plan points at `src/stim/util_bot/probability_util.test.cc` for probability validation.
Gap: the referenced upstream file also tests `sample_hit_indices` and biased random bit generation, which require RNG and bit-storage behavior that M4 does not otherwise define.
Proposed amendment: state that M4 owns only closed-unit probability validation and disjoint probability-list validation from this file; random hit-index sampling and biased bit generation should move to the first milestone that introduces equivalent RNG and bit/sampler APIs.
Resolution: M4 now owns only closed-unit probability validation and disjoint probability-list validation from `src/stim/util_bot/probability_util.test.cc`. Random hit-index sampling and biased random bit generation are excluded from M4 acceptance and are assigned to the first bit or sampler milestone that consumes equivalent APIs, plus M12 performance hardening when those utilities become benchmark targets. Evidence is `cargo test -p stab-core probability --quiet` and the implemented `coverage-util-bot-probability-util` manifest row.

## 2026-06-27 - M2: Manifest-Only Subcase Granularity

Status: Resolved
Revealed by: milestone audit of the M2 manifest coverage rows.
Current text: M2 and the test-porting plan allow red or manifest-only oracle cases for all P0 and P1 files needed by M4 through M11.
Gap: file-level manifest-only rows can satisfy coverage without identifying the upstream subcases, fixture families, malformed-input cases, or extraction criteria that future implementation milestones must port.
Proposed amendment: require manifest-only rows to name planned subcase groups or extraction criteria for each upstream test file before the owning implementation milestone starts.
Resolution: M2 now requires every manifest-only row to identify planned subcase groups, fixture families, malformed-input classes, or extraction criteria in its manifest note, and file-level placeholders must be split or updated before the owning implementation milestone starts. The remaining manifest-only M9 detector-analysis rows now name their planned subcase groups for detecting regions, missing detectors, and feedback inlining. Evidence is `rg ",manifest-only," oracle/fixtures/manifest.csv` and `just oracle::list --milestone M9`.

## 2026-06-26 - M0: Upstream Smoke References Overreach

Status: Resolved
Revealed by: milestone audit of the M0 oracle lab implementation.
Current text: M0 links `src/stim.test.cc`, `src/stim/main_namespaced.test.cc`, and `src/stim_included_twice.test.cc` as C++ smoke references.
Gap: those upstream files include behavior from later milestones, including circuit parsing, gate metadata, analyzer behavior, and richer CLI mode handling, so treating the full files as M0 requirements would pull M4, M6, and M10 work into the foundation milestone.
Proposed amendment: clarify that M0 extracts only oracle-process smoke checks from these files, specifically help-command health, main binary namespacing health, and one tiny deterministic circuit case; all parser, gate table, analyzer, and broader CLI behavior stays with later milestones.
Resolution: M0 now extracts only oracle-process smoke checks from the upstream smoke references: help-command health, binary namespacing or inclusion health, and one tiny deterministic circuit case. Full parser behavior, gate metadata, analyzer behavior, and broader CLI mode handling stay with their owning implementation milestones. Evidence is `just oracle::run --case smoke/help`, `just oracle::run --case smoke/tiny-circuit`, and the M0 linked-test text in `docs/plans/rust-stim-drop-in-rewrite.md`.

## 2026-06-26 - M0: Oracle Tiny Sample Shim Boundary

Status: Resolved
Revealed by: milestone audit and full-code-review of the M0 `stab-cli sample` smoke shim.
Current text: M0 requires `just oracle::run --case smoke/tiny-circuit`, while the CLI compatibility order defers real `sample` support to M8.
Gap: the plan does not say whether a minimal M0 sample command counts as CLI compatibility or is only an oracle fixture target.
Proposed amendment: state that any M0 sample path is an oracle-only smoke shim and does not count as implemented `stim sample` compatibility; M8 remains responsible for the public `sample` command contract.
Resolution: The M0 roadmap now states that any M0 `sample` path is an oracle-only smoke shim for `smoke-tiny-circuit` and does not count as public `stim sample` CLI compatibility. M8 remains responsible for the real `sample` command contract. Evidence is `just oracle::run --case smoke/tiny-circuit`, the M8 `sample` milestone tasks, and the CLI compatibility order.

## 2026-06-26 - M0: Benchmark Smoke Before Benchmark Harness

Status: Resolved
Revealed by: milestone audit and full-code-review of `just bench::smoke`.
Current text: M0 requires CI benchmark smoke tests, while M3 owns the benchmark package, baseline measurements, benchmark matrix, and performance contracts.
Gap: before M3, benchmark smoke can only prove workspace wiring unless the plan requires an explicit placeholder benchmark target.
Proposed amendment: clarify whether M0 benchmark smoke is compile-only workspace smoke or require a tiny explicit benchmark target that is intentionally replaced by the M3 benchmark harness.
Resolution: M0 now defines `just bench::smoke` as a compile and wiring smoke for benchmark operations only. It must not claim benchmark baselines, performance thresholds, or workload parity before M3 creates the real benchmark package, baseline commands, and benchmark matrix. Evidence is `just bench::smoke` and the M3 benchmark-baseline milestone text.
