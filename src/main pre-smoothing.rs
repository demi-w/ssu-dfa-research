#![allow(warnings)] 
use std::hash::Hash;
use std::fmt::{self, write, format};
use automata::dfa::Node;
use crossbeam::queue::{SegQueue, ArrayQueue};
use petgraph::graph::NodeIndex;
use petgraph::dot::{Dot, Config};
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

use serde_json::Result;

use xml::writer::{EventWriter, EmitterConfig, XmlEvent, Result as XmlResult};
use std::marker::PhantomData;
use std::time::Instant;


use serde::{Deserialize, Serialize};
use std::thread;
#[macro_use]
extern crate lazy_static;

const WORKERS : usize = 32;

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
struct SRSTranslator{
    rules : Ruleset,
    goal : DFA,
    board_solutions : HashMap<Vec<SymbolIdx>,bool>,
    symbol_set : SymbolSet,
}

impl SRSTranslator {
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
        for i in 0..5 {
            starting_boards.push(vec![0;old_boards[0].1.len()+i]);
        }
        /* 
        for masta in (old_boards[0].1.len()+1)..(old_boards[0].1.len()+6) {
            for l in 0..masta {
                let mut prefix = vec![];
                let mut suffix = vec![];
                for v in 0..l { //0, 1, 2, 3, 4, 5
                    prefix.push(0);
                }
                for v in (l+1)..masta{  //5, 4, 3, 2, 1, 0
                    suffix.push(0);
                }
                for v in &[1,2,4] {
                    let mut result = prefix.clone();
                    result.push(*v);
                    result.extend(&suffix);
                    starting_boards.push(result);
                }
                
            }
        }*/
        //println!("{:?}",starting_boards);
        accepted_boards.retain(|k| k.len() >= old_boards[0].1.len());
        self.rules.all_reverse_from(&starting_boards,&mut accepted_boards);
        
