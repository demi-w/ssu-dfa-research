use std::any::type_name;
use std::fs::File;

use crate::solver::*;
use crate::builder::*;
use crate::util::DFA;
#[cfg(not(target_arch = "wasm32"))]
pub fn test_standard_examples<S : Solver>() {
    let solve_test = build_default1dpeg::<S>();
    assert!(solve_test.run(5) == DFA::jflap_load(&mut File::open("jffresults/1dpeg.jff").unwrap()),"1dpeg failed for {}",type_name::<S>());

    let solve_test = build_threerule1dpeg::<S>();
    assert!(solve_test.run(5) == DFA::jflap_load(&mut File::open("jffresults/threerule1dpeg.jff").unwrap()),"threerule1dpeg failed for {}",type_name::<S>());

    let solve_test = build_defaultsolver::<S>();
    assert!(solve_test.run(5) == DFA::jflap_load(&mut File::open("jffresults/defaultsolver.jff").unwrap()),"defaultsolver failed for {}",type_name::<S>());

    let solve_test = build_threerulesolver::<S>();
    assert!(solve_test.run(5) == DFA::jflap_load(&mut File::open("jffresults/threerulesolver.jff").unwrap()),"threerulesolver failed for {}",type_name::<S>());
}
#[cfg(not(target_arch = "wasm32"))]
pub fn test_cyclic_examples<S : Solver>() {
    let solve_test = build_flip::<S>();
    assert!(solve_test.run(5) == DFA::jflap_load(&mut File::open("jffresults/flip.jff").unwrap()),"flip failed for {}",type_name::<S>());

    let solve_test = build_flipx3::<S>();
    assert!(solve_test.run(5) == DFA::jflap_load(&mut File::open("jffresults/flipx3.jff").unwrap()),"flipx3 failed for {}",type_name::<S>());
}
#[cfg(not(target_arch = "wasm32"))]
pub fn test_intensive_examples<S : Solver>() {
    let solve_test = build_flip::<S>();
    assert!(solve_test.run(11) == DFA::jflap_load(&mut File::open("jffresults/2xnswap.jff").unwrap()),"2xnswap failed for {}",type_name::<S>());
}