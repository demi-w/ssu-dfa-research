use std::collections::HashSet;

use crate::{util::{DFA, SymbolSet, Ruleset, SymbolIdx}, solver::Solver};



pub fn build_threerulesolver<S>() -> S where S: Solver {
    let b_symbol_set = SymbolSet {
        length : 3,
        representations : vec!["0".to_owned(),"1".to_owned(),"2".to_owned()]
    };
    let ruleset = Ruleset::from_vec(
        vec![(vec![1,1,0],vec![0,0,1]),
                     (vec![0,1,1],vec![1,0,0]),
                     (vec![1,0,1],vec![0,1,0]),
                     (vec![2,1,0],vec![0,0,2]),
                     (vec![0,1,2],vec![2,0,0]),
                     (vec![2,0,1],vec![0,2,0]),
                     (vec![1,0,2],vec![0,2,0]),
        ],
        b_symbol_set.clone()
    );
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,2,1],vec![1,2,2],vec![2,2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        symbol_set : b_symbol_set.clone()
    };
    S::new(ruleset,goal_dfa)
}

pub fn build_defaultsolver<S>() -> S where S: Solver {
    let b_symbol_set = SymbolSet {
        length : 3,
        representations : vec!["0".to_owned(),"1".to_owned(),"2".to_owned()]
    };
    let ruleset = Ruleset::from_vec(
        vec![(vec![1,1,0],vec![0,0,1]),
                     (vec![0,1,1],vec![1,0,0]),
                     (vec![2,1,0],vec![0,0,2]),
                     (vec![0,1,2],vec![2,0,0]),
        ],
        b_symbol_set.clone()
    );
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,2,1],vec![1,2,2],vec![2,2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        symbol_set : b_symbol_set.clone()
    };
    S::new(ruleset,goal_dfa)
}

pub fn build_2xnswap<S>() -> S where S: Solver {
    let symbol_set = SymbolSet {
        length : 3,
        representations : vec!["0".to_owned(),"1".to_owned(),"2".to_owned()]
    };

    let mut rules : Vec::<(Vec<SymbolIdx>,Vec<SymbolIdx>)> = vec![];

    for i in 0..(8 as SymbolIdx) {
        let big = (i / 4) % 2;
        let mid = (i / 2) % 2;
        let sml = i % 2;
        rules.push((vec![1+big,1+mid,0+sml],vec![0+big,0+mid,1+sml]));
        rules.push((vec![0+big,1+mid,1+sml],vec![1+big,0+mid,0+sml]));
    }

    let ruleset = Ruleset::from_vec(
        rules,
        symbol_set.clone()
    );

    let old_dfa = DFA::load(&mut std::fs::File::open("dfaresults/default1dpeg").unwrap()).unwrap();
    let mut new_transitions = vec![];

    let error_state = 10;
    for state in old_dfa.state_transitions {
        new_transitions.push(vec![state[0],state[1],error_state]);

    }
    let goal_dfa = DFA { 
        starting_state: 0, 
        state_transitions: new_transitions, 
        accepting_states: old_dfa.accepting_states, 
        symbol_set: symbol_set.clone()
     };
     S::new(ruleset,goal_dfa)
}

pub fn build_default1dpeg<S>() -> S where S: Solver {
    let b_symbol_set = SymbolSet {
        length : 2,
        representations : vec!["0".to_owned(),"1".to_owned()]
    };
    let ruleset = Ruleset::from_vec(
        vec![(vec![1,1,0],vec![0,0,1]),
                     (vec![0,1,1],vec![1,0,0]),
        ],
        b_symbol_set.clone()
    );
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1],vec![1,2],vec![2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        symbol_set : b_symbol_set.clone()
    };
    S::new(ruleset,goal_dfa)
}

pub fn build_threerule1dpeg<S>() -> S where S: Solver {
    let b_symbol_set = SymbolSet {
        length : 2,
        representations : vec!["0".to_owned(),"1".to_owned()]
    };
    let ruleset = Ruleset::from_vec(
        vec![(vec![1,1,0],vec![0,0,1]),
                     (vec![0,1,1],vec![1,0,0]),
                     (vec![1,0,1],vec![0,1,0])
        ],
        b_symbol_set.clone()
    );
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1],vec![1,2],vec![2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        symbol_set : b_symbol_set.clone()
    };
    S::new(ruleset,goal_dfa)
}

