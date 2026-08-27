# ADR 0007: Product Dependency Graph

## Status

Accepted for Stab 0.2.

## Context

Stab began with most product behavior in `stab-core`, which made ownership easy to discover locally but coupled Stable model and record consumers to the Nightly facade. The modular architecture needs a directed Cargo graph that keeps models independent of algorithms, keeps execution independent of codecs and filesystems, isolates portable SIMD, and prevents qualification tooling from becoming a product dependency.

A prose diagram is insufficient as the sole decision record because an apparently convenient dependency can introduce a cycle, move Nightly into a Stable consumer graph, or give two crates ownership of the same semantic contract.

## Decision

Product dependency arrows point from a consumer to its dependency:

```text
stab-kernels-simd -> no Stab crate

stab-bits --portable-simd--> stab-kernels-simd
stab-records -> stab-bits
stab-algebra -> stab-bits
stab-algebra --portable-simd--> stab-kernels-simd
stab-model -> no Stab crate
stab-analysis -> stab-model + stab-algebra
stab-engine -> stab-model + stab-records + stab-algebra + stab-analysis
stab-decoder -> stab-model + stab-records
stab-core -> stab-engine + stab-analysis + stab-model + stab-algebra + stab-bits + stab-records + stab-decoder
stab-cli -> stab-analysis + stab-bits + stab-engine + stab-model + stab-records

ops -> product crates
product crates -X-> ops
```

The `stab-decoder` edges are active after A7 adds the physical package and the public external-decoder proof. Architecture policy continues to reject every dependency outside the graph.

`stab-model` owns closed Stim circuit and detector-error-model syntax and structure. `stab-analysis` may depend on model and algebra semantics, while `stab-engine` may depend on analysis lowering; the inverse engine-to-analysis edge is forbidden. Records know packed result layouts but not circuits. SIMD kernels accept raw words and have no Stab dependency. `stab-cli` composes the owning components directly, while `stab-core` is an ergonomic facade for external consumers and not an internal service locator.

Stable components declare Rust 1.97.1 as their minimum supported version and must not reach `stab-kernels-simd` through default features or development dependencies. Nightly is explicit for the SIMD leaf, facade, CLI, and operational consumers that deliberately enable portable SIMD.

## Consequences

- A component can be consumed directly without compiling unrelated facade or CLI code.
- Pure analysis and mutable execution have one dependency direction and cannot share algorithms through a cycle.
- Qualification, benchmark, and repository operations can inspect product crates, but product crates cannot import operational policy.
- Adding or reversing a product edge requires an architecture decision update, exact consumer evidence, and architecture-check changes in the same change set.
- Facade convenience may require adapters, but an adapter cannot create a second canonical type or algorithm owner.

## Enforcement

`just architecture::check` validates the complete Cargo graph, reserved package identities, Stable and Nightly feature reachability, product-to-ops rejection, and portable-SIMD source ownership from `cargo metadata` and Rust syntax.

`just architecture::consumer-check` compiles standalone Stable component, scalar facade, portable facade, and mixed direct-component consumers and verifies their resolved feature graphs.

`just architecture::docs-check` validates repository-owned Markdown links and anchors so the architecture index and local evidence references cannot silently drift.

The canonical-owner qualification inventory requires component behavior to execute in its owning package unless a checked cross-component or facade-integration exception names the reason direct ownership is impossible.