        let mut useful_prefixes = HashSet::<Vec<SymbolIdx>>::new();
        for (_,board) in &old_boards{
            useful_prefixes.insert(board.clone());
        }
        accepted_boards.retain(|k| useful_prefixes.contains(&k[0..old_boards[0].1.len()]));
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
fn verify_to_len(&mut self,test_dfa : DFA, n:usize){
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
                    return;
                }
            }
            None => {std::thread::sleep(time::Duration::from_millis(100));}
            }
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
impl DFA {
    fn contains(&self, input : &Vec<SymbolIdx>) -> bool {
        let mut state = self.starting_state;
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
    //get this back up (why not ya know)
}
/* 
#[derive(Debug,EnumIter,Clone,Copy)]
enum EF {
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven
}

enum Requirement {
    Empty,
    Full,
    Irrelevant
}

fn single_group_transform(value : EF) -> Vec<EF> {
    match value {
        EF::Six => vec![EF::Six,EF::One],
        EF::Three => vec![EF::Three,EF::Four],
        _ => vec![value]
    }
}

//Accounts for single-group transformation
fn probe(value : EF, query : [Requirement; 3]) -> bool {
    let mut f_result = false;
    
    for val in single_group_transform(value) {
        let mut intval = val as u8;
        let mut result = true;
        for i in 0..3 {
            result &= match &query[i] {
                Requirement::Empty => intval%2==0,
                Requirement::Full => intval%2==1,
                Requirement::Irrelevant => true
            };
            intval /= 2;
        }
        f_result |= result;
    }
    f_result
}

struct Ruleset<In,Out> where In : fmt::Debug, Out : fmt::Debug {
    rules : Vec<(In,Out)>,
    name : String
}

impl<In,Out> fmt::Display for Ruleset<In,Out> where In : fmt::Debug, Out : fmt::Debug{
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        write!(f, "Ruleset: {}", self.name);
        for rule in &self.rules {
            writeln!(f, "{0:?} -> {1:?}", rule.0, rule.1);
        }
        Ok(())
    }
}*/

fn bfs_solver(starting_board : &Vec<bool>) -> bool{
    let mut cur_boards = Vec::new();
    let mut next_boards = vec![starting_board.clone()];
    let mut known_states = HashSet::<Vec<bool>>::new();
    let mut ones = 0;
    known_states.insert(starting_board.clone());
    return match starting_board.len() >= 3{
        true => 
        {
        while next_boards.len() > 0 {
            std::mem::swap(&mut cur_boards,&mut next_boards);
            next_boards = Vec::new();
            for board in &cur_boards{
                let mut index = 0;
                ones = 0;
                let mut onebehind = board[0];
                if onebehind {ones += 1}
                let mut notbehind = board[1];
                if notbehind {ones += 1}
                while index+2 < board.len(){
                    let twobehind = onebehind;
                    onebehind = notbehind;
                    notbehind = board[index+2];
                    if notbehind {ones += 1}
                    
                    if twobehind && onebehind && !notbehind {
                        let mut new_board = board.clone();
                        new_board[index] = false;
                        new_board[index+1] = false;
                        new_board[index+2] = true;
                        if !known_states.contains(&new_board){
                            next_boards.push(new_board);
                        }
                    } else if !twobehind && onebehind && notbehind {
                        let mut new_board = board.clone();
                        new_board[index] = true;
                        new_board[index+1] = false;
                        new_board[index+2] = false;
                        if !known_states.contains(&new_board){
                            next_boards.push(new_board);
                        }
                    } 
                    /* 
                    else if twobehind && !onebehind && notbehind {
                        let mut new_board = board.clone();
                        new_board[index] = false;
                        new_board[index+1] = true;
                        new_board[index+2] = false;
                        if !known_states.contains(&new_board){
                            next_boards.push(new_board);
                        }
                    }*/
                    /*
                    let mut new_board = board.clone();
                    new_board[index] = !twobehind;
                    new_board[index+1] = !onebehind;
                    new_board[index+2] = !notbehind;
                    if !known_states.contains(&new_board) {
                        next_boards.push(new_board.clone());
                        known_states.insert(new_board);
                    } */
                    
                    index+=1;
                }
                if ones == 1{
                    return true;
                }
            }
        }
        false 
        },
        
        false =>
        {
        for i in &next_boards[0] {
            if *i {
                ones += 1;
            }
        }
        ones == 1
        }
    }
}

fn which_prefixes_solvable(board : &Vec<bool>) -> [bool; SIGNATURE_LENGTH] {
    let mut results = [false;SIGNATURE_LENGTH];
    for (i,addition) in SIGNATURE_ELEMENTS.iter().enumerate() {
        let mut new_board = board.clone();
        new_board.extend(addition);
        results[i] = bfs_solver(&new_board);
        //println!("{},{}",board_to_string(&new_board),results[i]);
    }
    results
}

fn board_to_string(board : &Vec<bool>) -> String{
    let mut str = String::new();
    for i in board {
        match i {
            true => str.push('█'),
            false => str.push('░')
        }
    }
    str
}
 
fn group_to_string(group : &[bool; SIGNATURE_LENGTH]) -> String{
    let mut str = "(".to_owned();
    let mut matches = 0;
    for (i,sig_element) in SIGNATURE_ELEMENTS.iter().enumerate(){
        if group[i] {
            if matches >= 1 {
                str.push(',');
            }
            matches +=1;
            str = str + &board_to_string(&sig_element);
            
        }
    }
    str.push(')');
    str
}

/* 
fn group_to_string(group : &[bool; SIGNATURE_LENGTH]) -> String {
    "Too big to print".to_owned()
}*/

fn group_builder_consumer(
    input_q : Arc<ArrayQueue<Vec<bool>>>, 
    output_q :  Arc<ArrayQueue<(([bool;SIGNATURE_LENGTH],Vec<bool>),([bool;SIGNATURE_LENGTH],Vec<bool>))>>
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while let Some(mut new_board) = input_q.pop() {
                //let mut new_board : Vec<bool> = new_board_ref.clone();

                output_q.push(board_to_next(new_board)).unwrap();
            }
        })
    }
fn board_solvability_consumer(
    input_q : Arc<ArrayQueue<Vec<bool>>>, 
    output_q :  Arc<ArrayQueue<(bool,Vec<bool>)>>
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while let Some(mut new_board) = input_q.pop() {
                //let mut new_board : Vec<bool> = new_board_ref.clone();

                output_q.push((bfs_solver(&new_board),new_board)).unwrap();
            }
        })
    }

fn signature_append_consumer(
    signature_set : &Vec<SignatureElement>,
    input_q : Arc<ArrayQueue<(usize,SignatureElement)>>,
    output_q : Arc<ArrayQueue<(usize,SignatureElement)>>
    ) -> thread::JoinHandle<()> {
        let my_signature_set = signature_set.clone();
        thread::spawn(move || {
            while let Some((idx, mut sig_element)) = input_q.pop() {
                for j in &my_signature_set {
                    let mut combined_board = sig_element.board.clone();
                    combined_board.append(&mut j.board.clone());
                    sig_element.signature.push(bfs_solver(&combined_board));
                }
                output_q.push((idx,sig_element)).unwrap();
            }
        })
    }

