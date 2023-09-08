#![allow(warnings)] 
use std::hash::Hash;
use std::fmt::{self, write, format};
use std::ops::Index;
use automata::dfa::{Node, self};
use bitvec::prelude::*;
use petgraph::algo::toposort;
use petgraph::{prelude::*, visit};
use crossbeam::queue::{SegQueue, ArrayQueue};
use petgraph::graph::{NodeIndex, UnGraph, DiGraph};
use petgraph::dot::{Dot, Config};
use petgraph::visit::Reversed;
use strum_macros::EnumIter;
//use bit_set::Vec<bool>;
use std::mem;
use crossbeam;
use std::collections::{HashMap,HashSet};
use petgraph::Graph;
use std::fs;
use std::time;
use std::sync::Arc;
use std::fmt::Display;
extern crate xml;
use std::fs::File;
use std::io::{self, Write};
use std::fmt::Debug;
use rand::prelude::*;

use petgraph::algo::{tarjan_scc,condensation};

use serde_json::Result;

use xml::writer::{EventWriter, EmitterConfig, XmlEvent, Result as XmlResult};
use std::marker::PhantomData;
use std::time::Instant;


use serde::{Deserialize, Serialize};
use std::thread;
#[macro_use]
extern crate lazy_static;

const WORKERS : usize = 37;

const SIGNATURE_K : usize = 6;

const SIGNATURE_LENGTH : usize = (2 << SIGNATURE_K)-1; // (2 ^ (SIGNATURE_K+1)) - 1. 
// e.g. K = 5; (2^6) - 1; the +1 comes from the way bitshifting works
//Total number of items in the signature, currently hardcoded to all binary options of len 0 <= x <= SIGNATURE_K

type SymbolIdx = u8;

lazy_static! {
    
static ref SIGNATURE_ELEMENTS : [Vec<bool>;SIGNATURE_LENGTH] = {
    let mut start_index = 0;
    const EMPTY_VEC : Vec<bool> = vec![];
    let mut result : [Vec<bool>;SIGNATURE_LENGTH] = [EMPTY_VEC;SIGNATURE_LENGTH];
    let mut end_index = 1;
    let mut new_index = 1;
    for _ in 0..SIGNATURE_K {
        for i in start_index..end_index{
            result[new_index] = result[i].clone();
            result[new_index].push(false);
            new_index += 1;
            result[new_index] = result[i].clone();
            result[new_index].push(true);
            new_index += 1;
        }
        start_index = end_index;
        end_index = new_index;
    }
    result
};
}


#[derive(Clone, Debug)]
struct SignatureElement {
    board : Vec<bool>,
    signature : Vec<bool>
}
#[derive(Clone,Serialize,Deserialize,Debug)]
struct SymbolSet {
    length : usize,
    representations : Vec<String>
}
impl SymbolSet {
    fn new(representations : Vec<String>) -> SymbolSet{
        SymbolSet { length: representations.len(), representations: representations }
    }

    fn find_in_sig_set<'a>(&self, string : impl Iterator<Item = &'a SymbolIdx>) -> usize
    {
        let mut result = 0;
        for sym in string {
            result *= self.length;
            result += *sym as usize + 1;
        }
        result
    }
    fn idx_to_element(&self, mut idx : usize) -> Vec<SymbolIdx>
    {
        let mut result = vec![];
        while idx > 0 {
            idx -= 1;
            result.push((idx % self.length) as SymbolIdx);
            idx /= self.length;
        }
        result.reverse();
        result
    }
}

#[derive(Clone)]
struct Ruleset  {
    min_input : usize,
    max_input : usize,
    rules : Vec<(Vec<SymbolIdx>,Vec<SymbolIdx>)>,
    symbol_set : SymbolSet,
    map : HashMap<Vec<SymbolIdx>, Vec<Vec<SymbolIdx>>>, //need this for speed : )
    reverse_map : HashMap<Vec<SymbolIdx>, Vec<Vec<SymbolIdx>>>
}

impl Ruleset {
    fn new(rules : Vec<(Vec<SymbolIdx>,Vec<SymbolIdx>)>, symbol_set : SymbolSet) -> Ruleset{
        let mut min_input : usize = usize::MAX;
        let mut max_input : usize = 0;
        let mut rule_hash : HashMap<Vec<SymbolIdx>,Vec<Vec<SymbolIdx>>> = HashMap::new();
        let mut reverse_rule_hash : HashMap<Vec<SymbolIdx>,Vec<Vec<SymbolIdx>>> = HashMap::new();
        //Should use a fancy map function here I admit
        for i in &rules {
            let input_len = i.0.len();
            if input_len < min_input {
                min_input = input_len;
            }
            if input_len > max_input {
                max_input = input_len;
            }
            match rule_hash.get_mut(&i.0) {
                Some(result_vec) => {result_vec.push(i.1.clone())},
                None => {rule_hash.insert(i.0.clone(), vec![i.1.clone()]);}
            }
            match reverse_rule_hash.get_mut(&i.1) {
                Some(result_vec) => {result_vec.push(i.0.clone())},
                None => {reverse_rule_hash.insert(i.1.clone(), vec![i.0.clone()]);}
            }
        }
        
        Ruleset { min_input: min_input, 
                max_input: max_input, 
                rules: rules, 
                symbol_set: symbol_set, 
                map: rule_hash,
                reverse_map : reverse_rule_hash }
    }
    
    fn rule_applications(&self, start_board : &Vec<SymbolIdx>) -> Vec<(usize, usize)>{
        let mut index = 0;
        let mut result = vec![];
        while index < start_board.len(){
            for (rule_idx,rule) in self.rules.iter().enumerate() {
                let end_index = index+rule.0.len();
                if end_index <= start_board.len() && rule.0[..] == start_board[index..end_index] {
                    result.push((rule_idx,index))                    
                }
            }
            index += 1;
        }
        result
    }
    fn apply_rule(&self, start_board : &Vec<SymbolIdx>, rule_idx : usize, rule_pos : usize) -> Vec<SymbolIdx> {
        let rule = &self.rules[rule_idx];
        let mut new_board = start_board[0..rule_pos].to_vec();
        new_board.extend(rule.1.clone());
        new_board.extend(start_board[rule_pos+rule.0.len()..start_board.len()].to_vec());
        /* 
        for new_sym in &rule.1 {
            
            new_board[copy_index] = *new_sym;
            copy_index += 1;
        }*/
        new_board
    }
    fn single_rule(&self, start_board : &Vec<SymbolIdx>) -> Vec<Vec<SymbolIdx>> {
        let mut result = vec![];
        for new_option in self.rule_applications(start_board) {
            result.push(self.apply_rule(&start_board,new_option.0,new_option.1));
        }
        result
    }
    //Do we need to do this? honestly i don't think so.
    //Feels like we'd benefit from much faster (manual) 2d operations
    //i.e. keeping the same amounts of rules as 1d but crawling through rows + columns (basically no diagonal moves allowed lol).
    //or maybe there's a way to keep diagonal moves too idk
    fn single_rule_hash(&self, start_board : &Vec<SymbolIdx>) -> Vec<Vec<SymbolIdx>> {
        let mut result = vec![];
        for lftmst_idx in 0..start_board.len() {
            for slice_length in (self.min_input..core::cmp::min(self.max_input,start_board.len()-lftmst_idx)+1) {
                match self.map.get(&start_board[lftmst_idx..(lftmst_idx+slice_length)]) {
                    Some(new_swaps) => {
                        let new_board = start_board[0..lftmst_idx].to_vec();

                        for new_swap in new_swaps {
                            let mut newest_board = new_board.clone();
                            newest_board.extend(new_swap);
                            newest_board.extend(start_board[lftmst_idx+slice_length..start_board.len()].to_vec());
                            result.push(newest_board);
                        }
                    }
                    None => {}
                }
            }
        }
        result
        
    }
    fn reverse_single_rule_hash(&self, start_board : &Vec<SymbolIdx>) -> Vec<Vec<SymbolIdx>> {
        let mut result = vec![];
        for lftmst_idx in 0..start_board.len() {
            for slice_length in (self.min_input..core::cmp::min(self.max_input,start_board.len()-lftmst_idx)+1) {
                match self.reverse_map.get(&start_board[lftmst_idx..(lftmst_idx+slice_length)]) {
                    Some(new_swaps) => {
                        let new_board = start_board[0..lftmst_idx].to_vec();

                        for new_swap in new_swaps {
                            let mut newest_board = new_board.clone();
                            newest_board.extend(new_swap);
                            newest_board.extend(start_board[lftmst_idx+slice_length..start_board.len()].to_vec());
                            result.push(newest_board);
                        }
                    }
                    None => {}
                }
            }
        }
        result
    }

    fn reverse_single_rule_hash_fucko(&self, start_board : &Vec<SymbolIdx>, immutably_threshold : usize) -> Vec<Vec<SymbolIdx>> {
        let mut result = vec![];
        for lftmst_idx in immutably_threshold..start_board.len() {
            for slice_length in (self.min_input..core::cmp::min(self.max_input,start_board.len()-lftmst_idx)+1) {
                match self.reverse_map.get(&start_board[lftmst_idx..(lftmst_idx+slice_length)]) {
                    Some(new_swaps) => {
                        let new_board = start_board[0..lftmst_idx].to_vec();

                        for new_swap in new_swaps {
                            let mut newest_board = new_board.clone();
                            newest_board.extend(new_swap);
                            newest_board.extend(start_board[lftmst_idx+slice_length..start_board.len()].to_vec());
                            result.push(newest_board);
                        }
                    }
                    None => {}
                }
            }
        }
        result
    }

    fn all_reverse_from(&self,start_boards : &Vec<Vec<SymbolIdx>>, result_map : &mut HashSet<Vec<SymbolIdx>> ) {
        let mut old_boards = vec![];
        let mut new_boards = start_boards.clone();
        while new_boards.len() > 0 {
            std::mem::swap(&mut old_boards, &mut new_boards);
            new_boards.clear();
            for old_board in &old_boards {
                for potential_board in self.reverse_single_rule_hash(old_board) {
                    if result_map.insert(potential_board.clone()) {
                        new_boards.push(potential_board);
                    }
                }
            }
        }
        
    }
    fn all_reverse_from_fucko(&self,start_boards : &Vec<Vec<SymbolIdx>>, immutably_threshold : usize) -> HashSet<Vec<SymbolIdx>> {
        let mut result_map : HashSet<Vec<SymbolIdx>> = HashSet::new();
        let mut old_boards = vec![];
        let mut new_boards = start_boards.clone();
        while new_boards.len() > 0 {
            std::mem::swap(&mut old_boards, &mut new_boards);
            new_boards.clear();
            for old_board in &old_boards {
                for potential_board in self.reverse_single_rule_hash(old_board) {
                    if !result_map.contains(&potential_board) {
                        new_boards.push(potential_board.clone());
                        result_map.insert(potential_board);
                    }
                }
            }
        }
        result_map
    }
}

fn worker_thread(translator : Arc<SRSTranslator>, input : Arc<SegQueue<(Vec<SymbolIdx>,usize)>>, output : Arc<SegQueue<(bool,usize)>>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while true {
            match input.pop() {
                Some(input_string) => 
                {
                    if input_string.1 == usize::MAX && input_string.0 == vec![69,42] {
                        return;
                    }
                    let result = translator.bfs_solver_batch(&input_string.0);
                    output.push((result,input_string.1));
                }
                None => {}//{std::thread::sleep(time::Duration::from_millis(10));}
            }
        }
    })
}

