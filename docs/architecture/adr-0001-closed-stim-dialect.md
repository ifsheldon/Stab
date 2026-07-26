# ADR 0001: Stim Dialect Remains Closed

## Status

Accepted for Stab 0.2 architecture work.

## Context

Stab targets exact implemented compatibility with Stim v1.16.0.

The current static gate table enables exhaustive validation, lowering, testing, and optimization.

A dynamic string-keyed gate registry would weaken those properties and make compatibility claims ambiguous.

Researchers still need custom noise models, annotations, and experiment-specific behavior.

## Decision

`Circuit`, `DetectorErrorModel`, gate syntax, targets, and public result formats remain closed Stim-compatible models.

Custom behavior enters through typed passes that lower into a validated Stim-compatible circuit or DEM.

An operation that cannot be represented in the closed dialect is rejected with a structured unsupported-capability diagnostic.

No runtime gate registry is introduced.

## Consequences

- Existing parser, printer, gate, and oracle evidence remains authoritative.
- Passes can be independently developed and tested.
- Truly new executable operations require a later explicit model and IR decision instead of silently changing Stim semantics.
