
use std::{sync::mpsc::{Sender, Receiver, channel}, collections::{HashMap, HashSet, VecDeque}, io::{self, Write}, path};

use std::thread;

use crate::{util::{Ruleset, DFA, SymbolIdx, SymbolSet}, test};

pub use self::events::*;
mod events;

mod bfs;
pub use self::bfs::BFSSolver;

mod hash;
pub use self::hash::HashSolver;

mod subset;
pub use self::subset::SubsetSolver;

mod minkid;
pub use self::minkid::MinkidSolver;

use petgraph::{graph::{DiGraph,NodeIndex}, visit::EdgeRef};
#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;


#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

pub trait Solver where Self:Sized + Clone + Send + 'static{
    fn get_phases() -> Vec<String>;

    #[cfg(not(target_arch = "wasm32"))]
    fn run_debug(&self,
        sig_k : usize) -> (Receiver<(DFAStructure,SSStructure)>, Receiver<std::time::Duration>, thread::JoinHandle<DFA>) {
            let self_clone = self.clone();
            let (dfa_tx, dfa_rx) = channel();
            let (phase_tx, phase_rx) = channel();
            (dfa_rx, phase_rx, thread::spawn(move || {self_clone.run_internal(sig_k, true, dfa_tx, phase_tx)}))
            
        }
    
    //Changing the function signature based on the architecture is disgusting!
    //But ya know what -- so is the state of Rust WASM, so i'm making do.
    #[cfg(target_arch = "wasm32")]
    fn run_debug(&self,
        sig_k : usize) -> (Receiver<(DFAStructure,SSStructure)>, Receiver<std::time::Duration>) {
            let self_clone = self.clone();
            let (dfa_tx, dfa_rx) = channel();
            let (phase_tx, phase_rx) = channel();
            wasm_bindgen_futures::spawn_local(async move {self_clone.run_internal(sig_k, true, dfa_tx, phase_tx);});
            (dfa_rx, phase_rx)
            
        }
    fn run_internal(self,
                    sig_k : usize, 
                    is_debug : bool,
                    dfa_events : Sender<(DFAStructure,SSStructure)>, 
                    phase_events : Sender<std::time::Duration>) -> DFA;
    fn run(&self, sig_k : usize) -> DFA {
        let (dfa_tx, _dfa_rx) = channel();
        let (phase_tx, _phase_rx) = channel();
        self.clone().run_internal(sig_k, false, dfa_tx,phase_tx)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn run_with_print(&self, sig_k : usize) -> DFA {
        let (dfa_rx, phase_rx, run_handle) = self.run_debug(sig_k);
        let mut phase_idx;
        let mut phase_lens = vec![];
        let mut iterations = 0;
        let phases = Self::get_phases();
        let mut last_len = 0;
        if let Ok(time) = phase_rx.recv() {
            println!("Initialization time: {}ms",time.as_millis());
        }
        while let Ok((partial_dfa, _sig_sets)) = dfa_rx.recv() {
            let mut update_string = format!("Iteration {} | {} states solved, {} new", iterations,partial_dfa.len() ,partial_dfa.len() - last_len);
            last_len = partial_dfa.len();
            phase_idx = 0;
            print!("{}\r",update_string);
            io::stdout().flush().unwrap();
            while phase_idx < phases.len() {
                //Disconnection is guaranteed here -- should send final DFA then dc on both channels
                match phase_rx.recv() {
                    Ok(time) => {phase_lens.push(time);}
                    _ => {break}
                }
                
                update_string.push_str(&format!(" | {}: {}ms",phases[phase_idx],phase_lens.last().unwrap().as_millis()));
                print!("{}\r",update_string);
                io::stdout().flush().unwrap();
                phase_idx += 1;
            }
            iterations += 1;
            println!("{}",update_string);
        }
        run_handle.join().unwrap()
    }

    fn new(ruleset : Ruleset, goal : DFA) -> Result<Self,DomainError>;

    fn ensure_expansion(ruleset : &mut Ruleset, goal : &mut DFA){
        if ruleset.symbol_set == goal.symbol_set {return;} 
        if ruleset.symbol_set.is_subset(&goal.symbol_set) {
            ruleset.expand_to_symset(goal.symbol_set.clone());
        }else if goal.symbol_set.is_subset(&ruleset.symbol_set) {
            goal.expand_to_symset(ruleset.symbol_set.clone());
        } else {
            let mut merge_symbols = goal.symbol_set.representations.clone();
            merge_symbols.append(&mut ruleset.symbol_set.representations.clone());
            let merge_symset = SymbolSet::new(merge_symbols);
            ruleset.expand_to_symset(merge_symset.clone());
            goal.expand_to_symset(merge_symset);
        }
    }

    fn get_ruleset(&self) -> &Ruleset;

    fn get_goal(&self) -> &DFA;

    fn get_min_input(&self) -> usize;
    fn get_max_input(&self) -> usize;
    fn single_rule_hash(&self, start_board : &Vec<SymbolIdx>) -> Vec<Vec<SymbolIdx>> {
        let mut result = vec![];
        if start_board.is_empty() {
            if let Some(new_swaps) = self.get_ruleset().rules.get(&start_board[..]) {
                for new_swap in new_swaps {
                    result.push(new_swap.clone());
                }
            }
            return result;
        }
        
        
        for lftmst_idx in 0..start_board.len() {
            for slice_length in self.get_min_input()..core::cmp::min(self.get_max_input(),start_board.len()-lftmst_idx)+1 {
                if let Some(new_swaps) = self.get_ruleset().rules.get(&start_board[lftmst_idx..(lftmst_idx+slice_length)]) {
                    let new_board = start_board[0..lftmst_idx].to_vec();

                    for new_swap in new_swaps {
                        let mut newest_board = new_board.clone();
                        newest_board.extend(new_swap);
                        newest_board.extend(start_board[lftmst_idx+slice_length..start_board.len()].to_vec());
                        result.push(newest_board);
                    }
                }
            }
        }
        result
    }
    //returns an annotated list of all possible moves from a string
    //annotation is as follows: starting idx of rule application, len of lhs of rule used, len of rhs of rule used, resulting board.
    fn single_rule_hash_annotated(&self, start_board : &Vec<SymbolIdx>) -> Vec<(usize,usize,usize,Vec<SymbolIdx>)> {
        let mut result = vec![];
        if start_board.is_empty() {
            if let Some(new_swaps) = self.get_ruleset().rules.get(&start_board[..]) {
                for new_swap in new_swaps {
                    result.push((0,0,new_swap.len(),new_swap.clone()));
                }
            }
            return result;
        }
        for lftmst_idx in 0..start_board.len() {
            for slice_length in self.get_min_input()..core::cmp::min(self.get_max_input(),start_board.len()-lftmst_idx)+1 {
            if let Some(new_swaps) = self.get_ruleset().rules.get(&start_board[lftmst_idx..(lftmst_idx+slice_length)]) {
                    let new_board = start_board[0..lftmst_idx].to_vec();
                    for new_swap in new_swaps {
                        let mut newest_board = new_board.clone();
                        newest_board.extend(new_swap);
                        newest_board.extend(start_board[lftmst_idx+slice_length..start_board.len()].to_vec());
                        result.push((lftmst_idx,slice_length,new_swap.len(),newest_board));
                    }
                }
            }
        }
        result
    }

    fn sized_init(rules : &Ruleset) -> (usize, usize) {
        let mut min_input : usize = usize::MAX;
        let mut max_input : usize = 0;
        for i in &rules.rules {
            let input_len = i.0.len();
            if input_len < min_input {
                min_input = input_len;
            }
            if input_len > max_input {
                max_input = input_len;
            }
        }
        (min_input, max_input)
    }
    fn solve_string(&self, possible_dfa : &DFA, input_str : &Vec<SymbolIdx>) -> Result<Vec<Vec<SymbolIdx>>,()> {
        if !possible_dfa.contains(input_str) {
            return Err(())
        }
        let mut intrepid_str = input_str.clone();
        let mut visited = HashSet::new();
        let mut result = vec![intrepid_str.clone()];
        visited.insert(intrepid_str.clone());
        while !self.get_goal().contains(&intrepid_str) {
            for option in self.single_rule_hash(&intrepid_str) {
                if !visited.contains(&option) && possible_dfa.contains(&option) {
                    //println!("{}",symbols_to_string(&intrepid_str));
                    intrepid_str = option;
                    result.push(intrepid_str.clone());
                    visited.insert(intrepid_str.clone());
                }
            }
        }
        Ok(result)
    }

    fn solve_string_annotated(&self, possible_dfa : &DFA, input_str : &Vec<SymbolIdx>) -> Result<Vec<(usize,usize,usize,Vec<SymbolIdx>)>,()> {
        if !possible_dfa.contains(input_str) {
            return Err(())
        }
        if self.get_goal().contains(input_str) {
            return Ok(vec![])
        }
        let mut visited = HashMap::new();
        

        let mut old_options = vec![];
        let mut new_options = vec![input_str.clone()];
        visited.insert(input_str.clone(),(0,0,0,vec![]));
        loop {
            old_options.clear();
            std::mem::swap(&mut old_options, &mut new_options);
            for option_str in &old_options {
                for option in self.single_rule_hash_annotated(option_str) {
                    if self.get_goal().contains(&option.3) {
                        let mut result = vec![option];
                        let mut ancestor = option_str;
                        while ancestor != input_str {
                            let ancestor_info = visited.get(ancestor).unwrap();
                            result.push((ancestor_info.0,ancestor_info.1,ancestor_info.2, ancestor.clone()));
                            ancestor = &ancestor_info.3;
                        }
                        result.reverse();
                        return Ok(result);
                    } 
                    if !visited.contains_key(&option.3) && possible_dfa.contains(&option.3) {
                        //println!("{}",symbols_to_string(&intrepid_str));
                        new_options.push(option.3.clone());
                        visited.insert(option.3.clone(),(option.0,option.1,option.2,option_str.clone()));
                    }
                }
            }
        }
    }

    fn build_rule_graph(&self, possible_dfa : &DFA) -> DiGraph::<usize,RuleGraphRoot> {
        let mut rule_graph = DiGraph::<usize,RuleGraphRoot>::new();

        for index in 0..possible_dfa.state_transitions.len() {
            rule_graph.add_node(index);
        }
        for origin in 0..possible_dfa.state_transitions.len() {
            for rule_list in &self.get_ruleset().rules {
                let lhs = rule_list.0;
                let mut parent = origin;
                for i in 0..lhs.len() {
                    parent = possible_dfa.state_transitions[parent][lhs[i] as usize];
                }
                for rhs in rule_list.1 {
                    let mut child = origin;

                    for i in 0..rhs.len() {
                        child = possible_dfa.state_transitions[child][rhs[i] as usize];
                    }
                    rule_graph.update_edge(NodeIndex::new(parent),NodeIndex::new(child),RuleGraphRoot::new(lhs.clone(),rhs.clone(),parent,child,origin));
                }
            }  
        }
        // After establishing the starting points of all links, extend those links outward.
        let mut old_len = 0;
        while old_len < rule_graph.edge_count() {
            let new_len = rule_graph.edge_count();
            for edge_idx in old_len..new_len {
                let old_weight = rule_graph.raw_edges()[edge_idx].weight.clone();
                let old_parent = rule_graph[rule_graph.raw_edges()[edge_idx].source()];
                let old_child = rule_graph[rule_graph.raw_edges()[edge_idx].target()];
                for sym in 0..self.get_ruleset().symbol_set.length {
                    let new_parent = possible_dfa.state_transitions[old_parent][sym as usize];
                    let new_child = possible_dfa.state_transitions[old_child][sym as usize];
                    if !rule_graph.contains_edge(NodeIndex::new(new_parent), NodeIndex::new(new_child)) {
                        rule_graph.add_edge(NodeIndex::new(new_parent),NodeIndex::new(new_child),old_weight.clone());
                    }
                }
            }
            old_len = new_len;
        }
        rule_graph
    }
    fn is_superset(&self, test_dfa : &DFA) -> Result<(),Option<(RuleGraphRoot,usize,usize)>> {
        //println!("verbal reminder that this currently assumes that test_dfa >= self.get_goal()");
        if !(test_dfa >= self.get_goal()) {
            return Err(None)
        }
        let rule_graph = self.build_rule_graph(test_dfa);
        for edge in rule_graph.edge_references() {
            if !test_dfa.accepting_states.contains(&edge.source().index()) && test_dfa.accepting_states.contains(&edge.target().index()) {
                return Err(Some((edge.weight().clone(),edge.source().index(),edge.target().index())));
            }
        }
        Ok(())
    }
    fn build_no_rule_dfa(&self) -> DFA {
        if self.get_ruleset().rules.contains_key(&vec![]) {
            DFA {
                accepting_states : HashSet::new(),
                starting_state : 0,
                state_transitions : vec![vec![0;self.get_goal().symbol_set.length]],
                symbol_set : self.get_goal().symbol_set.clone(),
            }
        } else {
            let mut state_buffers : Vec<Vec<SymbolIdx>> = vec![vec![SymbolIdx::MAX],vec![]];
            
            let mut state_transitions = vec![vec![0;self.get_goal().symbol_set.length];2];
            let mut last_states = 1;
            while last_states != state_transitions.len() {
                let old_states = last_states;
                last_states = state_transitions.len();
                for new_state in old_states..state_transitions.len() {
                    
                    for new_buffer_sym in 0..self.get_goal().symbol_set.length {
                        let mut new_buffer = state_buffers[new_state].clone();
                        let mut perfect_match_found = false;
                        let mut match_found = false;
                        new_buffer.push(new_buffer_sym as SymbolIdx);
                        
                        //If any group of our last characters is the lhs side of a rule, then obviously a rule can be performed
                        //I.e. if buffer is 1,1,0,0,2
                        //we check 1,1,0,0,2 & 1,0,0,2 & 0,0,2 & 0,2 & 2
                        for i in 0..new_buffer.len() {

                            if self.get_ruleset().rules.contains_key(&new_buffer[i..]) {
                                state_transitions[new_state][new_buffer_sym] = 0;
                                perfect_match_found = true;
                                break;
                            }
                        } 
                        if perfect_match_found {
                            continue;
                        }
                        //If there's not, we strip the buffer of any characters that we know will not be used as lhs
                        //I.e. if buffer is 2,0,2 we know that first 2 is never used, so buffer should become 0,2
                        while !match_found {
                            for (lhs, _) in &self.get_ruleset().rules {
                                if lhs.len() > new_buffer.len() && lhs[..new_buffer.len()] == new_buffer {
                                    match_found = true;
                                    break;
                                }
                            }
                            if !match_found {
                                new_buffer.remove(0);
                            }
                        } 
                        match_found = false;
                        for (idx,buffer) in state_buffers.iter().enumerate() {
                            if &new_buffer == buffer {
                                match_found = true;
                                state_transitions[new_state][new_buffer_sym] = idx;
                                break;
                            }
                        }
                        if !match_found {
                            state_buffers.push(new_buffer);
                            state_transitions[new_state][new_buffer_sym] = state_transitions.len();
                            state_transitions.push(vec![0;self.get_goal().symbol_set.length])
                        }
                    }
                }
            }


            DFA {
                accepting_states : HashSet::from_iter(1..state_transitions.len()),
                starting_state : 1,
                state_transitions : state_transitions,
                symbol_set : self.get_goal().symbol_set.clone(),
            }

        }

    }
    fn build_path_graph(&self, possible_dfa : &DFA) -> Vec<Vec<Path>> {
        let mut new_paths = vec![(possible_dfa.starting_state, 0)];
        let mut old_paths = vec![];
        let mut paths = vec![vec![];possible_dfa.state_transitions.len()];
        paths[possible_dfa.starting_state].push(Path {buffer : vec![], rhs_connections : vec![],buffer_origin : possible_dfa.starting_state, goal_state : self.get_goal().starting_state});
        while !new_paths.is_empty() {
            std::mem::swap(&mut old_paths, &mut new_paths);
            new_paths.clear();
            for old_path in &old_paths {
                for symbol in 0..possible_dfa.symbol_set.length {
                    let mut new_buffer = paths[old_path.0][old_path.1].buffer.clone();
                    new_buffer.push(symbol as SymbolIdx);

                    let  new_goal_state = self.get_goal().state_transitions[paths[old_path.0][old_path.1].goal_state][symbol];

                    let mut new_path = Path {buffer : new_buffer, rhs_connections : vec![], buffer_origin : paths[old_path.0][old_path.1].buffer_origin, goal_state : new_goal_state};
                    
                    //Follow all old pure links
                    for old_rhs_connection in &paths[old_path.0][old_path.1].rhs_connections {
                        let new_connection = possible_dfa.state_transitions[*old_rhs_connection][symbol];
                        if !new_path.rhs_connections.contains(&new_connection) {
                            new_path.rhs_connections.push(new_connection)
                        }
                    }

                   
                    //If any group of our buffer is the lhs side of a rule, then obviously a rule can be performed
                    //I.e. if buffer is 1,1,0,0,2
                    //we check 1,1,0,0,2 & 1,0,0,2 & 0,0,2 & 0,2 & 2
                    for i in 0..new_path.buffer.len() {

                        //If a buffer matches a left-hand side, determine all rhs states could be reached by the origin + buffer after a single rule application
                        match self.get_ruleset().rules.get(&new_path.buffer[i..]) {
                            Some(rhs_list) => {
                                //Find where the origin where the lhs substring would begin
                                let mut relevant_origin = paths[old_path.0][old_path.1].buffer_origin;
                                for j in 0..i {
                                    relevant_origin = possible_dfa.state_transitions[relevant_origin][new_path.buffer[j] as usize];
                                }
                                for rhs in rhs_list {
                                    let mut rhs_end_idx = relevant_origin;
                                    for rhs_element in rhs {
                                        rhs_end_idx = possible_dfa.state_transitions[rhs_end_idx][*rhs_element as usize];
                                    }
                                    if !new_path.rhs_connections.contains(&rhs_end_idx) {
                                        new_path.rhs_connections.push(rhs_end_idx);
                                    }
                                }
                            }
                            None => {}
                        }
                    } 
                    //We strip the buffer of any characters that we know will not be used as lhs
                    //I.e. if buffer is 2,0,2 we know that first 2 is never used, so buffer should become 0,2
                    let mut match_found = false;
                    while !match_found {
                        for (lhs, _) in &self.get_ruleset().rules {
                            //is the whole buffer relevant as the of the lhs of a rule?
                            if lhs.len() > new_path.buffer.len() && lhs[..new_path.buffer.len()] == new_path.buffer {
                                match_found = true;
                                break;
                            }
                        }
                        if !match_found {
                            //Move 
                            let unnecesary_char = new_path.buffer.remove(0);
                            new_path.buffer_origin = possible_dfa.state_transitions[new_path.buffer_origin][unnecesary_char as usize];
                        }
                    }
                    //Find where this path would be added
                    let dest_idx = possible_dfa.state_transitions[old_path.0][symbol];
                    if !paths[dest_idx].contains(&new_path) {
                        new_paths.push((dest_idx,paths[dest_idx].len()));
                        paths[dest_idx].push(new_path);
                    }
                }
            }
        }
        paths
    }
    fn is_correct(&self, possible_dfa : &DFA) -> bool {
        //If there are strings that don't match the goal dfa with no rules
        //That aren't rejected by the possible dfa
        if self.is_superset(&possible_dfa).is_err() {
            return false
        }
        if !(&self.build_no_rule_dfa() - self.get_goal() < !possible_dfa) {
            //Throw the whole thing out!
            return false
        }
        let path_graph = self.build_path_graph(possible_dfa);
        for (state_idx,state_paths) in path_graph.iter().enumerate() {
            if possible_dfa.accepting_states.contains(&state_idx) {
                for path in state_paths {
                    if !self.get_goal().accepting_states.contains(&path.goal_state) && path.rhs_connections.iter().all(|f| !possible_dfa.accepting_states.contains(f)) {
                        return false
                    }
                }
            }
        }
        true
    }
}