fn board_to_next(mut board : Vec<bool>) -> (([bool;SIGNATURE_LENGTH],Vec<bool>),([bool;SIGNATURE_LENGTH],Vec<bool>)) {
    board.push(false);
    let empty_board = board.clone();
    let empty_add = which_prefixes_solvable(&board);
    *board.last_mut().expect("impossible") = true;
    let full_add = which_prefixes_solvable(&board);
    ((empty_add,empty_board),(full_add,board))
}

fn smart_group_builder() -> HashMap::<Vec<bool>,Vec<SignatureElement>>
{
    const word_size : usize = 3;
    //This needs to flip old and new signature pieces but doesn't yet lol (thankflly we're working with symetrical systems)
    const start_sig_len : usize = (2 << word_size)-1;
    let mut start_index = 0;
    let mut signature_set : Vec<SignatureElement> =  vec![SignatureElement{board:vec![], signature:vec![]}];
    let mut end_index = 1;
    let mut new_index = 1;
    for _ in 0..3 {
        for i in start_index..end_index{
            signature_set.push(SignatureElement {
                board : signature_set[i].board.clone(),
                signature : Vec::new()
            }
            );
            signature_set[new_index].board.push(false);
            new_index += 1;
            signature_set.push(SignatureElement {
                board : signature_set[i].board.clone(),
                signature : Vec::new()
            }
            );
            signature_set[new_index].board.push(true);
            new_index += 1;
        }
        start_index = end_index;
        end_index = new_index;
    }
    let mut iter_count = 0;
    let mut last_signature_set = signature_set.clone();
    let silly_rust = signature_set.clone();
    signature_set = append_solvability(signature_set, &silly_rust);
    let start_set = signature_set.clone();
    // building 1st-order knowledge base
    let mut board_groups = boards_to_groups(&signature_set);
    //let mut new_board_groups = boards_to_groups(&signature_set,&signature_set);
    let mut new_count = 1;
    while new_count > 0 {
        new_count = 0;
        //finding 2nd-order additions
        let mut combo_boards = Vec::new();
        
        for i in &last_signature_set {
            //let mut solvability : Vec<bool> = Vec::new();
            for j in &start_set {
                //anything less is redundant 
                //
                if i.board.len() + j.board.len() > (word_size*(iter_count+1)) {
                    let mut combined_board = i.board.clone();
                    combined_board.append(&mut j.board.clone());
                    combo_boards.push(SignatureElement { 
                        board : combined_board,
                        signature : Vec::new()
                    });
                }
                //solvability.push(bfs_solver(&combined_board));
            }
            /* 
            //The actual proof (very slow)
            for j in &signature_set {
                //anything less is redundant 
                //
                if i.board.len() + j.board.len() > (word_size << (iter_count)) {
                    let mut combined_board = i.board.clone();
                    combined_board.append(&mut j.board.clone());
                    combo_boards.push(SignatureElement { 
                        board : combined_board,
                        signature : Vec::new()
                    });
                }
                //solvability.push(bfs_solver(&combined_board));
            }*/
        }
        println!("{} combo boards to build signatures for, {} board solutions necessary",combo_boards.len(),combo_boards.len()*signature_set.len());
        combo_boards = append_solvability(combo_boards, &signature_set);
        println!("solutions finished, sorting into groups");
        let second_board_groups = boards_to_groups(&combo_boards);
        let mut new_signature_elements = Vec::new();
        for (signature, mut elements) in second_board_groups {
            match board_groups.get(&signature) {
                Some(_) => {},
                None => {new_signature_elements.append(&mut elements); new_count += 1;}
                //None => {signature_set.push(elements.first_mut().unwrap().clone()); new_count += 1;}
            }
        }
        //println!("{}  boards solved", )
        println!("{} new groups, {} in total",new_count, new_count + board_groups.len());
        let mut silly_rust = new_signature_elements.clone();
        if new_count > 0 {
            
            new_signature_elements = append_solvability(new_signature_elements, &silly_rust);
            signature_set = append_solvability(signature_set, &new_signature_elements);
            signature_set.append(&mut new_signature_elements);
            board_groups = boards_to_groups(&signature_set);
            println!("{} groups after integration, {} active boards",board_groups.len(),signature_set.len());
        }
        std::mem::swap(&mut last_signature_set, &mut silly_rust);
        iter_count += 1;
    }
    println!("Process complete! hopefully this matches up with what I rambled about, it does still feel a lil shaky");
    board_groups
}

