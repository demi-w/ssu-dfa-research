use std::collections::{HashSet, HashMap};

use bitvec::prelude::*;
use petgraph::{prelude::DiGraph, graph::NodeIndex, Direction, algo::toposort};

use crate::{util::{DFA, Ruleset, SymbolIdx}, solver::{DFAStructure, SSStructure}};

use super::{Solver, Instant, DomainError};



#[derive(Clone)]
pub struct SubsetSolver {
    pub goal : DFA,
    pub rules : Ruleset,
    pub max_input : usize,
    pub min_input : usize,
    trans_table : Vec<Vec<usize>>,
    sig_sets : Vec<BitVec>,
    solved_yet : Vec<BitVec>,
    unique_sigs : HashMap<BitVec,usize>
}

impl Solver for SubsetSolver {

    fn get_max_input(&self) -> usize {
        self.max_input
    }
    fn get_min_input(&self) -> usize {
        self.min_input
    }

    fn get_goal(&self) -> &DFA {
        &self.goal
    }

    fn new(mut ruleset : Ruleset, mut goal : DFA) -> Result<Self,DomainError> {
        if let Some((lhs,rhs)) = ruleset.has_non_length_preserving_rule() {
            if lhs.len() < rhs.len() {
                return Err(DomainError::Generating((lhs,rhs)));
            } else {
                return Err(DomainError::Deleting((lhs,rhs)));
            }
        }

        if let Some(problem) = ruleset.has_definitely_cyclic_rule() {
            return Err(DomainError::Cyclic(problem));
        }
        Self::ensure_expansion(&mut ruleset,&mut goal);


        let (min_input, max_input) = SubsetSolver::sized_init(&ruleset);
        Ok(SubsetSolver { goal: goal, rules: ruleset, sig_sets : vec![], solved_yet : vec![] , trans_table : vec![], min_input : min_input, max_input : max_input, unique_sigs : HashMap::new() })
    }
    fn get_phases() -> Vec<String> {
        vec!["Rule graph generation".to_owned(), "Sig set generation".to_owned(),"Uniqueness checks".to_owned(), "Clean up".to_owned()]
    }

    fn get_ruleset(&self) -> &Ruleset{
        &self.rules
    }