#[derive(Debug,Clone)]
pub struct RuleGraphRoot {
    lhs :Vec<SymbolIdx>,
    rhs : Vec<SymbolIdx>,
    init_lhs_state : usize,
    init_rhs_state : usize,
    origin : usize
}

impl RuleGraphRoot {
    fn new(lhs :Vec<SymbolIdx>,rhs : Vec<SymbolIdx>,init_lhs_state : usize, init_rhs_state : usize, origin : usize) -> Self {
        RuleGraphRoot { lhs: lhs, rhs: rhs, init_lhs_state: init_lhs_state, init_rhs_state: init_rhs_state, origin : origin }
    }
    pub fn to_string(&self, symset : &SymbolSet) -> String {
        let mut result = "".to_owned();
        result.push_str("LHS ");
        result.push_str(&symset.symbols_to_string(&self.lhs));
        result.push_str(" | RHS ");
        result.push_str(&symset.symbols_to_string(&self.rhs));
        result.push_str(" | initial LHS state ");
        result.push_str(&self.init_lhs_state.to_string());
        result.push_str(" | initial RHS state ");
        result.push_str(&self.init_rhs_state.to_string());
        result.push_str(" | origin ");
        result.push_str(&self.origin.to_string());
        result
    }
}

/*
method to solve a string
todo: implement generically



*/