pub fn build_flip<S>() -> S where S: Solver {
    let b_symbol_set = SymbolSet {
        length : 2,
        representations : vec!["0".to_owned(),"1".to_owned()]
    };
    let mut rules_vec = vec![];
    for i in 0..8 {
        rules_vec.push((vec![i/4 % 2, i / 2 % 2, i % 2],vec![1-i/4 % 2, 1-i / 2 % 2, 1-i % 2]))
    }
    let ruleset = Ruleset::from_vec(
        rules_vec,
        b_symbol_set.clone()
    );
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1],vec![1,1]],
        accepting_states : HashSet::from_iter(vec![0]),
        symbol_set : b_symbol_set.clone()
    };
    S::new(ruleset,goal_dfa)
}

pub fn build_flipx3<S>() -> S where S: Solver {
    let b_symbol_set = SymbolSet {
        length : 2,
        representations : vec!["0".to_owned(),"1".to_owned()]
    };
    let mut rules_vec = vec![];
    for i in 0..8 {
        rules_vec.push((vec![i/4 % 2, i / 2 % 2, i % 2],vec![1-i/4 % 2, 1-i / 2 % 2, 1-i % 2]))
    }
    
    let k = 3;
    let symbol_num = 2_u32.pow(k as u32) as usize;
    let mut new_rules = vec![];
    let mut vert_starts ;
    let mut vert_ends ;
    for rule in rules_vec {
        //Horizontal (single-symbol) rules
        for i in 0..(rule.0.len() - k+1) {
            let mut start_sym_idx = 0;
            for (rule_sym_idx, rule_sym) in rule.0.iter().enumerate() {
                start_sym_idx += rule_sym*(b_symbol_set.length as SymbolIdx).pow((rule.0.len()-rule_sym_idx-1) as u32);
            }
            start_sym_idx *= (b_symbol_set.length as SymbolIdx).pow((i) as u32);

            let mut end_sym_idx = 0;
            for (rule_sym_idx, rule_sym) in rule.1.iter().enumerate() {
                end_sym_idx += rule_sym*(b_symbol_set.length as SymbolIdx).pow((rule.1.len()-rule_sym_idx-1) as u32);
            }
            end_sym_idx *= (b_symbol_set.length as SymbolIdx).pow(i as u32);
            new_rules.push((vec![start_sym_idx],vec![end_sym_idx]));
        }
        //Vertical (normal symbol length) rules
        //i is horizontal index selected
        //Represents the fixed column we're doing business with
        
        
        for i in 0..k {
            //LHS and RHS respectively
            vert_starts = vec![vec![0;rule.0.len()]];
            vert_ends = vec![vec![0;rule.1.len()]];
            //j is vertical index selected
            for j in 0..k {
                let cur_vert_rules = vert_starts.len();
                let pow_num = (b_symbol_set.length as SymbolIdx).pow(j as u32);
                
                //If we're looking at the fixed row
                if i == j {
                    for start_idx in 0..cur_vert_rules {
                        for vert_idx in 0..vert_starts[start_idx].len() {
                            vert_starts[start_idx][vert_idx] += rule.0[vert_idx]*pow_num;
                        }
                        for vert_idx in 0..vert_ends[start_idx].len() {
                            vert_ends[start_idx][vert_idx] += rule.1[vert_idx]*pow_num;
                        }
                    }
                } else {
                    for start_idx in 0..cur_vert_rules {
                        for k in 1..symbol_num {
                            let mut new_vert_start = vert_starts[start_idx].clone();
                            let mut new_vert_end = vert_ends[start_idx].clone();
                            for l in 0..k {
                                if (k >> l) % 2 == 1 {
                                    new_vert_start[l] += pow_num;
                                    new_vert_end[l] += pow_num;
                                }
                            }
                            
                            vert_starts.push(new_vert_start);
                            vert_ends.push(new_vert_end);
                        }
                    }
                }
                
            }
            for i in 0..vert_starts.len() {
                new_rules.push((vert_starts[i].clone(),vert_ends[i].clone()));
            }
        }
    }
    let by_k_symbol_set = SymbolSet {
        length : 2_u32.pow(k as u32) as usize,
        representations : vec!["000".to_owned(),"001".to_owned(),"010".to_owned(),"011".to_owned(),"100".to_owned(),"101".to_owned(),"110".to_owned(),"111".to_owned()] //whoops! lol
    };
    
    let ruleset = Ruleset::from_vec(new_rules,by_k_symbol_set.clone());
    
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1,1,1,1,1,1,1],vec![1,1,1,1,1,1,1,1]],
        accepting_states : HashSet::from_iter(vec![0]),
        symbol_set : by_k_symbol_set.clone()
    };
    
    S::new(ruleset,goal_dfa)
}