fn append_solvability(mut boards : Vec<SignatureElement>, signature_set : &Vec<SignatureElement>) -> Vec<SignatureElement>{

    let board_len = boards.len();
    let input_queue: ArrayQueue<(usize,SignatureElement)> = ArrayQueue::new(boards.len());
    let a_input_queue = Arc::new(input_queue);

    let output_queue: ArrayQueue<(usize,SignatureElement)> = ArrayQueue::new(boards.len());
    let a_output_queue = Arc::new(output_queue);
    let mut resulting_boards = Vec::with_capacity(board_len);
    let mut idx = boards.len();
    while let Some(i) = boards.pop() {
        idx -= 1;
        a_input_queue.push((idx,i));
        
    }

    for _ in 0..16 {
            signature_append_consumer(signature_set,a_input_queue.clone(), a_output_queue.clone());
            }
    let mut num_recieved = 0;

    for _ in 0..board_len {
        resulting_boards.push(SignatureElement{board:vec![],signature:vec![]})
    }
    //println!("0 completed boards, 0 board solutions");
    let max_len = " completed boards,  solutions needed  -".len() + board_len.to_string().len() + (board_len*signature_set.len()).to_string().len();
    const char_cycle : [char;4]= ['\\','|','/','-'];
    let mut cycle_idx = 0;
    while num_recieved < board_len {
        
        match a_output_queue.pop() {
            Some(output) => {
                let (idx, sig_element) = output;
                resulting_boards[idx] = sig_element;
                num_recieved += 1;
            },
            None => {thread::sleep(time::Duration::from_millis(250)); cycle_idx = (cycle_idx + 1) % 4}
        }
        let formatted_string = format!("{} boards remaining, {} solutions needed {}",board_len - num_recieved, (board_len - num_recieved)*signature_set.len(), char_cycle[cycle_idx]);
        
        //print!("\r{:<1$}",formatted_string,max_len);
        //std::io::stdout().flush();

    }
    print!("\r{:<1$}\r","",max_len);
    std::io::stdout().flush();
    resulting_boards
}

fn boards_to_groups(signature_set : &Vec<SignatureElement>) ->  HashMap<Vec<bool>,Vec<SignatureElement>> {
    let mut groups : HashMap<Vec<bool>,Vec<SignatureElement>> = HashMap::new();
    for i in signature_set {
        match groups.get_mut(&i.signature) {
            Some(group_boards) => group_boards.push(i.clone()),
            None => {groups.insert(i.signature.clone(),vec![i.clone()]);}
        }
    }
    groups
} 

fn fast_group_builder() -> HashMap::<[bool;SIGNATURE_LENGTH],Vec<Vec<bool>>> {
    let mut cur_boards = HashMap::<[bool;SIGNATURE_LENGTH],Vec<Vec<bool>>>::new();

    let mut new_boards : Vec::<Vec<bool>> = vec![vec![]];;

    let mut old_boards : Vec::<Vec<bool>> = Vec::new();

    cur_boards.insert(which_prefixes_solvable(&Vec::<bool>::new()),vec![Vec::<bool>::new()]);

    while new_boards.len() > 0 {
        std::mem::swap(&mut old_boards,&mut new_boards);
        new_boards.clear(); 
        println!("{} {}",old_boards.len(),old_boards[0].len());
        //TODO: Change to popping from old_boards
        for board in &old_boards {
            let (empty,full) = board_to_next(board.clone());
            for new_board in vec![empty,full] {
                //new_boards.push(new_board.1.clone());
                match cur_boards.get_mut(&new_board.0) {
                    Some(_) => {},
                    None => {
                        new_boards.push(new_board.1.clone());
                        cur_boards.insert(new_board.0,vec![new_board.1]);
                        }
                }
            }
        }
    }
    println!("{} groups constructed",cur_boards.len());
    cur_boards
}

