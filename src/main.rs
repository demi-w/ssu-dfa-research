#![warn(unused_crate_dependencies)]
pub mod util;
pub mod solver;
pub mod builder;
use crate::solver::{MinkidSolver, Solver};
use crate::util::*;



fn main() {
    let solver = builder::build_threerulesolver::<MinkidSolver>();
    let dfa_result = solver.run_with_print(5);
    let tried_n_true = DFA::load("dfaresults/2xnswap").unwrap();
    assert!(dfa_result == tried_n_true);
}