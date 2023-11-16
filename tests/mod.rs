use std::any::type_name;
use std::fs::File;

use srs_to_dfa::solver::*;
use srs_to_dfa::builder::*;
use srs_to_dfa::util::DFA;
use srs_to_dfa::test::*;
use srs_to_dfa::util::Ruleset;

#[test]
#[ignore = "expensive"]
fn bfs_solver() {
    test_standard_examples::<BFSSolver>();
    test_cyclic_examples::<BFSSolver>();
}

#[test]
#[ignore = "expensive"]
fn hash_solver() {
    test_standard_examples::<HashSolver>();
    test_cyclic_examples::<HashSolver>();
}

#[test]
fn minkid_solver() {
    test_standard_examples::<MinkidSolver>();
    test_cyclic_examples::<MinkidSolver>();
}

#[test]
fn subset_solver() {
    test_standard_examples::<SubsetSolver>();
}

#[ignore = "expensive"]
#[test]
fn complete_minkid_solver() {
    test_standard_examples::<MinkidSolver>();
    test_cyclic_examples::<MinkidSolver>();
    test_intensive_examples::<MinkidSolver>();
}

#[test]
#[ignore = "expensive"]
fn complete_subset_solver() {
    test_standard_examples::<SubsetSolver>();
    test_intensive_examples::<SubsetSolver>();
}

#[test]
fn correctness_check() {
    let solve_test = build_default1dpeg::<MinkidSolver>().unwrap();
    assert_k(&solve_test,5,"1dpeg");
    let solve_test = build_threerule1dpeg::<MinkidSolver>().unwrap();
    assert_k(&solve_test,4,"threerule1dpeg");
    let solve_test = build_defaultsolver::<MinkidSolver>().unwrap();
    assert_k(&solve_test,6,"defaultsolver");

    let solve_test = build_threerulesolver::<MinkidSolver>().unwrap();
    assert_k(&solve_test,5,"threerulesolver");
    let solve_test = build_flip::<MinkidSolver>().unwrap();
    assert_k(&solve_test,2,"flip");

    let solve_test = build_flipx3::<MinkidSolver>().unwrap();
    assert_k(&solve_test,2,"flipx3");
    let solve_test = build_2xnswap::<MinkidSolver>().unwrap();
    assert_k(&solve_test,11,"2xnswap");
}

//TODO: Make a couple nice iterator functions to make this less repetitive and to make the addition of more examples easier

#[test]
fn ruleset_parsing() {
    //TODO: Make this work for all examples
    
    let solve_test = build_default1dpeg::<MinkidSolver>().unwrap();
    assert!(solve_test.get_ruleset() == &Ruleset::from_string(&solve_test.get_ruleset().to_string()),"default1dpeg ruleset failed to recreate itself");

    let solve_test = build_threerule1dpeg::<MinkidSolver>().unwrap();
    assert!(solve_test.get_ruleset() == &Ruleset::from_string(&solve_test.get_ruleset().to_string()),"threerule1dpeg ruleset failed to recreate itself");

    let solve_test = build_defaultsolver::<MinkidSolver>().unwrap();
    assert!(solve_test.get_ruleset() == &Ruleset::from_string(&solve_test.get_ruleset().to_string()),"defaultsolver ruleset failed to recreate itself");

    let solve_test = build_threerulesolver::<MinkidSolver>().unwrap();
    assert!(solve_test.get_ruleset() == &Ruleset::from_string(&solve_test.get_ruleset().to_string()),"threerulesolver ruleset failed to recreate itself");
}

fn assert_k<S>(solver : &S, k : usize, test_string : &str) where S : Solver {
    let final_dfa = solver.run(k);
    for bad_k in 1..k {
        let bad_dfa = solver.run(bad_k);
        let is_superset = solver.is_superset(&bad_dfa).is_ok();
        assert!((bad_dfa >= final_dfa) as usize >= is_superset as usize, "Incorrectly deemed superset for {} when k = {}",test_string, bad_k);
        let is_correct = solver.is_correct(&bad_dfa);
        assert!(bad_dfa != final_dfa, "Incorrect k for {}, {} works fine",test_string,bad_k);
        assert!(!is_correct,"Incorrectly deemed correct for {} when k = {}",test_string, bad_k);
        
    }
    assert!(solver.is_correct(&final_dfa),"Incorrectly deemed incorrect for {}",test_string);
}