# Pre-0.2 Architecture And API Baseline

This snapshot records the architecture that the Stab 0.2 migration starts from.

## Source Identity

- Repository revision: `cfaa1098fe7d37512b71bd2f5974196bbcdb14b9`
- Stim version: `v1.16.0`
- Stim revision: `e2fc1eca7fd21684d433aa5f10f4504ea4860d07`
- Rust toolchain: `nightly-2026-06-20`
- Correctness inventory schema: 3
- Correctness semantic digest: `7a0f0fd50bc46221d4c1b489f9bb3d52f0a2e8ced996087f5714c72699645c7b`
- Discovered public Rust API items: 2,065
- Correctness evidence parents: 1,759
- Upstream cases: 2,886

The checked owner inventory is `oracle/qualification-manifest.json`.

This document does not duplicate its item-level ledger.

## Package Graph

```text
stab-core
  -> stab-compat-corpus (dev only)

stab-cli
  -> stab-core
  -> stab-compat-corpus (dev only)

stab-bench
  -> stab-cli
  -> stab-core

stab-oracle
  -> stab-core
  -> stab-compat-corpus

stab-pre-commit
  -> no product crate
```

The only product library package is `stab-core`.

## Root Product Surface

`stab-core` reexports circuit and DEM models, bit storage, stabilizer algebra, sampling, detection conversion, DEM sampling, generation, transforms, flow analysis, search, SAT generation, error matching, result formats, and compatibility conveniences from one root.

The migration inventory must account for each checked public item before removing, renaming, moving, or replacing it.

Ordinary derived traits and trivial declarations remain inventory-owned but do not require low-value standalone runtime tests.

## Known Dependency Knots

- Gate syntax exposes tableaus, flows, unitaries, and decompositions while algebra accepts `Gate`.
- `Circuit` exposes model, analysis, transform, and sampling-backed inherent methods.
- Sampling and detection use both result codecs and shared gate-lowering helpers.
- `CircuitError` spans parser, model, algebra, format, execution, I/O, and DEM failures.
- Folded DEM traversal is private but used by model queries, analysis, search, and execution.
- Direct `std::simd` use exists in ordinary bit kernels and Clifford multiplication.
- `ops-contracts` exposes qualification-only data through a product feature.

These knots are migration inputs, not accepted target dependencies.

## Compatibility Checkpoint

The accepted pre-refactor controlled evidence remains recorded against clean source revision `68d107a42f655254f31628f0cbedc55479f6c0f3`.

Later documentation and benchmark-harness commits do not retroactively change that source identity.

The architecture migration must regenerate affected correctness and performance evidence from its own clean source revision before Stab 0.2 release claims.

## Regeneration Commands

```text
just qualification::correctness-check
just bench::qualification-check
just qualification::status --check
```
