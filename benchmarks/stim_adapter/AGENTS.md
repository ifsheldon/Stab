# Pinned Stim Adapter Instructions

- Treat this directory as benchmark infrastructure only; do not create a Stab C++ API or header-compatibility promise here.
- Keep the C++ adapter pinned to Stim v1.16.0 and keep its JSON Lines fields, semantic work, start barrier, affinity checks, timing boundary, and memory boundary symmetric with the Rust qualification worker.
- New product workloads require exact correctness preflight, source-owned work and output contracts, bounded inputs and outputs, and report validation before their ratios can be promotable.
- Update `README.md`, the performance qualification plan, runner tests, and adapter build-receipt validation whenever the adapter protocol or build contract changes.
