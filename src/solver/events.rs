use std::collections::{HashMap, HashSet};

use bitvec::vec::BitVec;
use petgraph::prelude::DiGraph;

use crate::util::{Ruleset, SymbolIdx};
use crate::DFA;
pub enum DFAStructure {
    Dense(Vec<Vec<usize>>),
    Graph(DiGraph<(), SymbolIdx>),
}

impl DFAStructure {
    pub fn len(&self) -> usize {
        match self {
            Self::Dense(array) => array.len(),
            Self::Graph(graph) => graph.node_count(),
        }
    }
    pub fn to_dense(&self, symset_len: usize) -> Vec<Vec<usize>> {
        match self {
            Self::Dense(d) => d.clone(),
            Self::Graph(dfa_graph) => {
                let mut trans_table = vec![vec![0; symset_len]; dfa_graph.node_count()];
                for node in dfa_graph.node_indices() {
                    for edge in dfa_graph.edges_directed(node, petgraph::Direction::Outgoing) {
                        trans_table[node.index()][*edge.weight() as usize] =
                            petgraph::visit::EdgeRef::target(&edge).index();
                    }
                }
                trans_table
            }
        }
    }
}

pub enum SSStructure {
    Boolean(Vec<BitVec>),
    BooleanMap(HashMap<BitVec, usize>),
}
//TODO: Genericize this as well
impl SSStructure {
    pub fn accepting_states(&self) -> Vec<bool> {
        let mut result = vec![];
        match self {
            Self::Boolean(vec) => {
                for (idx, ss) in vec.iter().enumerate() {
                    result.push(ss[0]);
                }
            }
            Self::BooleanMap(map) => {
                result = vec![false; map.keys().len()];
                for (ss, idx) in map {
                    result[*idx] = ss[0];
                }
            }
        }
        result
    }
    pub fn element_len(&self) -> usize {
        match self {
            Self::Boolean(vec) => vec[0].len(),
            Self::BooleanMap(map) => map.iter().next().unwrap().0.len(),
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Self::Boolean(vec) => vec.len(),
            Self::BooleanMap(map) => map.len(),
        }
    }
}

pub fn event_to_dfa(dfa_s: &DFAStructure, sig_sets: &SSStructure, rules: &Ruleset) -> DFA {
    let trans_table = dfa_s.to_dense(rules.symbol_set.length);
    let accepting_states = sig_sets.accepting_states();
    DFA {
        starting_state: 0,
        state_transitions: trans_table,
        accepting_states: accepting_states,
        symbol_set: rules.symbol_set.clone(),
    }
}
