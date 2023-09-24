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
fn complet_minkid_solver() {
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

//TODO: Make a couple nice iterator functions to make this less repetitive and to make the addition of more examples easier

#[test]
fn ruleset_parsing() {
    //TODO: Make this work for all examples
    
    let solve_test = build_default1dpeg::<MinkidSolver>();
    assert!(solve_test.get_ruleset() == &Ruleset::from_string(&solve_test.get_ruleset().to_string()),"default1dpeg ruleset failed to recreate itself");

    let solve_test = build_threerule1dpeg::<MinkidSolver>();
    assert!(solve_test.get_ruleset() == &Ruleset::from_string(&solve_test.get_ruleset().to_string()),"threerule1dpeg ruleset failed to recreate itself");

    let solve_test = build_defaultsolver::<MinkidSolver>();
    assert!(solve_test.get_ruleset() == &Ruleset::from_string(&solve_test.get_ruleset().to_string()),"defaultsolver ruleset failed to recreate itself");

    let solve_test = build_threerulesolver::<MinkidSolver>();
    assert!(solve_test.get_ruleset() == &Ruleset::from_string(&solve_test.get_ruleset().to_string()),"threerulesolver ruleset failed to recreate itself");
}