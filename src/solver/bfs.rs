use std::collections::{HashMap, HashSet};
use std::sync::{mpsc::Sender, Arc};
use std::thread;
use std::time::Duration;

use async_trait::async_trait;
use crossbeam::queue::SegQueue;

use crate::util::{Ruleset, DFA, SymbolIdx};
use crate::solver::Solver;

use super::{DFAStructure, SSStructure, Instant};

use bitvec::prelude::*;

#[derive(Clone)]
pub struct BFSSolver {
    pub rules : Ruleset,
    min_input : usize,
    max_input : usize,
    pub goal : DFA,
    worker_threads : usize
}

#[async_trait]
impl Solver for BFSSolver {

    fn get_max_input(&self) -> usize {
        self.max_input
    }
    fn get_min_input(&self) -> usize {
        self.min_input
    }

    fn get_goal(&self) -> &DFA {
        &self.goal
    }

    fn get_phases() -> Vec<String> {
        vec!["Entire Iteration".to_owned()]
    }

    fn get_ruleset(&self) -> &Ruleset{
        &self.rules
    }

    fn new(ruleset:Ruleset, goal :DFA) -> Self {
        assert_eq!(ruleset.symbol_set,goal.symbol_set);
        let (min_input, max_input) = BFSSolver::sized_init(&ruleset);
        BFSSolver { 
            rules: ruleset,
            goal : goal,
            worker_threads : 32,
            min_input : min_input,
            max_input : max_input    
        }
    }
    fn run_internal(self, 
        sig_k : usize, 
        is_debug : bool,
        dfa_events : Sender<(DFAStructure,SSStructure)>, 
        phase_events : Sender<Duration>) -> DFA 
    {
        let sig_set = self.rules.symbol_set.build_sig_k(sig_k);
        let mut trans_table : Vec<Vec<usize>> = Vec::new(); //omg it's me !!!
        let mut table_reference = HashMap::<BitVec,usize>::new();
    
        let mut new_boards : Vec::<(usize,Vec<SymbolIdx>)> = vec![(0,vec![])];
    
        let mut old_boards : Vec::<(usize,Vec<SymbolIdx>)> = Vec::new();
    
        let mut accepting_states : HashSet<usize> = HashSet::new();
        
        let thread_translator : Arc<BFSSolver> = Arc::new(self.clone());

        let (input, output) = self.create_workers(thread_translator.clone());

        let mut empty_copy : Vec<usize> = Vec::new();
        for _ in 0..self.rules.symbol_set.length {
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
            if is_debug {
                dfa_events.send((DFAStructure::Dense(trans_table.clone()),SSStructure::BooleanMap(table_reference.clone()))).unwrap();
            }
            let iter_begin_time = Instant::now();
            std::mem::swap(&mut old_boards,&mut new_boards);
            new_boards.clear(); 
    
            for (start_idx,board) in &old_boards {
                //Finds ingoing end of board.
                
                //Gets sig set of all boards with a single symbol added.
                //TODO: Use pool of worker threads used with main-thread-blocking sig set requests.
                //Change Translator to a trait and add a batch SRSTranslator and a hash SRSTranslator.
                let next_results = self.board_to_next_batch(&board, &sig_set, &input, &output);
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
                if is_debug {
                    let dur = iter_begin_time.elapsed();
                    phase_events.send(dur).unwrap();
                }
            }
        if is_debug {
            dfa_events.send((DFAStructure::Dense(trans_table.clone()),SSStructure::BooleanMap(table_reference.clone()))).unwrap();
        }
        self.terminate_workers(input);
        DFA {
            state_transitions : trans_table,
            accepting_states : accepting_states,
            starting_state : 0,
            symbol_set : self.rules.symbol_set.clone()
        }
    }
}

impl BFSSolver {
    pub fn new(ruleset:Ruleset, goal: DFA, worker_threads : usize) -> Self {
        let (min_input, max_input) = BFSSolver::sized_init(&ruleset);
        BFSSolver { 
            rules: ruleset,
            goal : goal,
            worker_threads : worker_threads,
            min_input : min_input,
            max_input : max_input 
        }
    }
    fn create_workers(&self, thread_translator : Arc<BFSSolver>) -> (Arc<SegQueue<(Vec<SymbolIdx>,usize)>>,Arc<SegQueue<(bool,usize)>>) {

        let input : Arc<SegQueue<(Vec<SymbolIdx>,usize)>> = Arc::new(SegQueue::new());
        let output : Arc<SegQueue<(bool,usize)>> = Arc::new(SegQueue::new());

        for _ in 0..self.worker_threads {
            worker_thread(thread_translator.clone(), input.clone(), output.clone());
        }
        (input, output)
    }
    fn terminate_workers(&self, input : Arc<SegQueue<(Vec<SymbolIdx>,usize)>>) {
        for _ in 0..self.worker_threads {
            input.push((vec![69, 42], usize::MAX))
        }
    }
    fn board_to_next_batch(&self,board : &Vec<SymbolIdx>, sig_set : &Vec<Vec<SymbolIdx>>,input : &Arc<SegQueue<(Vec<SymbolIdx>,usize)>>, output : &Arc<SegQueue<(bool,usize)>>) -> Vec<(BitVec,Vec<SymbolIdx>)> {
        let mut results = Vec::with_capacity(self.rules.symbol_set.length);
        for sym in 0..(self.rules.symbol_set.length as SymbolIdx) {
            let mut new_board = board.clone();
            new_board.push(sym);
            results.push((self.sig_with_set_batch(&new_board,sig_set,input.clone(),output.clone()),new_board));
        }
        results
    }
    fn sig_with_set_batch(&self, board : &Vec<SymbolIdx>, sig_set : &Vec<Vec<SymbolIdx>>, input : Arc<SegQueue<(Vec<SymbolIdx>,usize)>>, output : Arc<SegQueue<(bool,usize)>>) -> BitVec {
        let mut result : BitVec = bitvec![0;sig_set.len()];

        for sig_element in sig_set.iter().enumerate() {
            let mut new_board = board.clone();
            new_board.extend(sig_element.1);
            input.push((new_board,sig_element.0));
        }
        let mut results_recieved = 0;
        while results_recieved < sig_set.len() {
            match output.pop() {
                Some(output_result) => {
                    result.set(output_result.1,output_result.0); 
                    results_recieved+=1;
                },
                None => {std::thread::sleep(Duration::from_millis(10));}
            }
        }
        result
    }
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
                for new_board in self.single_rule_hash(board) {
                    if !known_states.contains(&new_board) {
                        known_states.insert(new_board.clone());
                        new_boards.push(new_board);
                    }
                }
            }
        }
        false
    }
}

fn worker_thread(translator : Arc<BFSSolver>, input : Arc<SegQueue<(Vec<SymbolIdx>,usize)>>, output : Arc<SegQueue<(bool,usize)>>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        loop {
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