#[derive(Clone,Serialize,Deserialize)]
struct DFA {
    starting_state : usize,
    state_transitions : Vec<Vec<usize>>,
    accepting_states : HashSet::<usize>,
    symbol_set : SymbolSet
}

#[derive(Clone)]
struct SRSTranslator {
    rules : Ruleset,
    goal : DFA,
    board_solutions : HashMap<Vec<SymbolIdx>,bool>,
    symbol_set : SymbolSet,
    //signature sets of all known, then prospective states
    sig_sets : Vec<BitVec>,

    //which elements of the signature set are solved; for each prospective state
    solved_yet : Vec<BitVec>,

    //HashMap of known states' signature sets, used for uniqueness test
    unique_sigs : HashMap<BitVec,usize>,

    //2-D transition table of all known, the prospective states. 
    trans_table : Vec<Vec<usize>>,

    //Link graph of signature set elements
    ss_link_graph : DiGraph<SignatureSetElement,()>,

    //For each state of the goal DFA, what would its hypothetical minkid set look like?
    //Used as the basis for propagation in the minkid method
    goal_minkids : Vec<HashSet<NodeIndex>>
}

#[derive(Debug,Clone,Hash)]
struct DFAState {
    solved_children : usize,
    sig_set : BitVec,
    trans_states : Vec<usize>
}

#[derive(Debug,Clone,Hash)]
struct ProspectiveDFAState {
    solved_parents : usize,
    sig_set : BitVec,
    known_answer : BitVec,
    origin : usize,
    origin_char : SymbolIdx
}

#[derive(Debug,Clone, Default)]
struct SignatureSetElement {
    //Original elements of the signature set that this one node now represents
    original_idxs : Vec<usize>,
    //Pre-computed set of ancestors -- used under the assumption that pre-calculating this will ultimately make things way faster
    //assumption is wrong -- memory complexity is ridiculous lol
    //precomputed_ancestors : HashSet<NodeIndex>,
    //DFA states that lead to an accepting string after walking through !!any!! of the original elements for this node
    //Deprecated in favor of goal_minkids in SRS translator
    //accepting_states : Vec<usize>
}


impl SRSTranslator {

    fn new(rules : Ruleset, goal : DFA) -> SRSTranslator {
        let sym_set = goal.symbol_set.clone();
        SRSTranslator { rules: rules, 
            goal: goal, 
            board_solutions: HashMap::new(),
            symbol_set: sym_set,
            sig_sets : vec![],
            solved_yet : vec![],
            unique_sigs : HashMap::new(),
            trans_table : vec![],
            //link_graph for all members of signature set
            ss_link_graph : DiGraph::<SignatureSetElement,()>::new(),
            goal_minkids : vec![]
         }
    }
    fn build_ss_link_graph(&mut self, k : usize, sig_set : &Vec<Vec<SymbolIdx>>){
        let mut ss_link_graph = DiGraph::<usize,()>::with_capacity(sig_set.len(),10);
        //irritated that there is not an immediately obvious better way but w/e
        
        //build initial link graph
        for i in 0..sig_set.len() {
            ss_link_graph.add_node(i);
        }
        for i in 0..sig_set.len() {
            for result in self.rules.single_rule_hash(&sig_set[i]) {
                ss_link_graph.add_edge(NodeIndex::new(i), NodeIndex::new(self.symbol_set.find_in_sig_set(result.iter())), ());
            }
        }
        //Get rid of strongly-connected components
        let ss_link_graph = condensation(ss_link_graph, true);

        //Convert into actually-used data structure
        self.ss_link_graph = DiGraph::new();
        for i in ss_link_graph.node_indices() {
            let mut idxs_clone = ss_link_graph[i].clone();
            idxs_clone.shrink_to_fit();
            self.ss_link_graph.add_node(SignatureSetElement { original_idxs: idxs_clone });
        }
        //I would love to care about this. will not yet!
        //self.ss_link_graph.extend_with_edges(self.ss_link_graph.raw_edges().iter());
        for i in ss_link_graph.raw_edges(){
            self.ss_link_graph.add_edge(i.source(), i.target(), ());
        }

        //time to pre-compute ancestors & calculate valid DFA states
        let mut reversed_graph = self.ss_link_graph.clone();
        reversed_graph.reverse();
        
        //Building minkids for each state in the goal DFA
        //Done by performing DFS
        self.goal_minkids = Vec::with_capacity(self.goal.state_transitions.len());
        //There is a fancier DFS-based way to do this. Do I look like the type to care?
        //(jk again just not pre-emptively optimizing)
        for goal_state in 0..self.goal_minkids.len() {
            //Toposort used so no childer checks needed
            for element in toposort(&reversed_graph, None).unwrap() {
                //Are any of the strings represented by this node accepting?
                let is_accepted = self.ss_link_graph[element].original_idxs.iter().any(|x| self.goal.contains_from_start(&sig_set[*x], goal_state));
                //If it's an accepting state that is not the ancestor of any of the current minkids
                if is_accepted && !self.check_if_ancestor(&self.goal_minkids[goal_state], element) {
                    self.goal_minkids[goal_state].insert(element);
                }
            }
        }
        
        /* 
        for i in self.ss_link_graph.node_indices() {
            //Calculating all ancestors
            //Notably, this includes itself. Burns some memory, but allows us to skip what would otherwise be an additional check
           // let mut dfs = Dfs::new(&reversed_graph,i);
            //while let Some(nx) = dfs.next(&reversed_graph) {
            //    self.ss_link_graph[i].precomputed_ancestors.insert(nx);
            //}
            //Calculating valid DFA states
            //old method for building accpeting states for each string -- disliked bc worse for both time/memory complexity
            
            for start in 0..self.goal.state_transitions.len() {
                for element in &self.ss_link_graph[i].original_idxs {
                    if self.goal.contains_from_start(&sig_set[*element], start) {
                        self.ss_link_graph[i].accepting_states.push(start);
                        break
                    }
                }
                self.ss_link_graph[i].accepting_states.shrink_to_fit();
            }
        }*/


    }

    //Checks to see if a potentially new element of the minkid set is actually an ancestor to a pre-existing minkid
    //false means it is distinct from the current set
    fn check_if_ancestor(&self, min_children : &HashSet<NodeIndex>, potential : NodeIndex) -> bool {
        //This checks all children of the potential element.
        //If there's a minkid in the children of this potential element, we know that the potential element is redundant
        let mut dfs = Dfs::new(&self.ss_link_graph, potential);
        while let Some(nx) = dfs.next(&self.ss_link_graph) {
            
            if min_children.contains(&nx) {
                return true;
            }
        }
        false
    }
    //checks which elements of the minkid vec are ancestors of a potential minkid element
    //This is currently sub-optimal -- assuming checks are done properly, there are no children of a minkid element that are also within the minkid set
    //this means the DFS checks unnecesary values. But! This is just a sanitation method anyway -- hopefully it's not in the final cut
    fn check_if_childer(&self, min_children : &HashSet<NodeIndex>, potential : NodeIndex) -> HashSet<NodeIndex> {
        let mut result = HashSet::new();
        let reversed_graph = petgraph::visit::Reversed(&self.ss_link_graph);
        let mut dfs = Dfs::new(&reversed_graph, potential);
        while let Some(nx) = dfs.next(&reversed_graph) {
            //If a minkid element is an ancestor to the potential guy
            if min_children.contains(&nx) {
                result.insert(nx);
            }
        }
        result
    }
    //notably sub-optimal -- i am keeping things readble first because I am gonna go cross-eyed if I pre-emptively optimize THIS
    //Returns true if minkids is modified
    fn add_to_minkids(&self, min_children : &mut HashSet<NodeIndex>, potential : NodeIndex) -> bool {
        if self.check_if_ancestor(min_children, potential) {
            return false;
        }
        let redundant_kids = self.check_if_childer(min_children, potential);
        min_children.insert(potential);
        //This could be dumb!
        *min_children = min_children.difference(&redundant_kids).map(|x| *x).collect::<HashSet<_>>();
        return !redundant_kids.is_empty();
    }

    fn build_sig_k(&self, k : usize) -> Vec<Vec<SymbolIdx>> {
        //let start_sig_len : usize = (cardinality::<S>() << k)-1;
        let mut start_index = 0;
        let mut signature_set : Vec<Vec<SymbolIdx>> = vec![vec![]];
        let mut end_index = 1;
        let mut new_index = 1;
        for _ in 0..k {
            for i in start_index..end_index{
                for symbol in 0..(self.symbol_set.length as SymbolIdx) {
                    signature_set.push(signature_set[i].clone());
                    signature_set[new_index].push(symbol);
                    new_index += 1;
                }
            }
            start_index = end_index;
            end_index = new_index;
        }
        signature_set
    }

