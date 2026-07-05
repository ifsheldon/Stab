# Generated API Docs Progress Report

## Scope

This slice closes the non-deferred Rust API documentation workflow gap without expanding into deferred Python bindings, Python stubs, JS/WASM docs, or a generated compatibility matrix.

## Implemented

- Added `just docs::api` to generate workspace Rust API docs under `target/doc/`.
- Added `just docs::api-check` to run workspace rustdoc with warnings denied.
- Registered the modular docs namespace in the root `justfile`.
- Fixed a rustdoc warning in the benchmark CLI comments so the strict check is meaningful.
- Documented the workflow in `README.md`.
- Updated `docs/stab-feature-checklist.md` to mark the Rust API reference workflow as implemented while keeping Python stubs and generated feature matrices out of scope.
- Reconciled the later high-priority gaps row so generated feature/status matrix tooling is marked deferred instead of leaving a duplicate active `Partial` row after the Rust API documentation workflow closed.

## Verification

Completed checks for this slice:

```sh
just docs::api
just docs::api-check
cargo fmt --all --check
cargo test -p stab-bench --quiet
just maintenance::pre-commit
```

## Remaining Future Documentation Surface

- Generated Python stubs remain deferred until Python bindings exist.
- Generated feature/status matrix tooling remains future work and should get its own source-of-truth plan before implementation.
