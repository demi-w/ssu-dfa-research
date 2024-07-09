use std::any::type_name;
use std::fs::File;

use crate::solver::*;
use crate::builder::*;
use crate::util::DFA;
#[cfg(not(target_arch = "wasm32"))]
pub fn test_standard_examples<S : SRSSolver>() {
    let solve_test = build_default1dpeg::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(5,vec![])),"1dpeg failed for {}",type_name::<S>());

    let solve_test = build_threerule1dpeg::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(5,vec![])),"threerule1dpeg failed for {}",type_name::<S>());

    let solve_test = build_defaultsolver::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(6,vec![])),"defaultsolver failed for {}",type_name::<S>());

    let solve_test = build_threerulesolver::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(5,vec![])),"threerulesolver failed for {}",type_name::<S>());
}
#[cfg(not(target_arch = "wasm32"))]
pub fn test_cyclic_examples<S : SRSSolver>() {
    let solve_test = build_flip::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(2,vec![])));

    let solve_test = build_flipx3::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(2,vec![])));
}
#[cfg(not(target_arch = "wasm32"))]
pub fn test_intensive_examples<S : SRSSolver>() {
    let solve_test = build_2xnswap::<S>().unwrap();
    assert!(solve_test.is_correct(&solve_test.run(10,vec![])));
}