pub fn build_default2dpegx3<S>() -> S where S: Solver {
    let b_symbol_set = SymbolSet {
        length : 2,
        representations : vec!["0".to_owned(),"1".to_owned()]
    };

    let rule_vec = vec![(vec![1,1,0],vec![0,0,1]),
    (vec![0,1,1],vec![1,0,0])];

    let k = 3;
    let symbol_num = 2_u32.pow(k as u32) as usize;
    let mut new_rules = vec![];
    let mut vert_starts ;
    let mut vert_ends ;
    for rule in rule_vec {
        //Horizontal (single-symbol) rules
        for i in 0..(rule.0.len() - k+1) {
            let mut start_sym_idx = 0;
            for (rule_sym_idx, rule_sym) in rule.0.iter().enumerate() {
                start_sym_idx += rule_sym*(b_symbol_set.length as SymbolIdx).pow((rule.0.len()-rule_sym_idx-1) as u32);
            }
            start_sym_idx *= (b_symbol_set.length as SymbolIdx).pow((i) as u32);

            let mut end_sym_idx = 0;
            for (rule_sym_idx, rule_sym) in rule.1.iter().enumerate() {
                end_sym_idx += rule_sym*(b_symbol_set.length as SymbolIdx).pow((rule.1.len()-rule_sym_idx-1) as u32);
            }
            end_sym_idx *= (b_symbol_set.length as SymbolIdx).pow(i as u32);
            new_rules.push((vec![start_sym_idx],vec![end_sym_idx]));
        }
        //Vertical (normal symbol length) rules
        //i is horizontal index selected
        //Represents the fixed column we're doing business with
        
        
        for i in 0..k {
            //LHS and RHS respectively
            vert_starts = vec![vec![0;rule.0.len()]];
            vert_ends = vec![vec![0;rule.1.len()]];
            //j is vertical index selected
            for j in 0..k {
                let cur_vert_rules = vert_starts.len();
                let pow_num = (b_symbol_set.length as SymbolIdx).pow(j as u32);
                
                //If we're looking at the fixed row
                if i == j {
                    for start_idx in 0..cur_vert_rules {
                        for vert_idx in 0..vert_starts[start_idx].len() {
                            vert_starts[start_idx][vert_idx] += rule.0[vert_idx]*pow_num;
                        }
                        for vert_idx in 0..vert_ends[start_idx].len() {
                            vert_ends[start_idx][vert_idx] += rule.1[vert_idx]*pow_num;
                        }
                    }
                } else {
                    for start_idx in 0..cur_vert_rules {
                        for k in 1..symbol_num {
                            let mut new_vert_start = vert_starts[start_idx].clone();
                            let mut new_vert_end = vert_ends[start_idx].clone();
                            for l in 0..k {
                                if (k >> l) % 2 == 1 {
                                    new_vert_start[l] += pow_num;
                                    new_vert_end[l] += pow_num;
                                }
                            }
                            
                            vert_starts.push(new_vert_start);
                            vert_ends.push(new_vert_end);
                        }
                    }
                }
                
            }
            for i in 0..vert_starts.len() {
                new_rules.push((vert_starts[i].clone(),vert_ends[i].clone()));
            }
        }
    }
    let by_k_symbol_set = SymbolSet {
        length : 2_u32.pow(k as u32) as usize,
        representations : vec!["000".to_owned(),"001".to_owned(),"010".to_owned(),"011".to_owned(),"100".to_owned(),"101".to_owned(),"110".to_owned(),"111".to_owned()] //whoops! lol
    };
    
    let ruleset = Ruleset::from_vec(new_rules,by_k_symbol_set.clone());
    
    let root_dfa = DFA::load(&mut std::fs::File::open("default1dpeg").unwrap()).unwrap();

    let mut trans_table = vec![vec![1,2,2+16,2+16*2,2+16*2, 10,2,10],vec![1,3,3+16,3+16*2,3+16*2, 10,3,10]];
    for point in 0..=2 {
        let identical_indices = vec![vec![1,6],vec![2],vec![4,3]];
        
        
        for state in 2..root_dfa.state_transitions.len() {
            let mut new_vec = vec![10;8];
            for thing in &identical_indices[point] {
                new_vec[*thing] = root_dfa.state_transitions[state][1] + point * 16;
            }
            new_vec[0] = root_dfa.state_transitions[state][0] + point * 16;
            trans_table.push(new_vec);
        }
        
    }
    let mut new_accepting = root_dfa.accepting_states.clone();
    for i in root_dfa.accepting_states {
        new_accepting.insert(i + 16);
        new_accepting.insert(i + 32);
    }  

    
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : trans_table,
        accepting_states : new_accepting,
        symbol_set : by_k_symbol_set.clone()
    };
    
    
    S::new(ruleset,goal_dfa)
}