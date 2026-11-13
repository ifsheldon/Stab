//! Independent encoder for `docs/architecture/model-fingerprint-schema-v1.md`.
//!
//! The encoder consumes a test-owned description of the expected model instead of walking Stab's
//! model accessors, so a product traversal or field-order bug cannot be mirrored here.

#![allow(
    dead_code,
    reason = "each integration test binary uses a different subset of the shared schema encoder"
)]

use sha2::{Digest, Sha256};

pub(crate) const CIRCUIT_DIALECT: u8 = 1;
pub(crate) const DEM_DIALECT: u8 = 2;

#[derive(Clone, Debug)]
pub(crate) enum SchemaHead {
    /// Canonical Stim gate name.
    Gate(&'static str),
    /// DEM instruction-kind discriminator from the schema table.
    DemKind(u8),
}

#[derive(Clone, Debug)]
pub(crate) enum SchemaTarget {
    Qubit { id: u32, inverted: bool },
    Record { offset: i32, negative_zero: bool },
    Sweep(u32),
    Pauli { pauli: u8, id: u32, inverted: bool },
    Combiner,
    Detector(u64),
    Observable(u64),
    Separator,
    Numeric(u64),
}

#[derive(Clone, Debug)]
pub(crate) enum SchemaItem {
    Instruction {
        head: SchemaHead,
        args: Vec<f64>,
        targets: Vec<SchemaTarget>,
        tag: Option<Vec<u8>>,
    },
    Repeat {
        count: u64,
        tag: Option<Vec<u8>>,
        body: Vec<SchemaItem>,
    },
}

pub(crate) fn schema_digest(dialect: u8, items: &[SchemaItem]) -> [u8; 32] {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"stab:model-fingerprint");
    bytes.push(0);
    bytes.extend_from_slice(&1_u16.to_be_bytes());
    bytes.push(dialect);
    encode_model(&mut bytes, items);
    Sha256::digest(&bytes).into()
}

fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(&(len as u128).to_be_bytes());
}

fn encode_byte_string(bytes: &mut Vec<u8>, value: &[u8]) {
    encode_len(bytes, value.len());
    bytes.extend_from_slice(value);
}

fn encode_optional_tag(bytes: &mut Vec<u8>, tag: Option<&[u8]>) {
    match tag {
        None => bytes.push(0),
        Some(tag) => {
            bytes.push(1);
            encode_byte_string(bytes, tag);
        }
    }
}

fn encode_model(bytes: &mut Vec<u8>, items: &[SchemaItem]) {
    encode_len(bytes, items.len());
    for item in items {
        match item {
            SchemaItem::Instruction {
                head,
                args,
                targets,
                tag,
            } => {
                bytes.push(1);
                match head {
                    SchemaHead::Gate(name) => encode_byte_string(bytes, name.as_bytes()),
                    SchemaHead::DemKind(kind) => bytes.push(*kind),
                }
                encode_len(bytes, args.len());
                for arg in args {
                    let bits = if *arg == 0.0 { 0 } else { arg.to_bits() };
                    bytes.extend_from_slice(&bits.to_be_bytes());
                }
                encode_len(bytes, targets.len());
                for target in targets {
                    encode_target(bytes, target);
                }
                encode_optional_tag(bytes, tag.as_deref());
            }
            SchemaItem::Repeat { count, tag, body } => {
                bytes.push(2);
                bytes.extend_from_slice(&count.to_be_bytes());
                encode_optional_tag(bytes, tag.as_deref());
                encode_model(bytes, body);
            }
        }
    }
}

fn encode_target(bytes: &mut Vec<u8>, target: &SchemaTarget) {
    match target {
        SchemaTarget::Qubit { id, inverted } => {
            bytes.push(1);
            bytes.extend_from_slice(&id.to_be_bytes());
            bytes.push(u8::from(*inverted));
        }
        SchemaTarget::Record {
            offset,
            negative_zero,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&offset.to_be_bytes());
            bytes.push(u8::from(*negative_zero));
        }
        SchemaTarget::Sweep(id) => {
            bytes.push(3);
            bytes.extend_from_slice(&id.to_be_bytes());
        }
        SchemaTarget::Pauli {
            pauli,
            id,
            inverted,
        } => {
            bytes.push(4);
            bytes.push(*pauli);
            bytes.extend_from_slice(&id.to_be_bytes());
            bytes.push(u8::from(*inverted));
        }
        SchemaTarget::Combiner => bytes.push(5),
        SchemaTarget::Detector(id) => {
            bytes.push(1);
            bytes.extend_from_slice(&id.to_be_bytes());
        }
        SchemaTarget::Observable(id) => {
            bytes.push(2);
            bytes.extend_from_slice(&id.to_be_bytes());
        }
        SchemaTarget::Separator => bytes.push(3),
        SchemaTarget::Numeric(value) => {
            bytes.push(4);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }
}