    /* 
    fn dfs_solver(&self, start_board : &Vec<S>) -> bool {
        match self.dfs_pathed_helper(0, &mut HashSet::new(), start_board) {
            Some(_) => true,
            None => false
        }
    }
    
    fn dfs_pather(&self, start_board : &Vec<S>) -> Option<Vec<usize>> {
        return match self.dfs_pathed_helper(0, &mut HashSet::new(), start_board) {
            None => {None}
            Some(mut path) => {path.reverse();Some(path)}
        }
    }
    //this is not fucking happening today lmao
    fn path_decycler(&self, path : Vec<usize>) {
        let mut match_length : Vec<usize> = vec![0;path.len()/2];
        let mut new_match_length : Vec<usize> = vec![0;path.len()/2];
        let mut new_path : Vec<usize> = vec![];
        for idx in 0..path.len() {
            std::mem::swap(&mut match_length, &mut new_match_length);
            new_match_length = vec![0;path.len()/2];
            for cycle in 0..path.len()/2 {
                if cycle*2 + idx < path.len()   {
                    let mut does_match = true;
                    for cycle_check in 0..cycle {
                        does_match &= path[idx+cycle_check] == path[idx+cycle+cycle_check];
                    }
                    if does_match {
                        new_match_length[cycle] += 1;
                    } else {
                        new_match_length[cycle] = 0;
                    }
                }else{
                    new_match_length[cycle] = 0;
                }
            }
            if new_match_length.iter().all(|x| *x == 0) {
                let mut max = 0;
                let mut max_idx = 0;
                for (idx,cycle) in match_length.iter().enumerate() {
                    //max length of 2-cycle is 4 even if match length is 5
                    if (cycle/idx)*idx > max {
                        max = *cycle;
                        max_idx = idx;
                    }
                }
                
            }
        }
    }

    fn dfs_pathed_helper(&self, cursor : usize, known_states : &mut HashSet::<(usize,Vec<S>)>, board : &Vec<S>) -> Option<Vec<usize>> {
        let mut next_boards : Vec<(Vec<usize>,usize,Vec<S>)> = Vec::new();
        if self.goal.contains(&board) {
            return Some(vec![]);
        }
        let mut index = 0;
        let mut buffer : Vec<S> = Vec::new();
        while index < board.len() && index < self.rules.min_input - 1{
            buffer.push_back(board[index]);
            index += 1;
        }
        while index < board.len(){
           
                    if !known_states.contains(&(cursor,new_board.clone())) {
                        known_states.insert((cursor,new_board.clone()));
                        let mut rules_used = vec![rule_idx+2];
                        let l_or_r = match cursor > copy_index {
                            true => 0,
                            false => 1
                        };
                        for _ in 0..(cursor as i32 - copy_index as i32).abs() {
                            rules_used.push(l_or_r);
                        }
                        next_boards.push((rules_used,copy_index,new_board));
                    }
                }
            }
            index += 1;
        }
        while let Some(mut i) = next_boards.pop() {
            if let Some(mut path) = self.dfs_pathed_helper(i.1, known_states, &i.2) {
                path.append(&mut i.0);
                return Some(path);
            }
        }
        None
    }
    */
    //Way less memory usage because no addition/checking HashMap.
    //Also paralellizable, hence "batch"
    fn bfs_solver_batch(&self, start_board : &Vec<SymbolIdx>) -> bool { 
        let mut new_boards : Vec<Vec<SymbolIdx>> = vec![start_board.clone()];
        let mut old_boards : Vec<Vec<SymbolIdx>> = vec![];
        let mut known_states = HashSet::<Vec<SymbolIdx>>::new();
        known_states.insert(start_board.clone());
        while new_boards.len() > 0 {
            std::mem::swap(&mut old_boards, &mut new_boards);
            new_boards.clear();
            for board in &old_boards {
                if self.goal.contains(board) {
                    return true;
                }
                for new_board in self.rules.single_rule_hash(board) {
                    if !known_states.contains(&new_board) {
                        known_states.insert(new_board.clone());
                        new_boards.push(new_board);
                    }
                }
            }
        }
        false
    }

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
        for new_board in self.rules.single_rule_hash(&start_board) {
            let mut dfa_idx = 0;
            let mut board_idx = 0;
            //Find the location of the changed board in the DFA
            while board_idx < new_board.len() && dfa_idx < self.trans_table.len() {
                dfa_idx = self.trans_table[dfa_idx][new_board[board_idx] as usize];
                board_idx += 1;
            }
            if self.bfs_solver_sub(&new_board, dfa_idx, self.symbol_set.find_in_sig_set(new_board[board_idx..].iter()),investigated) {
                self.sig_sets[state_idx].set(sig_idx,true);
                break
            }
        }
        self.solved_yet[state_idx - self.trans_table.len()].set(sig_idx,true);
        self.sig_sets[state_idx][sig_idx]
    }

    fn bfs_solver(&mut self, start_board : &Vec<SymbolIdx>) -> bool {
        let mut start_idx = 0;
        let mut end_idx = 0;
        let mut all_boards : Vec<(usize,Vec<SymbolIdx>)> = vec![(0,start_board.clone())];
        let mut known_states = HashSet::<Vec<SymbolIdx>>::new();
        known_states.insert(start_board.clone());
        let mut answer_idx = 0;
        let mut answer_found = false;
        while (start_idx != end_idx || start_idx == 0) && !answer_found{
            start_idx = end_idx;
            end_idx = all_boards.len();
            for board_idx in start_idx..end_idx{
                if self.goal.contains(&all_boards[board_idx].1) {
                    answer_idx = board_idx;
                    answer_found = true;
                    break;
                }
                if let Some(found_answer) = self.board_solutions.get(&all_boards[board_idx].1) {
                    if !*found_answer {
                        continue
                    }else{
                        answer_idx = board_idx;
                        answer_found = true;
                        break;
                    }
                }
                for new_board in self.rules.single_rule_hash(&all_boards[board_idx].1) {
                    if !known_states.contains(&new_board) {
                        known_states.insert(new_board.clone());
                        all_boards.push((board_idx,new_board));
                    }
                }
            }
        }
        //did we find an answer board
        match answer_found{
            false => {
                //if it's unsolvable, then we know everything here is
            while let Some((_,board)) = all_boards.pop() {
                self.board_solutions.insert(board,false);
            }
            false
            }
            //this can be dramatically improved i think
            //following path of solvability
            true => {
                while answer_idx != 0 {
                    self.board_solutions.insert(all_boards[answer_idx].1.clone(),true);
                    answer_idx = all_boards[answer_idx].0;
                }
                self.board_solutions.insert(all_boards[0].1.clone(),true);
            true
            }
        }
    }
    fn sig_with_set(&mut self, board : &Vec<SymbolIdx>, sig_set : &Vec<Vec<SymbolIdx>>) -> Vec<bool> {
        let mut result : Vec<bool> = Vec::new();
        for sig_element in sig_set {
            let mut new_board = board.clone();
            new_board.extend(sig_element);
            result.push(self.bfs_solver(&new_board));
        }
        result
    }

    fn sig_with_set_sub(&mut self, board : &Vec<SymbolIdx>, sig_set : &Vec<Vec<SymbolIdx>>, state_idx : usize) {
        let solved_idx = state_idx - self.trans_table.len();
        let mut investigated = HashSet::new();
        for (idx,sig_element) in sig_set.iter().enumerate() {
            if !self.solved_yet[solved_idx][idx] {
                let mut new_board = board.clone();
                new_board.extend(sig_element);
                let result = self.bfs_solver_sub(&new_board, state_idx, idx, &mut investigated);
                investigated.clear();
            }
        }
    }

    fn sig_with_set_reverse(&mut self, board : &Vec<SymbolIdx>, sig_set : &Vec<Vec<SymbolIdx>>, accepted_boards : &HashSet<Vec<SymbolIdx>>) -> Vec<bool> {
        let mut result : Vec<bool> = Vec::with_capacity(sig_set.len());
        for sig_element in sig_set {
            let mut new_board = board.clone();
            new_board.extend(sig_element);
            result.push(accepted_boards.contains(&new_board));
        }
        result
    }

    fn sig_with_set_batch(&self, board : &Vec<SymbolIdx>, sig_set : &Vec<Vec<SymbolIdx>>, input : Arc<SegQueue<(Vec<SymbolIdx>,usize)>>, output : Arc<SegQueue<(bool,usize)>>) -> Vec<bool> {
        let mut result : Vec<bool> = vec![false;sig_set.len()];

        for sig_element in sig_set.iter().enumerate() {
            let mut new_board = board.clone();
            new_board.extend(sig_element.1);
            input.push((new_board,sig_element.0));
        }
        let mut results_recieved = 0;
        while results_recieved < sig_set.len() {
            match output.pop() {
                Some(output_result) => {
                    result[output_result.1] = output_result.0; 
                    results_recieved+=1;
                },
                None => {std::thread::sleep(time::Duration::from_millis(10));}
            }
        }
        //println!("{} {}",output.len(),input.len());
        result
    }

    fn board_to_next_batch(&self,board : &Vec<SymbolIdx>, sig_set : &Vec<Vec<SymbolIdx>>,input : &Arc<SegQueue<(Vec<SymbolIdx>,usize)>>, output : &Arc<SegQueue<(bool,usize)>>) -> Vec<(Vec<bool>,Vec<SymbolIdx>)> {
        let mut results : Vec<(Vec<bool>,Vec<SymbolIdx>)> = Vec::with_capacity(self.symbol_set.length);
        for sym in 0..(self.symbol_set.length as SymbolIdx) {
            let mut new_board = board.clone();
            new_board.push(sym);
            results.push((self.sig_with_set_batch(&new_board,sig_set,input.clone(),output.clone()),new_board));

        }
        results
    }

    fn board_to_next(&mut self,board : &Vec<SymbolIdx>, sig_set : &Vec<Vec<SymbolIdx>>) -> Vec<(Vec<bool>,Vec<SymbolIdx>)> {
        let mut results : Vec<(Vec<bool>,Vec<SymbolIdx>)> = Vec::with_capacity(self.symbol_set.length);
        for sym in 0..(self.symbol_set.length as SymbolIdx) {
            let mut new_board = board.clone();
            new_board.push(sym);
            results.push((self.sig_with_set(&new_board,sig_set),new_board));

        }
        results
    }
    fn board_to_next_reverse(&mut self,board : &Vec<SymbolIdx>, sig_set : &Vec<Vec<SymbolIdx>>, accepted_boards : &HashSet<Vec<SymbolIdx>>) -> Vec<(Vec<bool>,Vec<SymbolIdx>)> {
        let mut results : Vec<(Vec<bool>,Vec<SymbolIdx>)> = Vec::with_capacity(self.symbol_set.length);
        for sym in 0..(self.symbol_set.length as SymbolIdx) {
            let mut new_board = board.clone();
            new_board.push(sym);
            results.push((self.sig_with_set_reverse(&new_board,sig_set,accepted_boards),new_board));

        }
        results
    }

    fn dfa_with_sig_set_batch(&self, sig_set : &Vec<Vec<SymbolIdx>>) -> DFA {
        let mut trans_table : Vec<Vec<usize>> = Vec::new(); //omg it's me !!!
        let mut table_reference = HashMap::<Vec<bool>,usize>::new();
    
        let mut new_boards : Vec::<(usize,Vec<SymbolIdx>)> = vec![(0,vec![])];
    
        let mut old_boards : Vec::<(usize,Vec<SymbolIdx>)> = Vec::new();
    
        let mut accepting_states : HashSet<usize> = HashSet::new();
        
        let thread_translator : Arc<SRSTranslator> = Arc::new(self.clone());

        let (input, output) = self.create_workers(WORKERS);

        let mut empty_copy : Vec<usize> = Vec::new();
        for _ in 0..self.symbol_set.length {
            empty_copy.push(0);
        }

        let start_accepting = self.sig_with_set_batch(&vec![],&sig_set, input.clone(), output.clone());
        table_reference.insert(start_accepting.clone(),0);
        trans_table.push(empty_copy.clone());

        //redundant bc of start_accepting already checking this but idc
        if self.goal.contains(&vec![]) {
            accepting_states.insert(0);
        }
    
        while new_boards.len() > 0 {
            let iter_begin_time = Instant::now();
            std::mem::swap(&mut old_boards,&mut new_boards);
            new_boards.clear(); 
            println!("Thinking about {} states...",old_boards.len());
            print!("{} States | Length {} |",old_boards.len(),old_boards[0].1.len());
    
            for (start_idx,board) in &old_boards {
                //Finds ingoing end of board.
                
                //Gets sig set of all boards with a single symbol added.
                //TODO: Use pool of worker threads used with main-thread-blocking sig set requests.
                //Change Translator to a trait and add a batch SRSTranslator and a hash SRSTranslator.
                let next_results = self.board_to_next_batch(&board, sig_set, &input, &output);
                for (sym_idx,new_board) in next_results.iter().enumerate() {
                    //Checking if the next board's sig set already exists in DFA
                    let dest_idx = match table_reference.get(&new_board.0) {
                        //If it does, the arrow's obv going to the existing state in the DFA
                        Some(idx) => {
                            *idx
                        },
                        //If it doesn't, add a new state to the DFA!
                        None => {
                            let new_idx = trans_table.len();
                            new_boards.push((new_idx,new_board.1.clone()));
                            
                            
                            table_reference.insert(new_board.0.clone(),new_idx);
                            trans_table.push(empty_copy.clone());
    
                            if thread_translator.bfs_solver_batch(&new_board.1) {
                                accepting_states.insert(new_idx);
                            }
                            new_idx
                            }
                        };
                    trans_table[*start_idx][sym_idx] = dest_idx;
                    }  
                    
                }
                println!(" {} ms to complete",iter_begin_time.elapsed().as_millis());
            }
    
    self.terminate_workers(input, WORKERS);
    DFA {
        state_transitions : trans_table,
        accepting_states : accepting_states,
        starting_state : 0,
        symbol_set : self.symbol_set.clone()
    }
}
    fn create_workers(&self, worker_count : usize) -> (Arc<SegQueue<(Vec<SymbolIdx>,usize)>>,Arc<SegQueue<(bool,usize)>>) {
        let thread_translator : Arc<SRSTranslator> = Arc::new(self.clone());

        let input : Arc<SegQueue<(Vec<SymbolIdx>,usize)>> = Arc::new(SegQueue::new());
        let output : Arc<SegQueue<(bool,usize)>> = Arc::new(SegQueue::new());

        for _ in 0..worker_count {
            worker_thread(thread_translator.clone(), input.clone(), output.clone());
        }
        (input, output)
    }
    fn terminate_workers(&self, input : Arc<SegQueue<(Vec<SymbolIdx>,usize)>>, worker_count : usize) {
        for _ in 0..worker_count {
            input.push((vec![69, 42], usize::MAX))
        }
    }

    fn dfa_with_sig_set(&mut self, sig_set : &Vec<Vec<SymbolIdx>>) -> DFA {
        let mut trans_table : Vec<Vec<usize>> = Vec::new(); //omg it's me !!!
        let mut table_reference = HashMap::<Vec<bool>,usize>::new();
    
        let mut new_boards : Vec::<(usize,Vec<SymbolIdx>)> = vec![(0,vec![])];
    
        let mut old_boards : Vec::<(usize,Vec<SymbolIdx>)> = Vec::new();
    
        let mut accepting_states : HashSet<usize> = HashSet::new();
        

        let mut empty_copy : Vec<usize> = Vec::new();
        for _ in 0..self.symbol_set.length {
            empty_copy.push(0);
        }

        let start_accepting = self.sig_with_set(&vec![],&sig_set);
        table_reference.insert(start_accepting.clone(),0);
        trans_table.push(empty_copy.clone());

        //redundant bc of start_accepting already checking this but idc
        if self.bfs_solver(&vec![]) {
            accepting_states.insert(0);
        }
    
        while new_boards.len() > 0 {
            let iter_begin_time = Instant::now();
            std::mem::swap(&mut old_boards,&mut new_boards);
            new_boards.clear(); 
            print!("{} States | Length {} |",old_boards.len(),old_boards[0].1.len());
            //Horrific hack for 3xk boards. godspeed soldier
            self.board_solutions = HashMap::new();
            for (start_idx,board) in &old_boards {
                //Finds ingoing end of board.
                
                //Gets sig set of all boards with a single symbol added.
                let next_results = self.board_to_next(&board, sig_set);
                for (sym_idx,new_board) in next_results.iter().enumerate() {
                    //Checking if the next board's sig set already exists in DFA
                    let dest_idx = match table_reference.get(&new_board.0) {
                        //If it does, the arrow's obv going to the existing state in the DFA
                        Some(idx) => {
                            *idx
                        },
                        //If it doesn't, add a new state to the DFA!
                        None => {
                            let new_idx = trans_table.len();
                            new_boards.push((new_idx,new_board.1.clone()));
                            
                            
                            table_reference.insert(new_board.0.clone(),new_idx);
                            trans_table.push(empty_copy.clone());
    
                            if self.bfs_solver(&new_board.1) {
                                accepting_states.insert(new_idx);
                            }
                            new_idx
                            }
                        };
                    trans_table[*start_idx][sym_idx] = dest_idx;
                    }  
                    
                }
                println!(" {} ms",iter_begin_time.elapsed().as_millis());
            }
    DFA {
        state_transitions : trans_table,
        accepting_states : accepting_states,
        starting_state : 0,
        symbol_set : self.symbol_set.clone()
    }
    
}

