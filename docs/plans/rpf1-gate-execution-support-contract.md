# RPF1 Gate Execution Support Contract

## Purpose

This document records the RPF1 parser, metadata, and execution-support split for every canonical Stim v1.16.0 gate currently known to Stab.
It prevents parser acceptance from being mistaken for sampler, detection-conversion, or analyzer parity.

## Status Meanings

- `Validation` means the gate exists in Stab's canonical gate registry and has argument and target validation.
- `Tableau`, `Unitary`, `Flow`, and `Decomposition` refer only to Rust gate metadata accessors.
- `Sampler` means `CompiledSampler` currently has an execution path for the gate, while `No-op` means the sampler accepts the instruction as metadata and emits no sampling operation.
- `Detection conversion` means the current measurement-to-detection conversion path can use the gate through metadata handling or the sampler-backed reference path; RPF3 and RPF7 still own broader sweep and CLI parity.
- `Analyzer` means `circuit_to_detector_error_model` currently has a code path for the gate, while `Capped/folded` records the current repeat handling policy and `Reject` records an explicit fail-closed gap.
- This table is not Python `GateData` parity and does not expose public simulator products.

## Canonical Gate Table

| Gate | Validation | Tableau | Unitary | Flow | Decomposition | Sampler | Detection conversion | Analyzer |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `DETECTOR` | Yes | No | No | No | No | No-op | Metadata | Metadata |
| `OBSERVABLE_INCLUDE` | Yes | No | No | No | No | No-op | Metadata | Metadata |
| `TICK` | Yes | No | No | No | No | No-op | Metadata | Metadata |
| `QUBIT_COORDS` | Yes | No | No | No | No | No-op | Metadata | Metadata |
| `SHIFT_COORDS` | Yes | No | No | No | No | No-op | Metadata | Metadata |
| `REPEAT` | Yes | No | No | No | No | Repeat | Repeat | Capped/folded |
| `MPAD` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `MX` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `MY` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `M` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `MRX` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `MRY` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `MR` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `RX` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `RY` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `R` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `XCX` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `XCY` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `XCZ` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `YCX` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `YCY` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `YCZ` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `CX` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `CY` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `CZ` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `H` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `H_XY` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `H_YZ` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `H_NXY` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `H_NXZ` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `H_NYZ` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `DEPOLARIZE1` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `DEPOLARIZE2` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `X_ERROR` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `Y_ERROR` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `Z_ERROR` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `I_ERROR` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `II_ERROR` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `PAULI_CHANNEL_1` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `PAULI_CHANNEL_2` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `E` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `ELSE_CORRELATED_ERROR` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `HERALDED_ERASE` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `HERALDED_PAULI_CHANNEL_1` | Yes | No | No | No | No | Yes | Sampler-backed | Yes |
| `I` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `X` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `Y` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `Z` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `C_XYZ` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `C_ZYX` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `C_NXYZ` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `C_XNYZ` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `C_XYNZ` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `C_NZYX` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `C_ZNYX` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `C_ZYNX` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `SQRT_X` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `SQRT_X_DAG` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `SQRT_Y` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `SQRT_Y_DAG` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `S` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `S_DAG` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `II` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `SQRT_XX` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `SQRT_XX_DAG` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `SQRT_YY` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `SQRT_YY_DAG` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `SQRT_ZZ` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `SQRT_ZZ_DAG` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `MPP` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `SPP` | Yes | No | No | Yes | Yes | Reject | Reject | Reject |
| `SPP_DAG` | Yes | No | No | Yes | Yes | Reject | Reject | Reject |
| `SWAP` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `ISWAP` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `CXSWAP` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `SWAPCX` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `CZSWAP` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `ISWAP_DAG` | Yes | Yes | Yes | Yes | Yes | Yes | Sampler-backed | Yes |
| `MXX` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `MYY` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |
| `MZZ` | Yes | No | No | Yes | Yes | Yes | Sampler-backed | Yes |

## Open Follow-Ups

- RPF3 owns deciding whether `SPP` and `SPP_DAG` execution should be implemented in the sampler and detection conversion paths or remain explicit rejections for the current Rust/CLI scope.
- RPF6 owns analyzer parity for `SPP`, `SPP_DAG`, generated-circuit coverage, and loop-folding evidence.
