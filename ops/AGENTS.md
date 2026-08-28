# Instructions For Ops Tools

- Keep operational branching, validation, report generation, release control, and subprocess orchestration in Rust binaries under `ops/`. Keep `justfiles/` recipes thin and declarative.
- `ops/oracle` owns pinned Stim validation, behavior fixtures, result-format differential testing, gate metadata comparison, and the atomic parity ledger commands. Read [../docs/README.md](../docs/README.md) before changing these workflows.
- `ops/bench` owns the single end-to-end performance suite and shared bounded process supervisor. Read [../benchmarks/AGENTS.md](../benchmarks/AGENTS.md) before changing it.
- `ops/architecture` validates crate and consumer boundaries. `ops/release` owns coordinated publication safety. `ops/pre-commit` owns staged checks and instruction-document policy.
- Treat subprocesses, outputs, repository paths, and generated artifacts as hostile at their declared boundaries. Preserve bounded concurrent I/O, process-group cleanup, typed paths, non-reused artifact destinations, and clear domain errors.
- Do not recreate broad qualification inventories, receipt trees, duplicate benchmark ledgers, or shell-script workflow logic.
- Run targeted tool tests while iterating, then the workspace checks and matching `just` contract command before a requested commit.