    fn run_internal(mut self,
        sig_k : usize, 
        is_debug : bool,
        dfa_events : std::sync::mpsc::Sender<(DFAStructure,SSStructure)>, 
        phase_events : std::sync::mpsc::Sender<std::time::Duration>) -> DFA {

        
    let init_begin_time = Instant::now();
    //graph of connections based on LHS->RHS links for all states
    //Usize is index in trans_table
    
    
    let sig_set = &self.rules.symbol_set.build_sig_k(sig_k);

    //not allowed to complain about my dumb code -- not everything will be optimal i have DEADLINES.
    //okay i'm the one making up the deadlines... but still
    let smaller_sig = self.rules.symbol_set.build_sig_k(sig_k-1);

    //list of strings for the newest known states

    let mut recent_strings = vec![vec![]];

    let mut new_recent_strings = vec![];

    self.solved_yet.push(bitvec![0;sig_set.len()]);

    self.sig_sets.push(bitvec![0;sig_set.len()]);
    self.sig_with_set_sub(&vec![], &sig_set, 0);
    self.trans_table.push((1..=self.rules.symbol_set.length).collect());
    self.unique_sigs.insert(self.sig_sets[0].clone(),0);

    self.solved_yet = vec![];

    //number of known states at last pass
    let mut last_known : usize = 1;
    //number of states with finished edges
    let mut last_finished : usize = 0;

    
     //while there are still states to process
    if is_debug {
        let _ = phase_events.send(Instant::now() - init_begin_time);
    }
    while last_finished < last_known{

        if is_debug {
            dfa_events.send((DFAStructure::Dense(self.trans_table.clone()),SSStructure::BooleanMap(self.unique_sigs.clone()))).unwrap();
        }

        let begin_time = Instant::now();


        //First step is populating self.sig_sets and self.solved_yet 
        
        //trans_table should already be correct? make sure to that when adding elements
        let new_states = (last_known - last_finished) * self.rules.symbol_set.length;
        self.sig_sets.resize(self.sig_sets.len()+new_states,bitvec![0;sig_set.len()]);
        self.solved_yet.resize(new_states,bitvec![0;sig_set.len()]);

        //next is adding all edges appropriately to the graph. 
        //this can be optimized substantially but i don't wanna do it pre-emptively :)
        let mut link_graph = DiGraph::<usize,()>::new();

        for index in 0..(last_known + new_states) {
            link_graph.add_node(index);
        }
        let mut old_len = link_graph.edge_count();
        for origin in 0..last_known {
            for rule_list in &self.rules.rules {
                let lhs = rule_list.0;
                for rhs in rule_list.1 {
                    let mut parent = origin;
                    let mut child = origin;
                    let mut valid = true;
                    for i in 0..rule_list.0.len() {
                        if parent >= last_known || child >= last_known {
                            valid = false;
                            break;
                        }
                        parent = self.trans_table[parent][lhs[i] as usize];
                        child = self.trans_table[child][rhs[i] as usize];
                    }
                    if valid {
                        link_graph.update_edge(NodeIndex::new(parent),NodeIndex::new(child),());
                    }
                }
            }  
        }
        // After establishing the starting points of all links, extend those links outward.
        
        while old_len < link_graph.edge_count() {
            let new_len = link_graph.edge_count();
            for edge_idx in old_len..new_len {
                for sym in 0..self.rules.symbol_set.length {
                    let old_parent = link_graph[link_graph.raw_edges()[edge_idx].source()];
                    let old_child = link_graph[link_graph.raw_edges()[edge_idx].target()];
                    if old_parent >= last_known || old_child >= last_known {
                        continue
                    }
                    let new_parent = self.trans_table[old_parent][sym as usize];
                    let new_child = self.trans_table[old_child][sym as usize];
                    link_graph.update_edge(NodeIndex::new(new_parent),NodeIndex::new(new_child),());
                }
            }
            old_len = new_len;
        }

        //Next we implant the sig set info from previous states' into the prospective states.

        
        for origin_idx  in last_finished..last_known {
            for (sym,move_idx) in self.trans_table[origin_idx].iter().enumerate() {
                for elem in &smaller_sig {
                    let mut elem_in_origin = vec![sym as u8];
                    elem_in_origin.extend(elem.iter());
                    let old_idx = self.rules.symbol_set.find_in_sig_set(elem_in_origin.iter());
                    let new_idx = self.rules.symbol_set.find_in_sig_set(elem.iter());
                    let scared_rust = self.sig_sets[origin_idx][old_idx];
                    self.sig_sets[*move_idx].set(new_idx,scared_rust);
                    self.solved_yet[move_idx - last_known].set(new_idx,true);
                }
            }
        }
        
        //cycle detection and removal. note that this changes the type of node_weight from usize to Vec<usize>. 
        //tests indicate that this vec is always sorted smallest to largest, but this fact may not hold true if code is modified.
        let link_graph = petgraph::algo::condensation(link_graph, true);

        if is_debug {
            let dur = begin_time.elapsed();
            phase_events.send(dur).unwrap();
        }
        let second_time = Instant::now();
        //Next is updating prospective states with all known information.
        //We're intentionally leaning more heavily on solving ANY POSSIBLE strings ahead of time,
        //this operation is constant* which is MUCH better than O(x^n), so idgaf
        //*the cache does inevitably get rawdogged
        

        //This doesn't change the number of sig elements that get skipped at all ???
        for origin_node in link_graph.node_indices() {
            //Parent is known & child is unknown
            //This updates the value of impossible entries as
            //1. known 2. to be impossible 3. for the child
            //Notably, this never modifies the child's sig set! that's bc it starts as false anyway
            //also commented out bc it should be redundant

            let origin = link_graph[origin_node][0];
            if origin >= last_known {
                continue
            }

            let mut visit = HashSet::new();
            visit.insert(origin_node);
            let mut explore = vec![origin_node];
            //This updates the value of possible entries as
            //1. known 2. to be possible 3. for the parent
            
            while let Some(nx) = explore.pop() {
                for neighbor in link_graph.neighbors_directed(nx,Direction::Incoming) {
                    if link_graph[neighbor][0] >= last_known && !visit.contains(&neighbor) {
                        visit.insert(neighbor);
                        explore.push(neighbor);

                        //Unsure why these need to be cloned! hopefully it is nothing horrible 😅
                        self.solved_yet[link_graph[neighbor][0] - last_known] |= self.sig_sets[origin].clone();
                        let why = self.sig_sets[origin].clone();
                        self.sig_sets[link_graph[neighbor][0]] |= why;
                    }
                }
            }
        }
        
        //Known-unknown pairs are finally fucking over. Now it's time for the scariest --
        //Unknown-unknown.
        let mut reverse_link_graph = link_graph.clone();
        reverse_link_graph.reverse();
        for node in toposort(&reverse_link_graph, None).unwrap() {
            if link_graph[node][0] >= last_known {
                //Get info about what's false from all incoming neighbors
                for neighbor in link_graph.neighbors_directed(node,Direction::Incoming) {
                    //if the neighbor is a known state
                    if link_graph[neighbor][0] < last_known {
                        //everything that the sig set says is false for neighbor, is false for node
                        self.solved_yet[link_graph[node][0] - last_known] |= !self.sig_sets[link_graph[neighbor][0]].clone();
                    
                    }
                    //if the neighbor's also a prospective state
                    else{
                        //everything that's been solved and is false for neighbor, is false for node
                        let scared_rust = self.solved_yet[link_graph[neighbor][0] - last_known].clone();
                        self.solved_yet[link_graph[node][0] - last_known] |= 
                        !self.sig_sets[link_graph[neighbor][0]].clone() & scared_rust;
                    }
                }
                //creating a string to actually test with
                let connecting_state = (link_graph[node][0] - last_known) / self.rules.symbol_set.length;
                let connecting_symbol = ((link_graph[node][0] - last_known) % self.rules.symbol_set.length) as SymbolIdx;
                let mut new_board = recent_strings[connecting_state].clone();
                new_board.push(connecting_symbol);
                self.sig_with_set_sub(&new_board, &sig_set, link_graph[node][0]);
            }
        }

        if is_debug {
            let dur = second_time.elapsed();
            phase_events.send(dur).unwrap();
        }
        let third_time = Instant::now();
         
        //Now, we look at all prospective states' signature sets and add the unique ones.
        let mut new_known = 0;
        let mut new_sig_sets = vec![];
        for pros_state in link_graph.node_indices() {
            //If there's an equivalent state that already exists in the DFA, use that!
            let connector = match link_graph[pros_state].iter().find(|&x| x < &last_known) {
                Some(idx) => {
                    *idx
                },
                None => {
                    match self.unique_sigs.get(&self.sig_sets[link_graph[pros_state][0]]) {
                        Some(i) => {*i}
                        None => {
                            let connecting_state = (link_graph[pros_state][0] - last_known) / self.rules.symbol_set.length + last_finished;
                            let connecting_symbol = ((link_graph[pros_state][0] - last_known) % self.rules.symbol_set.length) as SymbolIdx;
                            let mut new_board = recent_strings[connecting_state - last_finished].clone();
                            new_board.push(connecting_symbol);
                            self.unique_sigs.insert(self.sig_sets[link_graph[pros_state][0]].clone(),new_known+last_known);
                            new_known += 1;
                            new_sig_sets.push(self.sig_sets[link_graph[pros_state][0]].clone());
                            new_recent_strings.push(new_board);
                            new_known+last_known-1
                        }
                    }
                    
                }
            };
            for dupe in &link_graph[pros_state] {
                if *dupe < last_known {
                    continue
                }
                let connecting_state = (dupe - last_known) / self.rules.symbol_set.length + last_finished;
                let connecting_symbol = (dupe - last_known) % self.rules.symbol_set.length;
                self.trans_table[connecting_state][connecting_symbol] = connector;
            }
        }

        if is_debug {
            let dur = third_time.elapsed();
            phase_events.send(dur).unwrap();
        }
        let fourth_time = Instant::now();

        //Now we clean up -- no prospective states left over anywhere!

        self.sig_sets.truncate(last_known);
        self.sig_sets.append(&mut new_sig_sets);

        self.solved_yet.clear();

        for i in 0..new_known {
            self.trans_table.push(((last_known+new_known+i*self.rules.symbol_set.length)..=(last_known+new_known+(i+1)*self.rules.symbol_set.length-1)).collect())
        }
        last_finished = last_known;
        last_known = self.trans_table.len();

        std::mem::swap(&mut recent_strings, &mut new_recent_strings);
        new_recent_strings.clear();
        if is_debug {
            let dur: std::time::Duration = fourth_time.elapsed();
            phase_events.send(dur).unwrap();
        }
        }
        let mut accepting_states = Vec::new();
        for (key, val) in self.unique_sigs.iter() {
            accepting_states.push(key[0])
        }
        let trans_table = self.trans_table.clone();
        if is_debug {
            dfa_events.send((DFAStructure::Dense(self.trans_table.clone()),SSStructure::BooleanMap(self.unique_sigs.clone()))).unwrap();
        }
        //self.sig_sets = vec![]; BAD AND TEMPORARY
        DFA {
            state_transitions : trans_table,
            accepting_states : accepting_states,
            starting_state : 0,
            symbol_set : self.rules.symbol_set.clone()
        }
    }
}

