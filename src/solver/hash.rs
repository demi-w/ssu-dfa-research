use std::collections::{HashMap, HashSet};

use crate::{
    solver::{DFAStructure, SSStructure},
    util::{Ruleset, SymbolIdx, DFA},
};

use super::{Instant, SRSSolver, Solver};
use crate::solver::srssolver::DomainError;

use bitvec::prelude::*;

#[derive(Clone)]
pub struct HashSolver {
    pub goal: DFA,
    pub rules: Ruleset,
    pub max_input: usize,
    pub min_input: usize,
    board_solutions: HashMap<Vec<SymbolIdx>, bool>,
}

impl SRSSolver for HashSolver {
    fn get_goal(&self) -> &DFA {
        &self.goal
    }
    fn get_ruleset(&self) -> &Ruleset {
        &self.rules
    }

    fn new(mut ruleset: Ruleset, mut goal: DFA) -> Result<Self, DomainError> {
        Self::ensure_expansion(&mut ruleset, &mut goal);
        let (min_input, max_input) = HashSolver::sized_init(&ruleset);
        Ok(HashSolver {
            min_input: min_input,
            max_input: max_input,
            goal: goal,
            rules: ruleset,
            board_solutions: HashMap::new(),
        })
    }
}

impl Solver for HashSolver {
    fn get_symset(&self) -> &crate::SymbolSet {
        &self.rules.symbol_set
    }
    const PHASES: &'static [&'static str] = &["Entire Iteration"];
    fn evaluate<'a, 'b>(&'a self, state: &'b Vec<u8>) -> bool {
        todo!()
    }

    fn mutate(&self, state: Vec<u8>, input: SymbolIdx) -> Vec<u8> {
        todo!()
    }
    fn run_internal(
        mut self,
        sig_k: usize,
        is_debug: bool,
        dfa_events: std::sync::mpsc::Sender<(DFAStructure, SSStructure)>,
        phase_events: std::sync::mpsc::Sender<std::time::Duration>,
        origin: Vec<SymbolIdx>,
    ) -> DFA {
        let init_begin_time = Instant::now();
        let sig_set = self.rules.symbol_set.build_sig_k(sig_k);

        let mut trans_table: Vec<Vec<usize>> = Vec::new(); //omg it's me !!!
        let mut table_reference = HashMap::<BitVec, usize>::new();

        let mut new_boards: Vec<(usize, Vec<SymbolIdx>)> = vec![(0, origin.clone())];

        let mut old_boards: Vec<(usize, Vec<SymbolIdx>)> = Vec::new();

        let mut accepting_states = vec![false; 1];

        let mut empty_copy: Vec<usize> = Vec::new();
        for _ in 0..self.rules.symbol_set.length {
            empty_copy.push(0);
        }

        let start_accepting = self.sig_with_set(&origin, &sig_set);
        table_reference.insert(start_accepting.clone(), 0);
        trans_table.push(empty_copy.clone());

        //redundant bc of start_accepting already checking this but idc
        if self.bfs_solver(&origin) {
            accepting_states[0] = true;
        }

        if is_debug {
            let _ = phase_events.send(Instant::now() - init_begin_time);
        }
        while new_boards.len() > 0 {
            if is_debug {
                dfa_events
                    .send((
                        DFAStructure::Dense(trans_table.clone()),
                        SSStructure::BooleanMap(table_reference.clone()),
                    ))
                    .unwrap();
            }
            let iter_begin_time = Instant::now();
            std::mem::swap(&mut old_boards, &mut new_boards);
            new_boards.clear();
            self.board_solutions = HashMap::new();
            for (start_idx, board) in &old_boards {
                //Finds ingoing end of board.

                //Gets sig set of all boards with a single symbol added.
                let next_results = self.board_to_next(&board, &sig_set);
                for (sym_idx, new_board) in next_results.iter().enumerate() {
                    //Checking if the next board's sig set already exists in DFA
                    let dest_idx = match table_reference.get(&new_board.0) {
                        //If it does, the arrow's obv going to the existing state in the DFA
                        Some(idx) => *idx,
                        //If it doesn't, add a new state to the DFA!
                        None => {
                            let new_idx = trans_table.len();
                            new_boards.push((new_idx, new_board.1.clone()));

                            table_reference.insert(new_board.0.clone(), new_idx);
                            trans_table.push(empty_copy.clone());
                            accepting_states.push(new_board.0[0]);
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
            dfa_events
                .send((
                    DFAStructure::Dense(trans_table.clone()),
                    SSStructure::BooleanMap(table_reference.clone()),
                ))
                .unwrap();
        }
        DFA {
            state_transitions: trans_table,
            accepting_states: accepting_states,
            starting_state: 0,
            symbol_set: self.rules.symbol_set.clone(),
        }
    }
}

impl HashSolver {
    fn sig_with_set(&mut self, board: &Vec<SymbolIdx>, sig_set: &Vec<Vec<SymbolIdx>>) -> BitVec {
        let mut result = bitvec![0;sig_set.len()];
        for (idx, sig_element) in sig_set.iter().enumerate() {
            let mut new_board = board.clone();
            new_board.extend(sig_element);
            result.set(idx, self.bfs_solver(&new_board));
        }
        result
    }
    fn bfs_solver(&mut self, start_board: &Vec<SymbolIdx>) -> bool {
        let mut start_idx = 0;
        let mut end_idx = 0;
        let mut all_boards: Vec<(usize, Vec<SymbolIdx>)> = vec![(0, start_board.clone())];
        let mut known_states = HashSet::<Vec<SymbolIdx>>::new();
        known_states.insert(start_board.clone());
        let mut answer_idx = 0;
        let mut answer_found = false;
        while (start_idx != end_idx || start_idx == 0) && !answer_found {
            start_idx = end_idx;
            end_idx = all_boards.len();
            for board_idx in start_idx..end_idx {
                if self.goal.contains(&all_boards[board_idx].1) {
                    answer_idx = board_idx;
                    answer_found = true;
                    break;
                }
                if let Some(found_answer) = self.board_solutions.get(&all_boards[board_idx].1) {
                    if !*found_answer {
                        continue;
                    } else {
                        answer_idx = board_idx;
                        answer_found = true;
                        break;
                    }
                }
                for new_board in self.single_rule_hash(&all_boards[board_idx].1) {
                    if !known_states.contains(&new_board) {
                        known_states.insert(new_board.clone());
                        all_boards.push((board_idx, new_board));
                    }
                }
            }
        }
        //did we find an answer board
        match answer_found {
            false => {
                //if it's unsolvable, then we know everything here is
                while let Some((_, board)) = all_boards.pop() {
                    self.board_solutions.insert(board, false);
                }
                false
            }
            //this can be dramatically improved i think
            //following path of solvability
            true => {
                while answer_idx != 0 {
                    self.board_solutions
                        .insert(all_boards[answer_idx].1.clone(), true);
                    answer_idx = all_boards[answer_idx].0;
                }
                self.board_solutions.insert(all_boards[0].1.clone(), true);
                true
            }
        }
    }
    fn board_to_next(
        &mut self,
        board: &Vec<SymbolIdx>,
        sig_set: &Vec<Vec<SymbolIdx>>,
    ) -> Vec<(BitVec, Vec<SymbolIdx>)> {
        let mut results = Vec::with_capacity(self.rules.symbol_set.length);
        for sym in 0..(self.rules.symbol_set.length as SymbolIdx) {
            let mut new_board = board.clone();
            new_board.push(sym);
            results.push((self.sig_with_set(&new_board, sig_set), new_board));
        }
        results
    }
}
