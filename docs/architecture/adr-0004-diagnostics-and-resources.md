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

Resource policies are operation-specific and preserve current defaults.

Semantic hard limits are not configurable.

Estimates label values as exact, upper-bound, or unknown and do not execute the requested workload.

## Consequences

- Tools can react to failures without parsing English.
- Existing safety boundaries remain explicit and testable.
- Policy objects do not become a global configuration dependency.
