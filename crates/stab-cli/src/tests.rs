//! In-process CLI tests, organized by command and by cross-cutting concern. Exact-output parity
//! with pinned Stim is owned by the oracle fixture manifest, which CI replays against the real
//! binaries; these tests cover Stab-owned contracts and behavior the manifest cannot express.

mod support;

mod analyze_errors;
mod convert;
mod detect;
mod generate; // `gen` is a reserved keyword in edition 2024.
mod m2d;
mod sample;
mod sample_dem;

mod agent;
mod args;
mod diagnostics;
mod help;
mod io_roles;
mod result_text_compat;
mod streaming_failures;