fn dfa_with_sig_set_subset(&mut self, sig_set_size : usize) -> DFA {


    //graph of connections based on LHS->RHS links for all states
    //Usize is index in trans_table
    
    
    let sig_set = &self.build_sig_k(sig_set_size);

    //not allowed to complain about my dumb code -- not everything will be optimal i have DEADLINES.
    //okay i'm the one making up the deadlines... but still
    let smaller_sig = self.build_sig_k(sig_set_size - 1);

    //list of strings for the newest known states

    let mut recent_strings = vec![vec![]];

    let mut new_recent_strings = vec![];

    self.solved_yet.push(bitvec![0;sig_set.len()]);

    self.sig_sets.push(bitvec![0;sig_set.len()]);
    let mut start_values = bitvec![0;sig_set.len()];
    //println!("{:?},{:?}",start_known,start_values);
    self.sig_with_set_sub(&vec![], &sig_set, 0);
    self.trans_table.push((1..=self.symbol_set.length).collect());
    self.unique_sigs.insert(self.sig_sets[0].clone(),0);

    self.solved_yet = vec![];

    //number of known states at last pass
    let mut last_known : usize = 1;
    //number of states with finished edges
    let mut last_finished : usize = 0;
    let mut update_string = "".to_owned();

    
     //while there are still states to process
 
    while last_finished < last_known{
        update_string = "".to_owned();

        let begin_time = Instant::now();

        update_string += &format!("{} States | ", last_known-last_finished);
        print!("{}\r",update_string);
        io::stdout().flush().unwrap();

        //println!("{:?}",self.sig_sets.last().unwrap());
        //First step is populating self.sig_sets and self.solved_yet 
        
        //trans_table should already be correct? make sure to that when adding elements
        let new_states = (last_known - last_finished) * self.symbol_set.length;
        self.sig_sets.resize(self.sig_sets.len()+new_states,bitvec![0;sig_set.len()]);
        self.solved_yet.resize(new_states,bitvec![0;sig_set.len()]);

        //next is adding all edges appropriately to the graph. 
        //this can be optimized substantially but i don't wanna do it pre-emptively :)
        let mut link_graph = DiGraph::<usize,()>::new();

        for index in 0..(last_known + new_states) {
            link_graph.add_node(index);
        }
        for origin in 0..last_known {
            for rule in &self.rules.rules {
                let mut parent = origin;
                let mut child = origin;
                let mut valid = true;
                for i in 0..rule.0.len() {
                    if parent >= last_known || child >= last_known {
                        valid = false;
                        break;
                    }
                    parent = self.trans_table[parent][rule.0[i] as usize];
                    child = self.trans_table[child][rule.1[i] as usize];
                }
                if valid {
                    link_graph.update_edge(NodeIndex::new(parent),NodeIndex::new(child),());
                }
            }  
        }
        // After establishing the starting points of all links, extend those links outward.
        let mut old_len = 0;
        while old_len < link_graph.edge_count() {
            let new_len = link_graph.edge_count();
            for edge_idx in old_len..new_len {
                for sym in 0..self.symbol_set.length {
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
                    let old_idx = self.symbol_set.find_in_sig_set(elem_in_origin.iter());
                    let new_idx = self.symbol_set.find_in_sig_set(elem.iter());
                    let scared_rust = self.sig_sets[origin_idx][old_idx];
                    self.sig_sets[*move_idx].set(new_idx,scared_rust);
                    self.solved_yet[move_idx - last_known].set(new_idx,true);
                }
            }
        }
        
        //cycle detection and removal. note that this changes the type of node_weight from usize to Vec<usize>. 
        //tests indicate that this vec is always sorted smallest to largest, but this fact may not hold true if code is modified.
        let initial_nodes = link_graph.node_count();
        let link_graph = condensation(link_graph, true);

        update_string += &format!("{} Links | {} Cyclic duplicates | ", link_graph.edge_count(),initial_nodes - link_graph.node_count());
        print!("{}\r",update_string);
        io::stdout().flush().unwrap();
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
            /* 
            let mut visit = HashSet::new();
            visit.insert(origin_node);
            let mut explore = vec![origin_node];
            
            while let Some(nx) = explore.pop() {
                for neighbor in link_graph.neighbors_directed(nx,Direction::Outgoing) {
                    if link_graph[neighbor][0] >= last_known && !visit.contains(&neighbor) {
                        visit.insert(neighbor);
                        explore.push(neighbor);

                        self.solved_yet[link_graph[neighbor][0] - last_known] |= !self.sig_sets[origin].clone();
                    }
                }
            } */

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
        let process_begin_time = Instant::now();
        let mut processed_states = 0;
        let mut skipped_strings = 0;
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
                        (!self.sig_sets[link_graph[neighbor][0]].clone() & scared_rust);
                    }
                }
                //creating a string to actually test with
                let connecting_state = (link_graph[node][0] - last_known) / self.symbol_set.length;
                let connecting_symbol = ((link_graph[node][0] - last_known) % self.symbol_set.length) as SymbolIdx;
                let mut new_board = recent_strings[connecting_state].clone();
                new_board.push(connecting_symbol);
                skipped_strings += self.solved_yet[link_graph[node][0]- last_known].count_ones();
                self.sig_with_set_sub(&new_board, &sig_set, link_graph[node][0]);
                processed_states += 1;
                print!("{}{}/{} Skipped | {}/{} Calculated\r",update_string,skipped_strings, processed_states * sig_set.len(), processed_states,new_states);
                io::stdout().flush().unwrap();
            }
        }

        update_string += &format!("{}/{} Skipped | ~{:.3} ms per string | ", skipped_strings, processed_states * sig_set.len(), 
            ((Instant::now() -process_begin_time).as_millis() as f64) / ((processed_states * sig_set.len() - skipped_strings) as f64));
        print!("{}\r",update_string);
        io::stdout().flush().unwrap();
        //println!("{:?}",self.sig_sets[0]);
        //Now, we look at all prospective states' signature sets and add the unique ones.
        let mut new_known = 0;
        let mut new_sig_sets = vec![];
        let mut new_identified = 0;
        for pros_state in link_graph.node_indices() {
            //If there's an equivalent state that already exists in the DFA, use that!
            let connector = match link_graph[pros_state].iter().find(|&x| x < &last_known) {
                Some(idx) => {
                    *idx
                },
                None => {
                    print!("{}{}/{} Identified\r",update_string,new_identified,new_states);
                    new_identified += 1;
                    match self.unique_sigs.get(&self.sig_sets[link_graph[pros_state][0]]) {
                        Some(i) => {*i}
                        None => {
                            let connecting_state = (link_graph[pros_state][0] - last_known) / self.symbol_set.length + last_finished;
                            let connecting_symbol = ((link_graph[pros_state][0] - last_known) % self.symbol_set.length) as SymbolIdx;
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
                let connecting_state = (dupe - last_known) / self.symbol_set.length + last_finished;
                let connecting_symbol = ((dupe - last_known) % self.symbol_set.length);
                self.trans_table[connecting_state][connecting_symbol] = connector;
            }
        }



        //Now we clean up -- no prospective states left over anywhere!

        self.sig_sets.truncate(last_known);
        self.sig_sets.append(&mut new_sig_sets);

        self.solved_yet.clear();

        for i in 0..new_known {
            self.trans_table.push(((last_known+new_known+i*self.symbol_set.length)..=(last_known+new_known+(i+1)*self.symbol_set.length-1)).collect())
        }
        last_finished = last_known;
        last_known = self.trans_table.len();

        std::mem::swap(&mut recent_strings, &mut new_recent_strings);
        new_recent_strings.clear();
        println!("{}{} ms               ", update_string,(Instant::now()-begin_time).as_millis());
    }
    let mut accepting_states = HashSet::new();
    for (key, val) in self.unique_sigs.iter() {
        if key[0] {
            accepting_states.insert(*val);
        }
    }
    let trans_table = self.trans_table.clone();
    self.trans_table = vec![];
    self.unique_sigs = HashMap::new();
    self.solved_yet = vec![];
    self.sig_sets = vec![];
    DFA {
        state_transitions : trans_table,
        accepting_states : accepting_states,
        starting_state : 0,
        symbol_set : self.symbol_set.clone()
    }

}

fn dfa_with_sig_set_ancestor(&mut self, sig_set_size : usize) -> DFA {


    //graph of connections based on LHS->RHS links for all states
    //Usize is index in trans_table
    
    
    let sig_set = &self.build_sig_k(sig_set_size);

    //not allowed to complain about my dumb code -- not everything will be optimal i have DEADLINES.
    //okay i'm the one making up the deadlines... but still
    let smaller_sig = self.build_sig_k(sig_set_size - 1);

    //list of strings for the newest known states

    let mut recent_strings = vec![vec![]];

    let mut new_recent_strings = vec![];

    self.solved_yet.push(bitvec![0;sig_set.len()]);

    self.sig_sets.push(bitvec![0;sig_set.len()]);
    let mut start_values = bitvec![0;sig_set.len()];
    //println!("{:?},{:?}",start_known,start_values);
    self.sig_with_set_sub(&vec![], &sig_set, 0);
    self.trans_table.push((1..=self.symbol_set.length).collect());
    self.unique_sigs.insert(self.sig_sets[0].clone(),0);

    self.solved_yet = vec![];

    //number of known states at last pass
    let mut last_known : usize = 1;
    //number of states with finished edges
    let mut last_finished : usize = 0;
    let mut update_string = "".to_owned();

    
     //while there are still states to process
 
    while last_finished < last_known{
        update_string = "".to_owned();

        let begin_time = Instant::now();

        update_string += &format!("{} States | ", last_known-last_finished);
        print!("{}\r",update_string);
        io::stdout().flush().unwrap();

        //println!("{:?}",self.sig_sets.last().unwrap());
        //First step is populating self.sig_sets and self.solved_yet 
        
        //trans_table should already be correct? make sure to that when adding elements
        let new_states = (last_known - last_finished) * self.symbol_set.length;
        self.sig_sets.resize(self.sig_sets.len()+new_states,bitvec![0;sig_set.len()]);
        self.solved_yet.resize(new_states,bitvec![0;sig_set.len()]);

        //next is adding all edges appropriately to the graph. 
        //this can be optimized substantially but i don't wanna do it pre-emptively :)
        let mut link_graph = DiGraph::<usize,()>::new();

        for index in 0..(last_known + new_states) {
            link_graph.add_node(index);
        }
        for origin in 0..last_known {
            for rule in &self.rules.rules {
                let mut parent = origin;
                let mut child = origin;
                let mut valid = true;
                for i in 0..rule.0.len() {
                    if parent >= last_known || child >= last_known {
                        valid = false;
                        break;
                    }
                    parent = self.trans_table[parent][rule.0[i] as usize];
                    child = self.trans_table[child][rule.1[i] as usize];
                }
                if valid {
                    link_graph.update_edge(NodeIndex::new(parent),NodeIndex::new(child),());
                }
            }  
        }
        // After establishing the starting points of all links, extend those links outward.
        let mut old_len = 0;
        while old_len < link_graph.edge_count() {
            let new_len = link_graph.edge_count();
            for edge_idx in old_len..new_len {
                for sym in 0..self.symbol_set.length {
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
                    let old_idx = self.symbol_set.find_in_sig_set(elem_in_origin.iter());
                    let new_idx = self.symbol_set.find_in_sig_set(elem.iter());
                    let scared_rust = self.sig_sets[origin_idx][old_idx];
                    self.sig_sets[*move_idx].set(new_idx,scared_rust);
                    self.solved_yet[move_idx - last_known].set(new_idx,true);
                }
            }
        }
        
        //cycle detection and removal. note that this changes the type of node_weight from usize to Vec<usize>. 
        //tests indicate that this vec is always sorted smallest to largest, but this fact may not hold true if code is modified.
        let initial_nodes = link_graph.node_count();
        let link_graph = condensation(link_graph, true);

        update_string += &format!("{} Links | {} Cyclic duplicates | ", link_graph.edge_count(),initial_nodes - link_graph.node_count());
        print!("{}\r",update_string);
        io::stdout().flush().unwrap();
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
            /* 
            let mut visit = HashSet::new();
            visit.insert(origin_node);
            let mut explore = vec![origin_node];
            
            while let Some(nx) = explore.pop() {
                for neighbor in link_graph.neighbors_directed(nx,Direction::Outgoing) {
                    if link_graph[neighbor][0] >= last_known && !visit.contains(&neighbor) {
                        visit.insert(neighbor);
                        explore.push(neighbor);

                        self.solved_yet[link_graph[neighbor][0] - last_known] |= !self.sig_sets[origin].clone();
                    }
                }
            } */

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
        let process_begin_time = Instant::now();
        let mut processed_states = 0;
        let mut skipped_strings = 0;
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
                        (!self.sig_sets[link_graph[neighbor][0]].clone() & scared_rust);
                    }
                }
                //creating a string to actually test with
                let connecting_state = (link_graph[node][0] - last_known) / self.symbol_set.length;
                let connecting_symbol = ((link_graph[node][0] - last_known) % self.symbol_set.length) as SymbolIdx;
                let mut new_board = recent_strings[connecting_state].clone();
                new_board.push(connecting_symbol);
                skipped_strings += self.solved_yet[link_graph[node][0]- last_known].count_ones();
                self.sig_with_set_sub(&new_board, &sig_set, link_graph[node][0]);
                processed_states += 1;
                print!("{}{}/{} Skipped | {}/{} Calculated\r",update_string,skipped_strings, processed_states * sig_set.len(), processed_states,new_states);
                io::stdout().flush().unwrap();
            }
        }

        update_string += &format!("{}/{} Skipped | ~{:.3} ms per string | ", skipped_strings, processed_states * sig_set.len(), 
            ((Instant::now() -process_begin_time).as_millis() as f64) / ((processed_states * sig_set.len() - skipped_strings) as f64));
        print!("{}\r",update_string);
        io::stdout().flush().unwrap();
        //println!("{:?}",self.sig_sets[0]);
        //Now, we look at all prospective states' signature sets and add the unique ones.
        let mut new_known = 0;
        let mut new_sig_sets = vec![];
        let mut new_identified = 0;
        for pros_state in link_graph.node_indices() {
            //If there's an equivalent state that already exists in the DFA, use that!
            let connector = match link_graph[pros_state].iter().find(|&x| x < &last_known) {
                Some(idx) => {
                    *idx
                },
                None => {
                    print!("{}{}/{} Identified\r",update_string,new_identified,new_states);
                    new_identified += 1;
                    match self.unique_sigs.get(&self.sig_sets[link_graph[pros_state][0]]) {
                        Some(i) => {*i}
                        None => {
                            let connecting_state = (link_graph[pros_state][0] - last_known) / self.symbol_set.length + last_finished;
                            let connecting_symbol = ((link_graph[pros_state][0] - last_known) % self.symbol_set.length) as SymbolIdx;
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
                let connecting_state = (dupe - last_known) / self.symbol_set.length + last_finished;
                let connecting_symbol = ((dupe - last_known) % self.symbol_set.length);
                self.trans_table[connecting_state][connecting_symbol] = connector;
            }
        }



        //Now we clean up -- no prospective states left over anywhere!

        self.sig_sets.truncate(last_known);
        self.sig_sets.append(&mut new_sig_sets);

        self.solved_yet.clear();

        for i in 0..new_known {
            self.trans_table.push(((last_known+new_known+i*self.symbol_set.length)..=(last_known+new_known+(i+1)*self.symbol_set.length-1)).collect())
        }
        last_finished = last_known;
        last_known = self.trans_table.len();

        std::mem::swap(&mut recent_strings, &mut new_recent_strings);
        new_recent_strings.clear();
        println!("{}{} ms               ", update_string,(Instant::now()-begin_time).as_millis());
    }
    let mut accepting_states = HashSet::new();
    for (key, val) in self.unique_sigs.iter() {
        if key[0] {
            accepting_states.insert(*val);
        }
    }
    println!("Building SigSet link graph...");
    self.build_ss_link_graph(sig_set_size, sig_set);
    let trans_table = self.trans_table.clone();
    //Building data for complexity (memory + time) of ancestor method
    println!("Built! Building data for comparison...");
    let mut total_ancestors = 0;
    for dfa_state in 0..self.trans_table.len() {
        
        let mut minimal_children : HashSet<NodeIndex> = HashSet::new();
        //Sorted so this way all children of a node come before it, meaning no checks for backsies
        for element in toposort(&petgraph::visit::Reversed(&self.ss_link_graph), None).unwrap() {
            //If this isn't accepting...we don't care abt it
            if !self.sig_sets[dfa_state][self.ss_link_graph[element].original_idxs[0]] {
                continue;
            }
            //Let's make sure this isn't already an ancestor of a known child.
            if self.check_if_ancestor(&minimal_children, element){
                continue;
            }
            minimal_children.insert(element);
        }
        total_ancestors += minimal_children.len();
        println!("another chugged thru...")
    }
    println!("{} total ancestors, average of {:.2} per state.",total_ancestors, (total_ancestors as f32) / (self.trans_table.len() as f32));
    println!("{}b in old version, {}b in new. Compression ratio of {:.2}",self.sig_sets[0].len()*self.sig_sets.len(), total_ancestors * 64, (self.sig_sets[0].len()*self.sig_sets.len()) as f32 / (total_ancestors * 64) as f32);
    self.trans_table = vec![];
    self.unique_sigs = HashMap::new();
    self.solved_yet = vec![];
    self.sig_sets = vec![];
    DFA {
        state_transitions : trans_table,
        accepting_states : accepting_states,
        starting_state : 0,
        symbol_set : self.symbol_set.clone()
    }

}

fn dfa_with_sig_set_reverse(&mut self, sig_set : &Vec<Vec<SymbolIdx>>) -> DFA {
    let mut trans_table : Vec<Vec<usize>> = Vec::new(); //omg it's me !!!
    let mut table_reference = HashMap::<Vec<bool>,usize>::new();

    let mut new_boards : Vec::<(usize,Vec<SymbolIdx>)> = vec![(0,vec![])];

    let mut old_boards : Vec::<(usize,Vec<SymbolIdx>)> = Vec::new();

    let mut accepting_states : HashSet<usize> = HashSet::new();
    

    let mut empty_copy : Vec<usize> = Vec::new();
    for _ in 0..self.symbol_set.length {
        empty_copy.push(0);
    }

    let start_accepting = self.sig_with_set(&vec![],&sig_set);
    table_reference.insert(start_accepting.clone(),0);
    trans_table.push(empty_copy.clone());

    //redundant bc of start_accepting already checking this but idc
    if self.bfs_solver(&vec![]) {
        accepting_states.insert(0);
    }
    let mut accepted_boards : HashSet<Vec<SymbolIdx>> = HashSet::new();
    while new_boards.len() > 0 {
        let iter_begin_time = Instant::now();
        std::mem::swap(&mut old_boards,&mut new_boards);
        new_boards.clear(); 
        println!("{} States to think about...",old_boards.len());
        print!("{} States | Length {} |",old_boards.len(),old_boards[0].1.len());
        //Horrific hack for 3xk boards. godspeed soldier
        //5 HERE IS ALSO A HORRIFIC HACK. WE ARE BEYOND THE LOOKING GLASS. WE ARE FIGHTING FOR SURVIVAL.
         
        let mut starting_boards = vec![];
        /*
        for i in 0..5 {
            starting_boards.push(vec![0;old_boards[0].1.len()+i]);
        }*/
         
        for masta in (old_boards[0].1.len()+1)..(old_boards[0].1.len()+7) {
            for l in 0..masta {
                let mut prefix = vec![];
                let mut suffix = vec![];
                for v in 0..l { //0, 1, 2, 3, 4, 5
                    prefix.push(0);
                }
                for v in (l+1)..masta{  //5, 4, 3, 2, 1, 0
                    suffix.push(0);
                }
                for v in &[1,2] {
                    let mut result = prefix.clone();
                    result.push(*v);
                    result.extend(&suffix);
                    starting_boards.push(result.clone());
                    accepted_boards.insert(result);
                }
                
            }
        }
        //println!("{:?}",starting_boards);
        accepted_boards.retain(|k| k.len() > old_boards[0].1.len());
        self.rules.all_reverse_from(&starting_boards,&mut accepted_boards);
        let mut useful_prefixes = HashSet::<Vec<SymbolIdx>>::new();
        for (_,board) in &old_boards{
            useful_prefixes.insert(board.clone());
         }
        //accepted_boards.retain(|k| useful_prefixes.contains(&k[0..old_boards[0].1.len()]));
        

        /* 
        for (_,board) in &old_boards{
            for sym_idx in 0..(self.symbol_set.length as SymbolIdx){
                for sig_board in sig_set{
                    let mut test_board = board.clone();
                    test_board.push(sym_idx);
                    test_board.extend(sig_board.clone());
                    let bfs_result = self.bfs_solver(&test_board);
                    let rev_result = accepted_boards.contains(&test_board);
                    if bfs_result != rev_result {
                        println!("BFS {} | REV {}: {:?}", bfs_result, rev_result, test_board);
                        break
                    }
                }
            }
         }*/
        let iter_sig_time = Instant::now();

        for (start_idx,board) in &old_boards {
            //Finds ingoing end of board.
            
            //Gets sig set of all boards with a single symbol added.
            let next_results = self.board_to_next_reverse(&board, sig_set,&accepted_boards);
            for (sym_idx,new_board) in next_results.iter().enumerate() {
                //Checking if the next board's sig set already exists in DFA
                let dest_idx = match table_reference.get(&new_board.0) {
                    //If it does, the arrow's obv going to the existing state in the DFA
                    Some(idx) => {
                        *idx
                    },
                    //If it doesn't, add a new state to the DFA!
                    None => {
                        let new_idx = trans_table.len();
                        new_boards.push((new_idx,new_board.1.clone()));
                        
                        
                        table_reference.insert(new_board.0.clone(),new_idx);
                        trans_table.push(empty_copy.clone());

                        if accepted_boards.contains(&new_board.1) {
                            accepting_states.insert(new_idx);
                        }
                        new_idx
                        }
                    };
                trans_table[*start_idx][sym_idx] = dest_idx;
                }  
                
            }
            println!(" {} Accepting Boards | Board-Gen {} ms | Sig-Set {} ms | Total {} ms",
            accepted_boards.len(),
            (iter_sig_time-iter_begin_time).as_millis(),
            iter_sig_time.elapsed().as_millis(),
            iter_begin_time.elapsed().as_millis()
            );
        }
DFA {
    state_transitions : trans_table,
    accepting_states : accepting_states,
    starting_state : 0,
    symbol_set : self.symbol_set.clone()
}
}
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
        //println!("iter started");
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

} 

