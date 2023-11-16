use std::any::type_name;
use std::fs::File;

use crate::solver::*;
use crate::builder::*;
use crate::util::DFA;
#[cfg(not(target_arch = "wasm32"))]
pub fn test_standard_examples<S : Solver>() {
    let solve_test = build_default1dpeg::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(5)),"1dpeg failed for {}",type_name::<S>());

    let solve_test = build_threerule1dpeg::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(5)),"threerule1dpeg failed for {}",type_name::<S>());

    let solve_test = build_defaultsolver::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(6)),"defaultsolver failed for {}",type_name::<S>());

    let solve_test = build_threerulesolver::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(5)),"threerulesolver failed for {}",type_name::<S>());
}
#[cfg(not(target_arch = "wasm32"))]
pub fn test_cyclic_examples<S : Solver>() {
    let solve_test = build_flip::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(2)));

    let solve_test = build_flipx3::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(2)));
}
#[cfg(not(target_arch = "wasm32"))]
pub fn test_intensive_examples<S : Solver>() {
    let solve_test = build_2xnswap::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(10)));
}