fn exhaustive_group_builder() -> HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>{
    //Group boards by what three-peg-combo they win/lose under
    //Example:
    //The empty set and 000 would be in the same group because they are solvable/unsolvable with the
    //same set of three bits afterward (options from now on) (succeed: 001, 010, 100, 110, 011) (fail: 000, 111)

    
    let mut cur_boards = HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>::new();

    let mut new_boards : Vec::<Vec<bool>> = vec![vec![]];;

    let mut old_boards : Vec::<Vec<bool>> = Vec::new();

    cur_boards.insert(which_prefixes_solvable(&Vec::<bool>::new()),vec![Vec::<bool>::new()]);
    /* 
    let mut new_board = Vec::<bool>::new();
    new_board.push(false);
    new_board.push(false);
    new_board.push(false);
    for i in 0..8 {
        new_board[0] = (i / 4) % 2 == 1;
        new_board[1] = (i / 2) % 2 == 1;
        new_board[2] = i % 2 == 1;
        let board_result = which_prefixes_solvable(&new_board);
        //println!("{} {}",board_to_string(&new_board),group_to_string(&board_result));
        match cur_boards.get_mut(&board_result) {
            
            Some(board_vec) => board_vec.push(new_board.clone()),
            None => {cur_boards.insert(board_result,vec![new_board.clone()]);}
        }
    }*/

    //Starting at 0... didn't work. gonna start at size 3 and pray!
    //cur_boards.insert([false,true,true,true,true,true,true,false], vec![Vec::new()]);
    //This tests to make sure that no prefixes that are lumped together diverge at any point.
    //For example, if there exists some option Sigma* w such that 000w is solvable but w is not (or vice versa),
    //this proof idea breaks down and I go to cry.

    for substr in 0..10 {
        let total_boards = (2 << substr) - 1;
        let old_board_total = new_boards.len();

        println!("new iteration (includes up to length {})", substr);
        println!("currently {} groups",cur_boards.len());
        println!("{} total boards",total_boards);
        let now = time::Instant::now();
        let input_queue: ArrayQueue<Vec<bool>> = ArrayQueue::new(old_board_total);
        let a_input_queue = Arc::new(input_queue);
    
        let output_queue: ArrayQueue<(([bool;SIGNATURE_LENGTH],Vec<bool>),([bool;SIGNATURE_LENGTH],Vec<bool>))> = ArrayQueue::new(old_board_total);
        let a_output_queue = Arc::new(output_queue);
        //For each group
        std::mem::swap(&mut old_boards,&mut new_boards);
        new_boards.clear();
        for board in &old_boards{  
            //println!("{}",count);
            //println!("{}",board_to_string(&board));
            //The crying step
            a_input_queue.push(board.clone()).unwrap();
            
        }
        let mut handlers = Vec::with_capacity(64);
        for _ in 0..64 {
            handlers.push(
                group_builder_consumer(a_input_queue.clone(), a_output_queue.clone())
            );
        }
        let mut num_recieved = 0;
        
        while num_recieved < old_board_total {
            match a_output_queue.pop() {
                Some(output) => {
                    let (empty, full) = output;
                    for new_board in vec![empty,full] {
                        new_boards.push(new_board.1.clone());
                        match cur_boards.get_mut(&new_board.0) {
                            Some(board_vec) => board_vec.push(new_board.1),
                            None => {cur_boards.insert(new_board.0,vec![new_board.1]);}
                        }
                    }
                    num_recieved += 1;
                }
                None => {thread::sleep(time::Duration::from_millis(250))}
            }

        }
        println!("ms per board: {}",((time::Instant::now()-now)/(old_board_total as u32)).as_millis());
        println!("");
    }
    cur_boards
}

fn prefix_test(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>){
    //Group boards by what three-peg-combo they win/lose under
    //Example:
    //The empty set and 000 would be in the same group because they are solvable/unsolvable with the
    //same set of three bits afterward (options from now on) (succeed: 001, 010, 100, 110, 011) (fail: 000, 111)
    for (prefix,boards) in groups{
        //println!("len = {},win conditions = {}",boards.len(), group_to_string(prefix));
        //For each board in a group, let's see what the next options will be!
        let next_prefixes = match boards.first() {
            Some(board) => {
                let mut new_board = board.clone();
                //println!("Reference board: {}",board_to_string(&new_board));
                new_board.push(false);
                let empty_add = which_prefixes_solvable(&new_board);
                *new_board.last_mut().expect("impossible") = true;
                let full_add = which_prefixes_solvable(&new_board);
                [empty_add,full_add]
            },
            None => [[false;SIGNATURE_LENGTH],[false;SIGNATURE_LENGTH]]
        };
        for board in boards{
            //The crying step
            let mut new_board = board.clone();
            new_board.push(false);
            let empty_add = which_prefixes_solvable(&new_board);
            *new_board.last_mut().expect("impossible") = true;
            let full_add = which_prefixes_solvable(&new_board);

            if next_prefixes != [empty_add,full_add]{
                println!("Damn. Prefix test failed.");
                println!("Prefix group: {}", group_to_string(&prefix));
                println!("Reference board: {}",board_to_string(boards.first().expect("impossible")));
                println!("Problem board: {}",board_to_string(board));

                println!("Reference: {},{}", group_to_string(&next_prefixes[0]),group_to_string(&next_prefixes[1]));
                println!("Problem: {},{}", group_to_string(&empty_add),group_to_string(&full_add));

                return;
            }
        }
        //println!("Win conditions when 0 added: {}",group_to_string(&next_prefixes[0]));
        //println!("Win conditions when 1 added: {}",group_to_string(&next_prefixes[1]));
    }
    println!("Prefix test passed!")
}