fn symbols_to_string(symbols : &Vec<SymbolIdx>) -> String{
    let mut string = "".to_owned();
    for sym in symbols {
        string += &format!("{}", sym);
    }
    string
}

impl PartialEq for DFA {
    fn eq(&self, other: &Self) -> bool {
        let mut stack = vec![(self.starting_state,other.starting_state)];
        let mut visited = HashSet::new();
        if self.symbol_set.length != other.symbol_set.length {
            return false;
        }
        visited.insert((self.starting_state,other.starting_state));
        while let Some(pair) = stack.pop() {
            if(self.accepting_states.contains(&pair.0) != other.accepting_states.contains(&pair.1)) {
                return false;
            }
            for i in 0..self.symbol_set.length {
                let test = (self.state_transitions[pair.0][i],other.state_transitions[pair.1][i]);
                if !visited.contains(&test) {
                    visited.insert(test.clone());
                    stack.push(test);
                }
            }
        }
        true
    }
}

impl DFA {
    fn contains(&self, input : &Vec<SymbolIdx>) -> bool {
        let mut state = self.starting_state;
        for i in input {
            state = self.state_transitions[state][(*i as usize)];
        }
        self.accepting_states.contains(&state)
    }

    fn final_state(&self, input : &Vec<SymbolIdx>) -> usize{
        let mut state = self.starting_state;
        for i in input {
            state = self.state_transitions[state][(*i as usize)];
        }
        state
    }

