# Instructions for Benchmark Contracts

- Treat `manifest.csv` as the source-owned benchmark contract manifest for M3 and later performance work.
- Keep benchmark rows explicit about owning milestone, threshold class, runner, upstream source, phase, measurement family, and description.
- Use `contract-only` only when there is no direct pinned C++ executable runner yet, and keep those rows tied to an upstream source or future compatibility-matrix anchor.
- Keep compare-note prefixes aligned with the benchmark comparability taxonomy in `README.md`; every primary row must resolve to a machine-readable class before it can count as M12 evidence.
- Do not write generated benchmark outputs in this directory; generated artifacts belong under `target/benchmarks/`.
- When changing the manifest schema, runner meanings, generated artifact locations, or benchmark workflow, update `README.md`, the root operational docs, and the roadmap or spec-gap log as appropriate.