fn group_solvability(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>) {
    for (prefix,boards) in groups{
        println!("len = {},win conditions = {}",boards.len(), group_to_string(prefix));
        //For each board in a group, let's see what the next options will be!
        let is_solvable = match boards.first() {
            Some(board) => {
                println!("Reference board: {}",board_to_string(board));
                bfs_solver(board)
            },
            None => false
        };
        for board in boards{
            if is_solvable != bfs_solver(&board){
                println!("Damn. Shared solvability failed.");
                println!("Prefix signature: {}",group_to_string(prefix));
                println!("Reference board: {}",board_to_string(boards.first().expect("impossible")));
                println!("Problem board: {}",board_to_string(board));

                println!("Reference: {:?}", is_solvable);
                println!("Problem: {:?}", bfs_solver(&board));

                return;
            }
        }
        println!("Is it solvable? {}",is_solvable);
    }
    println!("Solvability shared between groups!");
}

fn prefix_graph(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>) -> Graph::<String, String> {
    let mut group_graph = Graph::<String, String>::new();
    let mut group_idxs = HashMap::<[bool;SIGNATURE_LENGTH],NodeIndex>::new();

    for (prefix,boards) in groups{
        match boards.first() {
            Some(board) => {
                let solvable_char = match bfs_solver(&board) {
                    true => "Y",
                    false => "N"
                };
                let idx = group_graph.add_node(board_to_string(board) + solvable_char);
                group_idxs.insert(prefix.clone(),idx);
            },
            None => println!("ZOINKS")
        };
    }

    for (prefix,boards) in groups{
        //For each board in a group, let's see what the next options will be!
        match boards.first() {
            Some(board) => {
                let cur_idx = group_idxs.get(prefix).expect("no way jose");
                let mut new_board = board.clone();
                //println!("Reference board: {}",board_to_string(&new_board));
                new_board.push(false);
                let empty_add = which_prefixes_solvable(&new_board);
                let empty_idx = group_idxs.get(&empty_add).expect("no way jose");

                *new_board.last_mut().expect("impossible") = true;
                let full_add = which_prefixes_solvable(&new_board);
                let full_idx = group_idxs.get(&full_add).expect("no way jose");
                
                group_graph.add_edge(*cur_idx, *empty_idx, board_to_string(&vec![false]));
                group_graph.add_edge(*cur_idx, *full_idx, board_to_string(&vec![true]));
            },
            None => println!("ZOINKS")
        };
    }
    group_graph
}

fn SRS_groups(translator : &mut SRSTranslator, k : usize) -> Graph::<String, ()> {
    let mut group_graph = Graph::<String, ()>::new();
    let mut board_idxs = HashMap::<Vec<SymbolIdx>, NodeIndex>::new();

    for board in translator.build_sig_k(k){
        //Yeah i know this is obnoxiously inefficient but w/e
        if board.len() < k {
            continue;
        }
        let solvable_char = match translator.goal.contains(&board) {
                    true => "Y",
                    false => "N"
                };
        let idx = group_graph.add_node(symbols_to_string(&board) + solvable_char);
        board_idxs.insert(board,idx);
    }

    for (board, idx) in &board_idxs{
        //For each board in a group, let's see what the next options will be!
        //println!("Reference board: {}",board_to_string(&new_board));
        for i in translator.rules.single_rule(&board) {
            let end_idx = board_idxs.get(&i).unwrap();
            group_graph.add_edge(*idx, *end_idx, ());
        }
            
    }
    group_graph
}