    fn contains_from_start(&self, input : &Vec<SymbolIdx>, start : usize) -> bool {
        let mut state = start;
        for i in input {
            state = self.state_transitions[state][(*i as usize)];
        }
        self.accepting_states.contains(&state)
    }

    fn save_jflap_to_file(&self,file : &mut File) {
        let mut w = EmitterConfig::new().perform_indent(true).create_writer(file);
        w.write(XmlEvent::start_element("structure")).unwrap();
        w.write(XmlEvent::start_element("type")).unwrap();
        w.write(XmlEvent::characters("fa")).unwrap();
        w.write(XmlEvent::end_element()).unwrap();
        w.write(XmlEvent::start_element("automaton")).unwrap();
        
        for (idx,i) in self.state_transitions.iter().enumerate() {
            w.write(XmlEvent::start_element("state")
                                                                .attr("id",&idx.to_string())
                                                                .attr("name",&("q".to_owned()+&idx.to_string()))
                                                            ).unwrap();
            if idx == self.starting_state {
                w.write(XmlEvent::start_element("initial")).unwrap();
                w.write(XmlEvent::end_element()).unwrap();
            }                                
            if self.accepting_states.contains(&idx) {
                w.write(XmlEvent::start_element("final")).unwrap();
                w.write(XmlEvent::end_element()).unwrap();
            }
            w.write(XmlEvent::end_element()).unwrap();
        }
        let symbols = &self.symbol_set.representations;
        for (idx,state) in self.state_transitions.iter().enumerate() {
            for (idx2,target) in state.iter().enumerate() {
                w.write(XmlEvent::start_element("transition")).unwrap();
                w.write(XmlEvent::start_element("from")).unwrap();
                w.write(XmlEvent::characters(&idx.to_string())).unwrap();
                w.write(XmlEvent::end_element()).unwrap();
                w.write(XmlEvent::start_element("to")).unwrap();
                w.write(XmlEvent::characters(&target.to_string())).unwrap();
                w.write(XmlEvent::end_element()).unwrap();
                w.write(XmlEvent::start_element("read")).unwrap();
                w.write(XmlEvent::characters(&format!("{}",symbols[idx2]))).unwrap();
                w.write(XmlEvent::end_element()).unwrap();
                w.write(XmlEvent::end_element()).unwrap();
            }

        }
        w.write(XmlEvent::end_element()).unwrap();
        w.write(XmlEvent::end_element()).unwrap();
    }

    fn jflap_save(&self, filename : &str) {
        let mut file = File::create(filename.clone().to_owned() + ".jff").unwrap();
        self.save_jflap_to_file(&mut file);
    }
    fn save(&self, filename : &str) {
        let mut file = File::create(filename.clone().to_owned() + ".dfa").unwrap();
        file.write(serde_json::to_string(self).unwrap().as_bytes());
    }
    fn load(filename : &str) -> Result::<Self> {
        let mut contents = fs::read_to_string(filename.clone().to_owned() + ".dfa").unwrap();
        serde_json::from_str(&contents)
    }
}


fn build_threerulesolver() -> SRSTranslator {
    let b_symbol_set = SymbolSet {
        length : 3,
        representations : vec!["0".to_owned(),"1".to_owned(),"2".to_owned()]
    };
    let ruleset = Ruleset::new(
        vec![(vec![1,1,0],vec![0,0,1]),
                     (vec![0,1,1],vec![1,0,0]),
                     (vec![1,0,1],vec![0,1,0]),
                     (vec![2,1,0],vec![0,0,2]),
                     (vec![0,1,2],vec![2,0,0]),
                     (vec![2,0,1],vec![0,2,0]),
                     (vec![1,0,2],vec![0,2,0]),
                     //(Vec::from(vec![Binary::One,Binary::Zero,Binary::One]),Vec::from(vec![Binary::Zero,Binary::One,Binary::Zero])),
        ],
        b_symbol_set.clone()
    );
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,2,1],vec![1,2,2],vec![2,2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        symbol_set : b_symbol_set.clone()
    };
    SRSTranslator::new(ruleset,goal_dfa)
}
fn build_defaultsolver() -> SRSTranslator {
    let b_symbol_set = SymbolSet {
        length : 3,
        representations : vec!["0".to_owned(),"1".to_owned(),"2".to_owned()]
    };
    let ruleset = Ruleset::new(
        vec![(vec![1,1,0],vec![0,0,1]),
                     (vec![0,1,1],vec![1,0,0]),
                     (vec![2,1,0],vec![0,0,2]),
                     (vec![0,1,2],vec![2,0,0]),
                     //(Vec::from(vec![Binary::One,Binary::Zero,Binary::One]),Vec::from(vec![Binary::Zero,Binary::One,Binary::Zero])),
        ],
        b_symbol_set.clone()
    );
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,2,1],vec![1,2,2],vec![2,2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        symbol_set : b_symbol_set.clone()
    };
    SRSTranslator::new(ruleset,goal_dfa)
}

fn build_2xnswap() -> SRSTranslator {
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

    let ruleset = Ruleset::new(
        rules,
        symbol_set.clone()
    );

    let old_dfa = DFA::load("default1dpeg").unwrap();
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
     SRSTranslator::new(ruleset,goal_dfa)
}

fn build_default1dpeg() -> SRSTranslator {
    let b_symbol_set = SymbolSet {
        length : 2,
        representations : vec!["0".to_owned(),"1".to_owned()]
    };
    let ruleset = Ruleset::new(
        vec![(vec![1,1,0],vec![0,0,1]),
                     (vec![0,1,1],vec![1,0,0]),
                     //(vec![1,0,1],vec![0,1,0])
                     //(Vec::from(vec![Binary::One,Binary::Zero,Binary::One]),Vec::from(vec![Binary::Zero,Binary::One,Binary::Zero])),
        ],
        b_symbol_set.clone()
    );
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1],vec![1,2],vec![2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        symbol_set : b_symbol_set.clone()
    };
    SRSTranslator::new(ruleset,goal_dfa)
}

fn build_threerule1dpeg() -> SRSTranslator {
    let b_symbol_set = SymbolSet {
        length : 2,
        representations : vec!["0".to_owned(),"1".to_owned()]
    };
    let ruleset = Ruleset::new(
        vec![(vec![1,1,0],vec![0,0,1]),
                     (vec![0,1,1],vec![1,0,0]),
                     (vec![1,0,1],vec![0,1,0])
                     //(Vec::from(vec![Binary::One,Binary::Zero,Binary::One]),Vec::from(vec![Binary::Zero,Binary::One,Binary::Zero])),
        ],
        b_symbol_set.clone()
    );
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1],vec![1,2],vec![2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        symbol_set : b_symbol_set.clone()
    };
    SRSTranslator::new(ruleset,goal_dfa)
}

