# PFM3 Sampler Sweep Target-Order Scope

## Scope

This slice owns one PFM3 execution-boundary subcase: sampler-backed execution surfaces must reject `CX q sweep[k]` and `CY q sweep[k]` instead of treating them as valid sweep-controlled Pauli operations.
Pinned Stim v1.16.0 parses these circuits, but sampler and measurement-to-detection execution reject them with measurement-record-editing style errors.
Stab should fail closed before sampling or detection conversion produces partial output.

## Owned Surfaces

- Core sampler-backed reference sampling through `Circuit::reference_sample`.
- Core measurement-to-detection conversion with sweep records.
- Core detection sampling validation for non-frame paths.
- Public `stab m2d --sweep` behavior where the CLI reaches the same conversion planner.

## Explicit Non-Scope

- Flow-generator and flow-solving semantics for `CX q sweep[k]` or `CY q sweep[k]`, because pinned Stim v1.16.0 accepts those circuits as flow no-ops.
- Detecting-region behavior, which already rejects the same target-order shape under PFM5.
- Analyzer behavior, which already rejects these invalid controlled-Pauli target positions under PFM3.
- New `detect --sweep` CLI support, Python detector-sampler APIs, JS/WASM, diagrams, GPU, public simulator products, and exact random-stream parity.

## Comparator And Evidence

Comparator class: structural Rust and CLI rejection parity against pinned Stim v1.16.0 execution behavior.
Pinned Stim probe:

```sh
uv run --with stim==1.16.0 python - <<'PY'
import stim
for text in ["CX 0 sweep[0]\nM 0\n", "CY 0 sweep[0]\nM 0\n"]:
    c = stim.Circuit(text)
    try:
        c.compile_sampler().sample(shots=1)
    except Exception as e:
        print(type(e).__name__, str(e).split("\n")[0])
PY
```

Both cases reject during execution with `Measurement record editing is not supported.`.
Stab does not need the exact message, but it must reject before producing sampler, detector-conversion, or CLI output.

## Tests

- Add core tests proving `Circuit::reference_sample`, default-sweep detection sampling validation, and `convert_measurements_to_detection_events_with_sweep` reject `CX q sweep[k]` and `CY q sweep[k]`.
- Add CLI tests proving `stab m2d --sweep` rejects the same shapes with empty stdout, nonzero status, and diagnostic stderr.
- Keep existing flow-generator tests for sweep-controlled Pauli no-ops unchanged.

## Oracle Rows

Add one structural oracle row for the core rejection subset:

- `pf3-sampler-sweep-target-order-rust`

The existing PF7 `m2d` rows continue to cover broad public command behavior; CLI tests in this slice are source-owned regression coverage rather than a new exact pinned-Stim CLI row.

## Benchmarks

No benchmark row is added.
This is a validation and fail-closed compatibility slice, not a throughput path.
Existing report-only PFM3 sweep and gate-semantic benchmark rows remain unchanged.

## Done Criteria

- `CX q sweep[k]` and `CY q sweep[k]` fail in sampler-backed execution surfaces.
- Accepted sweep-first `CX sweep[k] q`, `CY sweep[k] q`, and both-order `CZ` sweep/qubit behavior remain unchanged.
- Flow-generator semantics remain unchanged for the same parsed circuits.
- Documentation and oracle metadata name the exact promoted rejection behavior without claiming broader sweep-conditioned parity.