fn identical_signature_elements(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>) {
    let mut identical_set = [true; SIGNATURE_LENGTH];
    let (reference_set,_) = groups.iter().next().unwrap();
    for (set,_) in groups {
        for i in 0..SIGNATURE_LENGTH {
            identical_set[i] &= reference_set[i] == set[i];
        }
    }
    print!("Identical boards:");
    for i in 0..SIGNATURE_LENGTH {
        if identical_set[i] {
            print!(" {},",board_to_string(&SIGNATURE_ELEMENTS[i]));
        }
    }
    println!("");
}
/*
written while high -- O(n^3) (i think) when it can be more interesting & O(n)
fn signature_element_groups(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>) {
    const empty_vec : Vec<bool> = Vec::new();
    let mut omnitable = [empty_vec;SIGNATURE_LENGTH];
    for (set,_) in groups {
        for i in 0..SIGNATURE_LENGTH {
            omnitable[i].push(set[i]);
        }
    }
    let mut meta_groups : HashMap::<Vec::<bool>,Vec::<usize>> = HashMap::new();
    for (idx,meta_element) in omnitable.iter().enumerate() {
        match meta_groups.get_mut(meta_element) {
            Some(meta_group) => meta_group.push(idx),
            None => {meta_groups.insert(meta_element.clone(),vec![idx]);}
        }
    }
    let mut idx = 0;
    for (meta_sig,meta_elements) in meta_groups {
        print!("Group {}: ",idx);
        for meta_element in meta_elements {
            print!(" {},",board_to_string(&SIGNATURE_ELEMENTS[meta_element]));
        }
        println!("");
        idx += 1;
    }
}*/

fn signature_element_groups(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>) {
    let (init_values,_) : (&[bool;SIGNATURE_LENGTH],&Vec::<Vec<bool>>) = groups.iter().next().unwrap();
    let mut s_or_d : Vec::<Vec<bool>> = Vec::new(); //Same or different
    //[i][j]
    //i = which signature element
    //j = which group
    for _ in 0..SIGNATURE_LENGTH{
        s_or_d.push(Vec::new());
    }
    for (set,_) in groups {
        for (idx,b) in set.iter().enumerate() {
            s_or_d[idx].push(init_values[idx] == *b);
        }
    }
    
    let mut s_or_d_groups : HashMap<Vec<bool>,Vec<usize>> = HashMap::new();
    for (idx,sig_element) in s_or_d.iter().enumerate() {
        match s_or_d_groups.get_mut(sig_element) {
            Some(group_idxs) => group_idxs.push(idx),
            None => {s_or_d_groups.insert(sig_element.clone(),vec![idx]);}
        }
    }
    let mut idx = 0;
    for (_, s_or_d_elements) in s_or_d_groups {
        print!("Group {}: ",idx);
        for s_or_d_element in s_or_d_elements {
            print!(" {},",board_to_string(&SIGNATURE_ELEMENTS[s_or_d_element]));
        }
        println!("");
        idx += 1;
    }
}

fn dot_parser(dot_output : String) -> String {
    dot_output.replace("\\\"", "")
    .replace("Y\" ]","\" shape=\"doublecircle\" ]")
    .replace("N\" ]","\" shape=\"circle\" ]")
}