fn build_flip() -> SRSTranslator {
    let b_symbol_set = SymbolSet {
        length : 2,
        representations : vec!["0".to_owned(),"1".to_owned()]
    };
    let mut rules_vec = vec![];
    for i in 0..8 {
        rules_vec.push((vec![i/4 % 2, i / 2 % 2, i % 2],vec![1-i/4 % 2, 1-i / 2 % 2, 1-i % 2]))
    }
    let ruleset = Ruleset::new(
        rules_vec,
        b_symbol_set.clone()
    );
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1],vec![1,1]],
        accepting_states : HashSet::from_iter(vec![0]),
        symbol_set : b_symbol_set.clone()
    };
    SRSTranslator::new(ruleset,goal_dfa)
}

fn build_flipx3() -> SRSTranslator {
    let b_symbol_set = SymbolSet {
        length : 2,
        representations : vec!["0".to_owned(),"1".to_owned()]
    };
    let mut rules_vec = vec![];
    for i in 0..8 {
        rules_vec.push((vec![i/4 % 2, i / 2 % 2, i % 2],vec![1-i/4 % 2, 1-i / 2 % 2, 1-i % 2]))
    }
    let ruleset = Ruleset::new(
        rules_vec,
        b_symbol_set.clone()
    );
    
    let k = 3;
    let symbol_num = 2_u32.pow(k as u32) as usize;
    let mut new_rules = vec![];
    let mut vert_starts = vec![];
    let mut vert_ends = vec![];
    for rule in ruleset.rules {
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
                    /* 
                    println!("god i am dumb");
                    //If we're not, go through each rule
                    for start_idx in 0..cur_vert_rules {
                        //For each non-zero character
                        
                        //Don't need to add 0 index to our options bc it's assumed!
                        /* 
                        for new_sym in 1..b_symbol_set.length {
                            let mut new_vert_start = vert_starts[start_idx].clone();
                            let mut new_vert_end = vert_ends[start_idx].clone();
                            //This is the issue!!!!
                            for vert_idx in 0..vert_starts[start_idx].len() {
                                new_vert_start[vert_idx] += new_sym*pow_num;
                            }
                            for vert_idx in 0..vert_ends[start_idx].len() {
                                new_vert_end[vert_idx] += new_sym*pow_num;
                            }
                            vert_starts.push(new_vert_start);
                            vert_ends.push(new_vert_end);
                        }
                        */
                        for new_sym in 1..b_symbol_set.length {
                            let mut new_vert_start = vert_starts[start_idx].clone();
                            let mut new_vert_end = vert_ends[start_idx].clone();
                            //This is the issue!!!!
                            new_vert_start[j] += new_sym*pow_num;
                            new_vert_end[j] += new_sym*pow_num;
                            vert_starts.push(new_vert_start);
                            vert_ends.push(new_vert_end);
                        }
                    }
                    */
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
    
    let ruleset = Ruleset::new(new_rules,by_k_symbol_set.clone());
    
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1,1,1,1,1,1,1],vec![1,1,1,1,1,1,1,1]],
        accepting_states : HashSet::from_iter(vec![0]),
        symbol_set : by_k_symbol_set.clone()
    };

    
    /* 
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1,1,1,1,1,1,1],vec![1,1,1,1,1,1,1,1]],
        accepting_states : HashSet::from_iter(vec![0]),
        symbol_set : by_k_symbol_set.clone()
    };
    */
    
    SRSTranslator::new(ruleset,goal_dfa)
}

fn build_default2dpegx3 () -> SRSTranslator {
    let b_symbol_set = SymbolSet {
        length : 2,
        representations : vec!["0".to_owned(),"1".to_owned()]
    };

    let ruleset = Ruleset::new(
        vec![(vec![1,1,0],vec![0,0,1]),
                     (vec![0,1,1],vec![1,0,0]),
                     //(vec![1,0,1],vec![0,1,0]),
                     //(vec![0,1,2],vec![2,0,0]),
                     //(vec![2,1,0],vec![0,0,2]),
                     //(vec![2,0,1],vec![0,2,0]),
                     //(vec![1,0,2],vec![0,2,0]),
                     //(vec![1,0,1],vec![0,1,0])
                     //(Vec::from(vec![Binary::One,Binary::Zero,Binary::One]),Vec::from(vec![Binary::Zero,Binary::One,Binary::Zero])),
        ],
        b_symbol_set.clone()
    );
    
    let k = 3;
    let symbol_num = 2_u32.pow(k as u32) as usize;
    let mut new_rules = vec![];
    let mut vert_starts = vec![];
    let mut vert_ends = vec![];
    for rule in ruleset.rules {
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
                    /* 
                    println!("god i am dumb");
                    //If we're not, go through each rule
                    for start_idx in 0..cur_vert_rules {
                        //For each non-zero character
                        
                        //Don't need to add 0 index to our options bc it's assumed!
                        /* 
                        for new_sym in 1..b_symbol_set.length {
                            let mut new_vert_start = vert_starts[start_idx].clone();
                            let mut new_vert_end = vert_ends[start_idx].clone();
                            //This is the issue!!!!
                            for vert_idx in 0..vert_starts[start_idx].len() {
                                new_vert_start[vert_idx] += new_sym*pow_num;
                            }
                            for vert_idx in 0..vert_ends[start_idx].len() {
                                new_vert_end[vert_idx] += new_sym*pow_num;
                            }
                            vert_starts.push(new_vert_start);
                            vert_ends.push(new_vert_end);
                        }
                        */
                        for new_sym in 1..b_symbol_set.length {
                            let mut new_vert_start = vert_starts[start_idx].clone();
                            let mut new_vert_end = vert_ends[start_idx].clone();
                            //This is the issue!!!!
                            new_vert_start[j] += new_sym*pow_num;
                            new_vert_end[j] += new_sym*pow_num;
                            vert_starts.push(new_vert_start);
                            vert_ends.push(new_vert_end);
                        }
                    }
                    */
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
    
    let ruleset = Ruleset::new(new_rules,by_k_symbol_set.clone());
    
    /*  let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1,1,2,1,2,2,2],vec![1,2,2,2,2,2,2,2],vec![2,2,2,2,2,2,2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        symbol_set : by_k_symbol_set.clone()
    };

    */
    let root_dfa = DFA::load("default1dpeg").unwrap();

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
    goal_dfa.jflap_save("experimental3xk");
    
    
    SRSTranslator::new(ruleset,goal_dfa)
}

//Testing subset method :((

/* 
fn main() {
    println!("default 1d");
    let mut translator = build_default1dpeg();
    assert!(translator.dfa_with_sig_set_subset(5) == DFA::load("default1dpeg").unwrap(), "Default 1d failed");
    println!("three rule 1d");
    let mut translator = build_threerule1dpeg();
    assert!(translator.dfa_with_sig_set_subset(5) == DFA::load("threerule1dpeg").unwrap(), "three rule 1d failed");
    println!("default solver");
    let mut translator: SRSTranslator = build_defaultsolver();
    assert!(translator.dfa_with_sig_set_subset(5) == DFA::load("defaultsolver").unwrap(), "Default solver failed");
    println!("three rule solver");
    let mut translator = build_threerulesolver();
    assert!(translator.dfa_with_sig_set_subset(5) == DFA::load("threerulesolver").unwrap(), "Three rule solver failed");
    println!("flip");
    let mut translator: SRSTranslator = build_flip();
    assert!(translator.dfa_with_sig_set_subset(5) == DFA::load("flip").unwrap(), "Flip failed");
    println!("flipx3");
    let mut translator: SRSTranslator = build_flipx3();
    assert!(translator.dfa_with_sig_set_subset(5) == DFA::load("flipx3").unwrap(), "Flipx3 failed");
} 

*/
//Building 2xn n swap


fn main() {
    let mut translator = build_2xnswap();
    let mut k = 11;
    println!("K: {}",k);
    let mut possible_dfa = translator.dfa_with_sig_set_ancestor(k);
    //let dfa = DFA::load("2xnswap").unwrap();
    //possible_dfa.save("3xkdefault");
    k += 1;
    println!("K: {}",k);
    let mut new_possible_dfa = translator.dfa_with_sig_set_ancestor(k);
    //new_possible_dfa.save("3xkdefault");
    while possible_dfa != new_possible_dfa {
        //possible_dfa = new_possible_dfa;
        k += 1;
        println!("K: {}",k);
        new_possible_dfa = translator.dfa_with_sig_set_ancestor(k);
        //new_possible_dfa.save("3xkdefault");
    }
    
    //translator.random_tests(dfa, 8,10000);
    
}

//Summary information about 2xn swap DFA
/* 
fn main () {
    let swap_dfa = DFA::load("2xnswap").unwrap();
    println!("{} States", swap_dfa.state_transitions.len());
    let mut visited = HashSet::new();
    visited.insert(0);
    let mut recent = vec![0];
    let mut second_recent = vec![];
    let mut diameter = 0;
    while recent.len() > 0 {
        diameter += 1;
        std::mem::swap(&mut second_recent, &mut recent);
        recent.clear();
        for state in &second_recent {
            for new_state in &swap_dfa.state_transitions[*state] {
                if visited.insert(*new_state) {
                    recent.push(*new_state);
                }
            }
        }
    }
    diameter -= 1;
    println!("Diameter of {}", diameter);
} */

/* 
fn main() {
    let mut translator = build_2xnswap();
    let swap_dfa = DFA::load("2xnswap").unwrap();
    translator.verify_to_len(swap_dfa, 16);
    //translator.random_tests(swap_dfa, 32, 100);
}
*/
// Analysis of state swap on single SRS move

/* 
fn main() {
    let mut translator = build_threerulesolver();
    let created_dfa = DFA::load("threerulesolver").unwrap();
    //created_dfa.jflap_save("default1dpeg");

    let mut possible_graph : HashSet<(usize,usize)>= HashSet::new();
    let mut possible_check = vec![false;created_dfa.state_transitions.len()];
    let mut possible_first = vec![vec![];created_dfa.state_transitions.len()];

    let mut possible_always : Vec<HashSet::<usize>>= vec![HashSet::new();created_dfa.state_transitions.len()];
    let mut possible_ever : Vec<HashSet::<usize>>= vec![HashSet::new();created_dfa.state_transitions.len()];

    for string in translator.build_sig_k(12){
        let result_state = created_dfa.final_state(&string);
        let mut result_options = HashSet::new();
        for option in translator.rules.single_rule_hash(&string) {
            result_options.insert(created_dfa.final_state(&option));
            if possible_graph.insert((result_state,created_dfa.final_state(&option)))
                && possible_check[result_state] {
                    println!("failed on {:?} with old string {:?} result state {} and new state {}", 
                    string,
                    possible_first[result_state],
                    result_state,
                    created_dfa.final_state(&option)
                );
            } 
        }
        if !possible_check[result_state] {
            possible_check[result_state] = true;
            possible_first[result_state] = string;
            possible_always[result_state] = result_options.clone();
            possible_ever[result_state] = result_options;
        } else {

            let mut final_set = HashSet::new();
            for x in result_options.intersection(&possible_always[result_state]) {
                final_set.insert(*x);
            }
            possible_always[result_state] = final_set;
            let mut final_set = HashSet::new();
            for x in result_options.union(&possible_ever[result_state]) {
                final_set.insert(*x);
            }
            //possible_ever[result_state] = final_set;
        }

    }
    let mut too_high_for_typecasting = vec![];
    for i in possible_graph{
        too_high_for_typecasting.push(i);
    }
    //let g = UnGraph::<usize, ()>::from_edges(&too_high_for_typecasting);
    println!("{}",created_dfa.state_transitions.len());
    println!("{:?}",possible_ever);
}*/