/*
old testing methods. they're takin a nap here while I decide what to do with em

fn verify_to_len(&mut self,test_dfa : DFA, n:usize) -> bool{
    //almost certainly a constant time answer to this but idk and idc
    let mut total_boards = 0;
    for i in 0..(n+1) {
        total_boards += (self.symbol_set.length as u64).pow(i as u32);
    }
    
    println!("Starting DFA verification for strings <= {}. {} total boards",n, total_boards);
    let mut num_completed = 0;
    let mut num_accepting = 0;
    let mut start_index = 0;

    let (input, output) = self.create_workers(WORKERS);

    let mut signature_set_old : Vec<Vec<SymbolIdx>> = vec![];
    let mut signature_set_new : Vec<Vec<SymbolIdx>> = vec![vec![]];
    for _ in 0..n {
        std::mem::swap(&mut signature_set_old, &mut signature_set_new);
        signature_set_new.clear();
        for (idx,i) in signature_set_old.iter().enumerate() {
            for symbol in 0..(self.symbol_set.length as SymbolIdx) {
                signature_set_new.push(i.clone());
                signature_set_new.last_mut().unwrap().push(symbol);
                let test_board = signature_set_new.last().unwrap();
                input.push((test_board.clone(),(idx*self.symbol_set.length + (symbol as usize))));
            }
        }
        let mut num_recieved = 0;
        while num_recieved < signature_set_new.len() {
            match output.pop() {
            Some((bfs_result,idx)) => {
                let test_board = &signature_set_new[idx];
                num_completed += 1;
                num_recieved += 1;
                if (num_completed) % (total_boards / 10) == 0 {
                    println!("{}% complete! ({} boards completed)", 100 * num_completed / total_boards, num_completed);
                }
                if bfs_result {num_accepting += 1}
                if test_dfa.contains(&test_board) != bfs_result {
                    println!("Damn. DFA-solvability failed.");
                    println!("Problem board: {}",symbols_to_string(&test_board));
                    println!("DFA: {}, BFS: {}",!bfs_result,bfs_result);
                    return false;
                }
            }
            None => {std::thread::sleep(time::Duration::from_millis(100));}
            }
        }
    }
    self.terminate_workers(input, WORKERS);

        
    println!("All verified! {}% accepting",(num_accepting as f64) * 100.0 / (total_boards as f64));

    true

}
fn random_tests(&mut self,test_dfa : DFA, n:usize, total_boards:usize){
    //almost certainly a constant time answer to this but idk and idc
    
    println!("Starting DFA verification for {} strings of length {}.",total_boards, n);
    let mut num_completed = 0;
    let mut num_accepting = 0;
    let mut start_index = 0;

    let (input, output) = self.create_workers(WORKERS);

    let mut test_items : Vec<Vec<SymbolIdx>> = vec![];
    let mut rng = rand::thread_rng();
    for i in 0..total_boards {
        let mut new_board = vec![];
        for _ in 0..n {
            new_board.push(rng.gen_range(0..self.symbol_set.length) as SymbolIdx);
        }
        input.push((new_board.clone(),i));
        test_items.push(new_board);
    }

    let mut num_recieved = 0;
    while num_recieved < total_boards {
        match output.pop() {
        Some((bfs_result,idx)) => {
            let test_board = &test_items[idx];
            num_completed += 1;
            num_recieved += 1;
            if (num_completed) % (total_boards / 10) == 0 {
                println!("{}% complete! ({} boards completed)", 100 * num_completed / total_boards, num_completed);
            }
            if bfs_result {num_accepting += 1}
            if test_dfa.contains(&test_board) != bfs_result {
                println!("Damn. DFA-solvability failed.");
                println!("Problem board: {}",symbols_to_string(&test_board));
                println!("DFA: {}, BFS: {}",!bfs_result,bfs_result);
                return;
            }
        }
        None => {std::thread::sleep(time::Duration::from_millis(100));}
        }
    }
    self.terminate_workers(input, WORKERS);

        
    println!("All verified! {}% accepting",(num_accepting as f64) * 100.0 / (total_boards as f64));

}
 */

 #[derive(Debug)]