/* 
fn dfa_builder() -> DFA {
    let mut trans_table : Vec<[usize;2]> = Vec::new(); //omg it's me !!!
    let mut table_reference = HashMap::<[bool;SIGNATURE_LENGTH],usize>::new();

    let mut new_boards : Vec::<Vec<bool>> = vec![vec![]];;

    let mut old_boards : Vec::<Vec<bool>> = Vec::new();

    let mut accepting_states : HashSet<usize> = HashSet::new();

    let start_accepting = which_prefixes_solvable(&Vec::<bool>::new());
    table_reference.insert(start_accepting.clone(),0);
    trans_table.push([0,0]);
    if bfs_solver(&Vec::<bool>::new()) {
        accepting_states.insert(0);
    }

    while new_boards.len() > 0 {
        std::mem::swap(&mut old_boards,&mut new_boards);
        new_boards.clear(); 
        println!("{} {}",old_boards.len(),old_boards[0].len());

        for board in &old_boards {
            let board_sig = which_prefixes_solvable(&board);
            let start_idx = *table_reference.get(&board_sig).unwrap();
            

            let (empty,full) = board_to_next(board.clone());
            for (sym_idx,new_board) in vec![empty,full].iter().enumerate() {

                let dest_idx = match table_reference.get(&new_board.0) {
                    Some(idx) => {
                        *idx
                    },
                    None => {
                        new_boards.push(new_board.1.clone());
                        let new_idx = trans_table.len();
                        
                        table_reference.insert(new_board.0,new_idx);
                        trans_table.push([0,0]);

                        if bfs_solver(&new_board.1) {
                            accepting_states.insert(new_idx);
                        }
                        new_idx
                        }
                    };
                trans_table[start_idx][sym_idx] = dest_idx;
                }  
                
            }
        }
    DFA {
        state_transitions : trans_table,
        accepting_states : accepting_states,
        starting_state : 0
    }
}*/
/* 
#[derive(Debug,Copy,Clone,Hash,PartialEq,Eq, Sequence)]
enum Binary {
    Zero,
    One
}

#[derive(Debug,Copy,Clone,Hash,PartialEq,Eq, Sequence)]
enum Ternary {
    Zero,
    One,
    Two
}

#[derive(Debug,Copy,Clone,Hash,PartialEq,Eq, Sequence)]
enum Collatz {
    Carry0,
    Carry1,
    Carry2,
    Zero,
    One,
    Start,
    Finish
}

impl fmt::Display for Binary {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        match self {
            Binary::Zero => write!(f, "0"),
            Binary::One => write!(f,"1")
        }
    }
}
impl fmt::Display for Ternary {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        match self {
            Ternary::Zero => write!(f, "0"),
            Ternary::One => write!(f,"1"),
            Ternary::Two => write!(f,"2"),
        }
    }
}
impl fmt::Display for Collatz {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        match self {
            Collatz::Zero => write!(f, "0"),
            Collatz::One => write!(f,"1"),
            Collatz::Carry0 => write!(f,"|C0|"),
            Collatz::Carry1 => write!(f,"|C1|"),
            Collatz::Carry2 => write!(f,"|C2|"),
            Collatz::Start => write!(f,"S"),
            Collatz::Finish => write!(f,"F"),

        }
    }
}
*/


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
    let mut translator = SRSTranslator {
        rules : ruleset,
        goal : goal_dfa,
        board_solutions : HashMap::new(),
        symbol_set : b_symbol_set
    };
    let possible_dfa = translator.dfa_with_sig_set(&translator.build_sig_k(5));
    possible_dfa.save("default1dpeg");
    */


    //Three-rule
    
    let b_symbol_set = SymbolSet {
        length : 2,
        representations : vec!["0".to_owned(),"1".to_owned()]
    };
    /* 
    let ruleset = Ruleset::new(
        vec![(vec![1,1,0],vec![0,0,1]),
                     (vec![0,1,1],vec![1,0,0]),
                     //(vec![1,0,1],vec![0,1,0])
                     //(Vec::from(vec![Binary::One,Binary::Zero,Binary::One]),Vec::from(vec![Binary::Zero,Binary::One,Binary::Zero])),
        ],
        b_symbol_set.clone()
    );
    */
    
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
    for (start, end) in &new_rules {
        println!("{:?} -> {:?}",start,end);
    }
    
    let by_k_symbol_set = SymbolSet {
        length : 2_u32.pow(k as u32) as usize,
        representations : vec!["000".to_owned(),"001".to_owned(),"010".to_owned(),"011".to_owned(),"100".to_owned(),"101".to_owned(),"110".to_owned(),"111".to_owned()] //whoops! lol
    };
    let ruleset = Ruleset::new(new_rules,by_k_symbol_set.clone());
    /* 
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1,1,2,1,2,2,2],vec![1,2,2,2,2,2,2,2],vec![2,2,2,2,2,2,2,2]],
        accepting_states : HashSet::from_iter(vec![1]),
        symbol_set : by_k_symbol_set.clone()
    };*/

    
    
    let goal_dfa = DFA {
        starting_state : 0,
        state_transitions : vec![vec![0,1,1,1,1,1,1,1],vec![1,1,1,1,1,1,1,1]],
        accepting_states : HashSet::from_iter(vec![0]),
        symbol_set : by_k_symbol_set.clone()
    };
    
    let mut translator = SRSTranslator {
        rules : ruleset,
        goal : goal_dfa,
        board_solutions : HashMap::new(),
        symbol_set : by_k_symbol_set
    };
    println!("This atrocity has {} rules", translator.rules.rules.len());
    //translator.verify_to_len(DFA::load("default2dpegx3").unwrap(), 8);
    //let possible_dfa = translator.dfa_with_sig_set_reverse(&translator.build_sig_k(5));
    
    //possible_dfa.save("flipx3"); 
    //possible_dfa.jflap_save("flipx3");
    let possible_dfa = DFA::load("flipx3").unwrap();
    translator.verify_to_len(possible_dfa,8);

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
}