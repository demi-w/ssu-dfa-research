use std::collections::{HashMap, HashSet};

use bitvec::vec::BitVec;
use petgraph::{prelude::DiGraph, adj::NodeIndex};

use crate::util::SymbolIdx;
pub enum DFAStructure {
    Dense(Vec<Vec<usize>>),
    Graph(DiGraph::<(),SymbolIdx>)
}

impl DFAStructure {
    pub fn len(&self) -> usize {
        match self {
            Self::Dense(array) => {array.len()},
            Self::Graph(graph) => {graph.node_count()}
        }
    }
}

pub enum SSStructure {
    Boolean(Vec<BitVec>),
    BooleanMap(HashMap<BitVec,usize>),
    Minkid(Vec<HashSet<NodeIndex>>)
}