pub enum DomainError {
    Generating((Vec<SymbolIdx>,Vec<SymbolIdx>)),
    Deleting((Vec<SymbolIdx>,Vec<SymbolIdx>)),
    Cyclic((Vec<SymbolIdx>,Vec<SymbolIdx>))
}

impl DomainError {
    pub fn to_string(&self, symset : &SymbolSet) -> String {
        let mut result = "Solver is incompatible with ".to_owned();
        match &self {
            DomainError::Generating((lhs,rhs)) => result.push_str(&format!("generating rules. SRS contains generating rule \"{} - {}\"",symset.symbols_to_string(&lhs),symset.symbols_to_string(&rhs))),
            DomainError::Deleting((lhs,rhs)) => result.push_str(&format!("deleting rules. SRS contains deleting rule \"{} - {}\".",symset.symbols_to_string(&lhs),symset.symbols_to_string(&rhs))),
            DomainError::Cyclic((lhs,rhs)) => result.push_str(&format!("cyclic rules. Rules \"{0} - {1}\" and \"{1} - {0}\" create a cycle.",symset.symbols_to_string(&lhs),symset.symbols_to_string(&rhs)))
        }
        result
    }
}

#[derive(PartialEq,Eq,Clone)]
pub struct Path {
    buffer : Vec<SymbolIdx>,
    rhs_connections : Vec<usize>,
    buffer_origin : usize,
    goal_state : usize
}