impl SubsetSolver {
    fn bfs_solver_sub(&mut self, start_board : &Vec<SymbolIdx>, state_idx : usize, sig_idx : usize, investigated : &mut HashSet<(usize,usize)>) -> bool { 
        /*if !investigated.insert((state_idx,sig_idx)) {
            return false;
        }*/

        if state_idx < self.trans_table.len() || self.solved_yet[state_idx - self.trans_table.len()][sig_idx] {
            return self.sig_sets[state_idx][sig_idx];
        }
        if self.goal.contains(&start_board) {
            self.solved_yet[state_idx - self.trans_table.len()].set(sig_idx,true);
            self.sig_sets[state_idx].set(sig_idx,true);

            //RECURSIVELY INFORM PARENTS THIS SHIT IS TRUE
            //not yet tho : )
            return true;
        }
        //Do not need to update this node if true because recursive thing above should cover it.
        for new_board in self.single_rule_hash(&start_board) {
            let mut dfa_idx = 0;
            let mut board_idx = 0;
            //Find the location of the changed board in the DFA
            while board_idx < new_board.len() && dfa_idx < self.trans_table.len() {
                dfa_idx = self.trans_table[dfa_idx][new_board[board_idx] as usize];
                board_idx += 1;
            }
            if self.bfs_solver_sub(&new_board, dfa_idx, self.rules.symbol_set.find_in_sig_set(new_board[board_idx..].iter()),investigated) {
                self.sig_sets[state_idx].set(sig_idx,true);
                break
            }
        }
        self.solved_yet[state_idx - self.trans_table.len()].set(sig_idx,true);
        self.sig_sets[state_idx][sig_idx]
    }
    fn sig_with_set_sub(&mut self, board : &Vec<SymbolIdx>, sig_set : &Vec<Vec<SymbolIdx>>, state_idx : usize) {
        let solved_idx = state_idx - self.trans_table.len();
        let mut investigated = HashSet::new();
        for (idx,sig_element) in sig_set.iter().enumerate() {
            if !self.solved_yet[solved_idx][idx] {
                let mut new_board = board.clone();
                new_board.extend(sig_element);
                self.bfs_solver_sub(&new_board, state_idx, idx, &mut investigated);
                investigated.clear();
            }
        }
    }
}