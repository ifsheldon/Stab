# ADR 0004: Diagnostics And Resource Policies

## Status

Accepted for Stab 0.2 architecture work. Revised on 2026-11-14 by the limit policy reset, which the [resource policy inventory](resource-policy-inventory.md) records constant by constant.

## Context

Current errors frequently carry unstructured messages, and safety limits are distributed as implementation constants.

People need readable errors while agents need stable codes, spans, and context.

Researchers also need to distinguish semantic limits, safe defaults, backend limits, materialization limits, and chosen experiment budgets.

## Decision

Domain crates expose typed errors and losslessly convert them into structured diagnostics.

Parser locations use byte spans.

CLI human output remains the default and JSON schema version 1 is additive.

Established human parser output remains stable where the old behavior already matches pinned Stim. Direct Stim v1.16.0 evidence takes precedence over preserving an incompatible old Stab acceptance or rejection.

Resource policies are owned by one concrete operation.
Each materializing or executing operation has exactly one admission dimension, a typed `MemoryBudget` of retained bytes, plus the shared repeat-nesting envelope where its traversal recurses.
The budget defaults to one gibibyte, is admitted from conservative checked arithmetic before allocation, RNG advancement, or output, and is followed by fallible reservation.

Hostile-input protection means memory safety, no panics, checked arithmetic, bounded allocation, and bounded recursion.
It does not mean bounded CPU time: count caps that only proxied memory and fixed work caps on expanded instructions, repeat iterations, sampled errors, replay work, or analysis work units are removed, and a compact input that fits the budget runs for as long as its work takes.
Bounding running time belongs to the caller, through one cloneable `stab_model::Cancellation` signal that every long-running library operation accepts on its limits or options value and polls inside repeat interpretation and between batches; a set signal yields a typed `Cancelled` outcome and never a partially delivered batch.

The per-operation limits types for parsing, circuit flattening, DEM flattening, detection conversion, DEM sampling, sampling, error analysis, and circuit generation each carry the one budget.
Logical-error search and SAT materialization keep their explicit search budgets with budget-exhausted results.
These policies are not aliases of one global limits structure, although the CLI feeds all of them from one `--memory-budget` option.

`stab-algebra` depends on no model crate, so it owns a typed `AlgebraMemoryBudget` with the same one-gibibyte default for values built from a requested size, such as a Tableau, a random Tableau, or the stabilizer solver's working rows; `stab-analysis` maps its `MemoryBudget` onto it, and its callers poll cancellation between algebra calls, each of which is polynomial in bytes already admitted.
Algebra operations whose storage is a bounded multiple of operands the caller already holds reserve it fallibly without a separate admission.

A constant is not automatically a public policy field.
Representation bounds, the parser-recursion envelope, platform limits, and implementation thresholds that select an algorithm without rejecting work stay private.

Semantic hard limits are not configurable.

Estimates label values as exact, upper-bound, or unknown and do not execute the requested workload.

## Consequences

- Tools can react to failures without parsing English.
- Existing safety boundaries remain explicit and testable.
- Policy objects do not become a global configuration dependency.
- Callers see only limits relevant to the operation they invoke.
- New operation policies require a real byte-admission boundary, exact-budget and first-excess tests, and structured `ResourceLimitError` context naming the budget.
- Long-running work on compact inputs is expected; callers bound time with cooperative cancellation, which the CLI does not expose because the process already stops on signals.
- Admission precedes allocation proportional to rejected work, RNG advancement, output mutation, and expensive lowering; bounded capacity estimation from an admitted source prefix remains an implementation optimization.
