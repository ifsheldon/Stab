use crate::{Circuit, CircuitResult, CompiledSampler};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReferenceSampleTree {
    pub prefix_bits: Vec<bool>,
    pub suffix_children: Vec<ReferenceSampleTree>,
    pub repetitions: u64,
}

impl ReferenceSampleTree {
    pub fn from_circuit_reference_sample(circuit: &Circuit) -> CircuitResult<Self> {
        let sampler = CompiledSampler::compile_allowing_sweep(circuit)?;
        Ok(Self {
            prefix_bits: sampler.reference_sample(),
            suffix_children: Vec::new(),
            repetitions: 1,
        })
    }

    pub fn size(&self) -> usize {
        let body_size = self.body_size();
        usize::try_from(self.repetitions)
            .ok()
            .and_then(|repetitions| body_size.checked_mul(repetitions))
            .unwrap_or(usize::MAX)
    }

    pub fn get(&self, index: usize) -> Option<bool> {
        let body_size = self.body_size();
        if body_size == 0 || index >= self.size() {
            return None;
        }
        let mut remaining = index % body_size;
        if remaining < self.prefix_bits.len() {
            return self.prefix_bits.get(remaining).copied();
        }
        remaining -= self.prefix_bits.len();
        for child in &self.suffix_children {
            let child_size = child.size();
            if remaining < child_size {
                return child.get(remaining);
            }
            remaining -= child_size;
        }
        None
    }

    pub fn decompress(&self) -> Vec<bool> {
        let mut out = Vec::with_capacity(self.size());
        self.decompress_into(&mut out);
        out
    }

    pub fn decompress_into(&self, out: &mut Vec<bool>) {
        for _ in 0..self.repetitions {
            out.extend_from_slice(&self.prefix_bits);
            for child in &self.suffix_children {
                child.decompress_into(out);
            }
        }
    }

    pub fn simplified(&self) -> Self {
        let mut children = self
            .suffix_children
            .iter()
            .map(Self::simplified)
            .filter(|child| child.size() != 0)
            .collect::<Vec<_>>();
        if self.prefix_bits.is_empty() && children.len() == 1 {
            let mut child = children.remove(0);
            child.repetitions = child.repetitions.saturating_mul(self.repetitions);
            return child;
        }
        if self.repetitions == 0 || (self.prefix_bits.is_empty() && children.is_empty()) {
            return Self::default();
        }
        Self {
            prefix_bits: self.prefix_bits.clone(),
            suffix_children: children,
            repetitions: self.repetitions,
        }
    }

    pub fn stim_string(&self) -> String {
        let mut out = format!("{}*('", self.repetitions);
        for bit in &self.prefix_bits {
            out.push(if *bit { '1' } else { '0' });
        }
        out.push('\'');
        for child in &self.suffix_children {
            out.push('+');
            out.push_str(&child.stim_string());
        }
        out.push(')');
        out
    }

    fn body_size(&self) -> usize {
        self.prefix_bits
            .len()
            .saturating_add(self.suffix_children.iter().map(Self::size).sum::<usize>())
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        reason = "reference-sample-tree tests use compact upstream-style assertions"
    )]

    use super::*;

    #[test]
    fn reference_sample_tree_matches_upstream_equality_and_string_subset() {
        let empty1 = ReferenceSampleTree {
            prefix_bits: Vec::new(),
            suffix_children: Vec::new(),
            repetitions: 0,
        };
        let empty2 = ReferenceSampleTree::default();
        assert_eq!(empty1, empty2);
        assert_ne!(
            empty1,
            ReferenceSampleTree {
                repetitions: 1,
                ..ReferenceSampleTree::default()
            }
        );
        assert_ne!(
            empty1,
            ReferenceSampleTree {
                prefix_bits: vec![false],
                ..ReferenceSampleTree::default()
            }
        );
        assert_eq!(empty1.stim_string(), "0*('')");
        assert_eq!(
            ReferenceSampleTree {
                prefix_bits: vec![true, true, false, true],
                suffix_children: Vec::new(),
                repetitions: 2,
            }
            .stim_string(),
            "2*('1101')"
        );
        assert_eq!(
            ReferenceSampleTree {
                prefix_bits: vec![true, true, false, true],
                suffix_children: vec![ReferenceSampleTree {
                    prefix_bits: vec![true],
                    suffix_children: Vec::new(),
                    repetitions: 5,
                }],
                repetitions: 2,
            }
            .stim_string(),
            "2*('1101'+5*('1'))"
        );
    }

    #[test]
    fn reference_sample_tree_simplifies_empty_and_zero_repetition_children() {
        let raw = ReferenceSampleTree {
            prefix_bits: Vec::new(),
            suffix_children: vec![
                ReferenceSampleTree {
                    prefix_bits: Vec::new(),
                    suffix_children: Vec::new(),
                    repetitions: 1,
                },
                ReferenceSampleTree {
                    prefix_bits: vec![true, false, true],
                    suffix_children: vec![ReferenceSampleTree::default()],
                    repetitions: 0,
                },
                ReferenceSampleTree {
                    prefix_bits: vec![true, true, true],
                    suffix_children: Vec::new(),
                    repetitions: 2,
                },
            ],
            repetitions: 3,
        };
        assert_eq!(raw.simplified().stim_string(), "6*('111')");
    }

    #[test]
    fn reference_sample_tree_decompresses_and_supports_random_access() {
        let tree = ReferenceSampleTree {
            prefix_bits: vec![true, true, false, true],
            suffix_children: vec![ReferenceSampleTree {
                prefix_bits: vec![true],
                suffix_children: Vec::new(),
                repetitions: 5,
            }],
            repetitions: 2,
        };
        let expected = vec![
            true, true, false, true, true, true, true, true, true, true, true, false, true, true,
            true, true, true, true,
        ];
        assert_eq!(tree.decompress(), expected);
        for (index, bit) in expected.iter().copied().enumerate() {
            assert_eq!(tree.get(index), Some(bit), "index {index}");
        }
        assert_eq!(tree.get(expected.len()), None);
    }

    #[test]
    fn reference_sample_tree_from_circuit_matches_sampler_reference_sample() {
        let circuit = Circuit::from_stim_str(
            "
            M 0
            X 0
            M 0
            ",
        )
        .expect("parse circuit");
        let tree =
            ReferenceSampleTree::from_circuit_reference_sample(&circuit).expect("reference tree");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
        assert_eq!(tree.decompress(), sampler.reference_sample());
        assert_eq!(tree.size(), 2);
        assert_eq!(tree.stim_string(), "1*('01')");
    }
}