// Principled graph creation (as a warmup to the new method)
/* 
fn main() {
    let mut translator = build_threerulesolver();
    let created_dfa = DFA::load("threerulesolver").unwrap();
    //created_dfa.jflap_save("default1dpeg");

    let mut edges : HashSet::<(usize,usize)> = HashSet::new();
    for origin in 0..created_dfa.state_transitions.len() {
        for rule in &translator.rules.rules {
            let mut parent = origin;
            let mut child = origin;
            for i in 0..rule.0.len() {
                parent = created_dfa.state_transitions[parent][rule.0[i] as usize];
                child = created_dfa.state_transitions[child][rule.1[i] as usize];
            }
            edges.insert((parent,child));
        }  
    }
    let mut old_edges = HashSet::new();
    let mut new_edges = edges.clone();

    while new_edges.len() > 0 {
        std::mem::swap(&mut old_edges, &mut new_edges);
        old_edges.clear();
        for edge in &old_edges {
            for sym in 0..translator.symbol_set.length {
                let new_parent = created_dfa.state_transitions[edge.0][sym as usize];
                let new_child = created_dfa.state_transitions[edge.1][sym as usize];
                if edges.insert((new_parent,new_child)) {
                    new_edges.insert((new_parent,new_child));
                }
            }
        }
    }

    let g = DiGraph::<usize, (), usize>::from_edges(edges);
    let mut file = File::create("fungraph.dot").unwrap();
    let output = format!("{:?}",Dot::with_config(&g,&[Config::EdgeNoLabel]));
    file.write(output.as_bytes());
    println!("{}",created_dfa.state_transitions.len());

}*/
/* 
fn main() {

    /*
    let c_ruleset = Ruleset::<Collatz> {
        min_input : 2,
        max_input : 2,
        rules : vec![
            //Carry & start
            (Vec::from(vec![Collatz::Start,Collatz::Carry2]),Vec::from(vec![Collatz::Start,Collatz::One,Collatz::Zero])),
            (Vec::from(vec![Collatz::Start,Collatz::Carry1]),Vec::from(vec![Collatz::Start,Collatz::One])),
            (Vec::from(vec![Collatz::Start,Collatz::Carry0]),Vec::from(vec![Collatz::Start,Collatz::Zero])),   
            (Vec::from(vec![Collatz::Start,Collatz::Zero]),Vec::from(vec![Collatz::Zero,Collatz::Start])),   

            //Carry & next
            (Vec::from(vec![Collatz::Zero,Collatz::Carry0]),Vec::from(vec![Collatz::Carry0,Collatz::Zero])),
            (Vec::from(vec![Collatz::One,Collatz::Carry0]),Vec::from(vec![Collatz::Carry1,Collatz::One])),
            (Vec::from(vec![Collatz::Zero,Collatz::Carry1]),Vec::from(vec![Collatz::Carry0,Collatz::One])),
            (Vec::from(vec![Collatz::One,Collatz::Carry1]),Vec::from(vec![Collatz::Carry2,Collatz::Zero])),
            (Vec::from(vec![Collatz::Zero,Collatz::Carry2]),Vec::from(vec![Collatz::Carry1,Collatz::Zero])),
            (Vec::from(vec![Collatz::One,Collatz::Carry2]),Vec::from(vec![Collatz::Carry2,Collatz::One])),

            //Finish
            (Vec::from(vec![Collatz::Zero,Collatz::Finish]),Vec::from(vec![Collatz::Finish])),
            (Vec::from(vec![Collatz::One,Collatz::Finish]),Vec::from(vec![Collatz::Carry2,Collatz::Finish])),
            ]
    };
    let c_goal_dfa = DFA::<Collatz> {
        symbols : all::<Collatz>().collect(),
        starting_state : 0,
        state_transitions : vec![vec![4,4,4,0,4,1,4],vec![4,4,4,4,2,4,4],vec![4,4,4,4,4,4,3],vec![4,4,4,3,4,4,4],vec![4,4,4,4,4,4,4]],
        accepting_states : HashSet::from_iter(vec![2,3])
        
    };
    let mut c_translator = SRSTranslator {
        rules : c_ruleset,
        goal : c_goal_dfa,
        board_solutions : HashMap::new()
    };
    
    c_translator.bfs_solver(&vec![Collatz::Start,Collatz::One,Collatz::Zero,Collatz::Finish]);
    let c_dfa = c_translator.dfa_with_sig_set(&SRSTranslator::<Collatz>::build_sig_k(7));
    c_translator.verify_to_len(c_dfa,12); */
    //c_dfa.save("collatz");
    /* 
    
    let possible_dfa = translator.dfa_with_sig_set(&translator.build_sig_k(5));
    possible_dfa.save("default1dpeg");
    */


    //Three-rule
    
    
    
   /*
    println!("This atrocity has {} rules", translator.rules.rules.len());
    //translator.verify_to_len(DFA::load("default2dpegx3").unwrap(), 8);
    let possible_dfa = translator.dfa_with_sig_set(&translator.build_sig_k(5));
    possible_dfa.save("flip");
    
    */
        let b_symbol_set = SymbolSet::new(vec!["0".to_owned(),"1".to_owned()]);
    
    
    for (start, end) in &new_rules {
        println!("{:?} -> {:?}",start,end);
    }
    
    
    println!("This atrocity has {} rules", translator.rules.rules.len());
    //translator.verify_to_len(DFA::load("default2dpegx3").unwrap(), 8);
    let possible_dfa = translator.dfa_with_sig_set_subset(5);
    
    //possible_dfa.save("flipx3"); 
    //possible_dfa.jflap_save("flipx3");
    //let possible_dfa = DFA::load("flipx3").unwrap();
    //translator.verify_to_len(possible_dfa,8);

    //println!("{:?}",new_rules);
    
    
    /* 
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1,1],vec![1,2,2],vec![2,2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        symbol_set : b_symbol_set.clone()
    };
    let mut translator = SRSTranslator {
        rules : ruleset,
        goal : goal_dfa,
        board_solutions : HashMap::new(),
        symbol_set : b_symbol_set
    };

    //println!("{:?}", translator.dfs_pather(&vec![Binary::Zero,Binary::One,Binary::Zero,Binary::One,Binary::One]).unwrap());
    //return;

    let possible_dfa = translator.dfa_with_sig_set(&translator.build_sig_k(5));
    */
    //translator.verify_to_len(possible_dfa, 20);
    //possible_dfa.save("threerule1dpeg");
    /* 
    let k = 0;

    for k in 0..20 {
        let graph = SRS_groups(&mut translator, k);
        println!("{} roots at length {}",graph.externals(petgraph::Direction::Incoming).count(),k);
        println!("{} leaves at length {}",graph.externals(petgraph::Direction::Outgoing).count(),k);
    }*/
    
    
    //fs::write("output.dot", dot_parser(format!("{:?}",Dot::new(&graph)))).expect("Unable to write file");
    /* 
    let ruleset = Ruleset::<Ternary> {
        min_input : 3,
        max_input : 3,
        rules : vec![(Vec::from(vec![Ternary::One,Ternary::One,Ternary::Zero]),Vec::from(vec![Ternary::Zero,Ternary::Zero,Ternary::One])),
                     (Vec::from(vec![Ternary::Zero,Ternary::One,Ternary::One]),Vec::from(vec![Ternary::One,Ternary::Zero,Ternary::Zero])),
                     (Vec::from(vec![Ternary::One,Ternary::Zero,Ternary::One]),Vec::from(vec![Ternary::Zero,Ternary::One,Ternary::Zero])),

                     //special rules
                     (Vec::from(vec![Ternary::Two,Ternary::One,Ternary::Zero]),Vec::from(vec![Ternary::Zero,Ternary::Zero,Ternary::Two])),
                     (Vec::from(vec![Ternary::Zero,Ternary::One,Ternary::Two]),Vec::from(vec![Ternary::Two,Ternary::Zero,Ternary::Zero])),
                     (Vec::from(vec![Ternary::Two,Ternary::Zero,Ternary::One]),Vec::from(vec![Ternary::Zero,Ternary::Two,Ternary::Zero])),
                     (Vec::from(vec![Ternary::One,Ternary::Zero,Ternary::Two]),Vec::from(vec![Ternary::Zero,Ternary::Two,Ternary::Zero])),
        
        ]
    };
    let goal_dfa = DFA::<Ternary> {
        starting_state : 0,
        state_transitions : vec![vec![0,2,1],vec![1,2,2],vec![2,2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        input_type : PhantomData
    };
    let mut translator = SRSTranslator {
        rules : ruleset,
        goal : goal_dfa,
        board_solutions : HashMap::new()
    };
    let which_dfa = translator.dfa_with_sig_set(&SRSTranslator::<Ternary>::build_sig_k(5));
    which_dfa.save("threerulesolver");
*/
    /* 
    let board_to_solve = vec![Ternary::Zero,Ternary::One,Ternary::One,Ternary::One,Ternary::One,Ternary::One,Ternary::One,Ternary::One,Ternary::One,Ternary::Zero,Ternary::One,Ternary::Zero];

    if possible_dfa.contains(&board_to_solve) {
        for idx in 0..board_to_solve.len() {
            if board_to_solve[idx] == Ternary::Zero {
                continue
            }
            let mut solve_dupe = board_to_solve.clone();
            solve_dupe[idx] = Ternary::Two;
            if which_dfa.contains(&solve_dupe) {
                println!("{}",symbols_to_string(&solve_dupe));
                //Finding a path
                while !translator.goal.contains(&solve_dupe) {
                    for new_option in translator.rules.single_rule(&solve_dupe) {
                        if which_dfa.contains(&new_option) {
                            solve_dupe = new_option;
                            println!("{}",symbols_to_string(&solve_dupe));
                            break;
                        }
                    }
                }
                break;
            }
        }

    } else {
        println!("not solvable!");
    }
    
*/
    //let final_dfa = translator.dfa_with_sig_set(&SRSTranslator::<Binary>::build_sig_k(5));
    //translator.verify_to_len(final_dfa,25);
    //final_dfa.save("1dpeg");
    //let groups = exhaustive_group_builder();
    //prefix_test(&groups);

    
    //println!("done");
    //signature_element_groups(&groups);

    

    //println!("{:?}",*SIGNATURE_ELEMENTS);
    //println!("{}",group_to_string(&which_prefixes_solvable(&vec![false,false,false,true,true])))
    //bfs_solver(&vec![true,true,true,false,true,true,true,true]);
    //let groups = fast_group_builder(); 
    //let groups = exhaustive_group_builder();
    //identical_signature_elements(&groups);
    //signature_element_groups(&groups);//29 meta-groups for 1dpeg threerule
    //prefix_test(&groups);
    //let dfa = dfa_builder();
    //dfa.save("threerule1dpeg");
    //println!("{} states",dfa.state_transitions.len());
    
    /*let mut test_board = Vec::<bool>::new();
    let str = "01111011110110110".to_owned();
    for i in str.chars() {
        if i == '1' {
            test_board.push(true);
        } else {
            test_board.push(false);
        }
    }
    println!("ravi board: {}", dfa.contains(&test_board));*/
    //dfa.verify_all_to_len(16); 
    //group_solvability(&groups);
    //let groups = fast_group_builder(); 
    //let graph = prefix_graph(&groups);
    
    /*let p2n = Ruleset::<(Need,EF),Need> {
        name : "Puzzle To Needs".to_owned(),
        rules : Vec::new()
    };
    for group in EF::iter() {

    }
    println!("{}",p2n);*/
} */