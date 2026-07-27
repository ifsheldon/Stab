# ADR 0004: Diagnostics And Resource Policies

## Status

Accepted for Stab 0.2 architecture work.

## Context

Current errors frequently carry unstructured messages, and safety limits are distributed as implementation constants.

People need readable errors while agents need stable codes, spans, and context.

Researchers also need to distinguish semantic limits, safe defaults, backend limits, materialization limits, and chosen experiment budgets.

## Decision

Domain crates expose typed errors and losslessly convert them into structured diagnostics.

Parser locations use byte spans.

CLI human output remains the default and JSON schema version 1 is additive.

Established human parser output remains stable where the old behavior already matches pinned Stim. Direct Stim v1.16.0 evidence takes precedence over preserving an incompatible old Stab acceptance or rejection.

Resource policies are owned by one concrete operation and preserve established safe acceptance and first-rejection behavior. A dimension with no prior aggregate rejection may default to the representable maximum only when compact input cannot cause unbounded traversal, retention, or allocation; otherwise its owner defines and tests a finite operation-safety default.

The first concrete policies are parsing, circuit flattening, DEM flattening, detection conversion, DEM sampling, logical-error search, and SAT materialization. They are not aliases of one global limits structure.

A constant is not automatically a public policy field. Representation bounds, parser-recursion envelopes, platform limits, and fixed algorithm invariants remain private when callers cannot safely raise them or gain useful control by lowering them.

Semantic hard limits are not configurable.

Estimates label values as exact, upper-bound, or unknown and do not execute the requested workload.

## Consequences

- Tools can react to failures without parsing English.
- Existing safety boundaries remain explicit and testable.
- Policy objects do not become a global configuration dependency.
- Callers see only limits relevant to the operation they invoke.
- New operation policies require a real admission boundary, exact default and first-rejection tests, and structured `ResourceLimitError` context.
- Admission precedes allocation proportional to rejected work, RNG advancement, output mutation, and expensive lowering; bounded capacity estimation from an admitted source prefix remains an implementation optimization.
