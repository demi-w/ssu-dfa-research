use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::{mpsc::Sender, Arc};
use std::thread;
use std::time::Duration;

use async_trait::async_trait;

use crossbeam::queue::SegQueue;

use crate::solver::Solver;
use crate::util::{Ruleset, SymbolIdx, DFA};
use crate::SymbolSet;

use super::Instant;
use super::{DFAStructure, GenericSolver, SRSSolver, SSStructure};
use crate::solver::srssolver::DomainError;

use bitvec::prelude::*;

#[derive(Clone)]
pub struct BFSSolver<State = Vec<u8>, Input = String, Output = bool>
where
    State: Clone,
{
    rules: Option<Ruleset>,
    goal: Option<DFA<Input, Output>>,
    pub symbol_set: SymbolSet<Input>,
    worker_threads: usize,
    mutator: fn(&Self, State, SymbolIdx) -> State,
    evaluator: fn(&Self, &State) -> Output,
}

impl SRSSolver for BFSSolver<Vec<SymbolIdx>, String, bool> {
    fn get_goal(&self) -> &DFA {
        &self.goal.as_ref().unwrap()
    }
    fn get_ruleset(&self) -> &Ruleset {
        &self.rules.as_ref().unwrap()
    }

    fn new(mut ruleset: Ruleset, mut goal: DFA) -> Result<Self, DomainError> {
        //Self::ensure_expansion(&mut ruleset,&mut goal);
        Ok(BFSSolver {
            rules: Some(ruleset),
            goal: Some(goal.clone()),
            symbol_set: goal.symbol_set.clone(),
            worker_threads: 32,
            evaluator: SRSSolver::default_evaluator,
            mutator: SRSSolver::MUTATOR,
        })
    }
}

#[async_trait]
impl<State, Input, Output> Solver<State, Input, Output> for BFSSolver<State, Input, Output>
where
    State: Clone + 'static + std::marker::Send + std::marker::Sync + Default,
    Input: std::marker::Send + std::marker::Sync + Clone + 'static,
    Output:
        std::marker::Send + std::marker::Sync + Clone + 'static + Default + std::hash::Hash + Eq,
{
    const PHASES: &'static [&'static str] = &["Entire Iteration"];
    fn get_symset(&self) -> &crate::SymbolSet<Input> {
        &self.symbol_set
    }
    fn mutate(&self, state: State, input: SymbolIdx) -> State {
        (self.mutator)(&self, state, input)
    }
    fn evaluate(&self, state: &State) -> Output {
        (self.evaluator)(&self, state)
    }

    fn run_internal(
        self,
        sig_k: usize,
        is_debug: bool,
        dfa_events: Sender<(DFAStructure, SSStructure)>,
        phase_events: Sender<Duration>,
        origin: State,
    ) -> DFA<Input, Output> {
        let init_begin_time = Instant::now();
        let sig_set = self.symbol_set.build_sig_k(sig_k);
        let mut trans_table: Vec<Vec<usize>> = Vec::new(); //omg it's me !!!
        let mut table_reference = HashMap::<Vec<Output>, usize>::new();

        let mut new_boards: Vec<(usize, State)> = vec![(0, origin.clone())];

        let mut old_boards: Vec<(usize, State)> = Vec::new();

        let mut state_outputs: Vec<Output> = vec![self.evaluate(&origin)];

        let thread_translator: Arc<Self> = Arc::new(self.clone());

        let (input, output) = self.create_workers(thread_translator.clone());

        let mut empty_copy: Vec<usize> = Vec::new();
        for _ in 0..self.symbol_set.length {
            empty_copy.push(0);
        }

        let start_accepting =
            self.sig_with_set_batch(&origin, &sig_set, input.clone(), output.clone());
        table_reference.insert(start_accepting.clone(), 0);
        trans_table.push(empty_copy.clone());

        //redundant bc of start_accepting already checking this but idc

        if is_debug {
            let _ = phase_events.send(Instant::now() - init_begin_time);
        }
        while new_boards.len() > 0 {
            if is_debug {
                //TODO: Genericize this
                //dfa_events.send((DFAStructure::Dense(trans_table.clone()),SSStructure::BooleanMap(table_reference.clone()))).unwrap();
            }
            let iter_begin_time = Instant::now();
            std::mem::swap(&mut old_boards, &mut new_boards);
            new_boards.clear();

            for (start_idx, board) in &old_boards {
                //Finds ingoing end of board.

                //Gets sig set of all boards with a single symbol added.
                //TODO: Use pool of worker threads used with main-thread-blocking sig set requests.
                //Change Translator to a trait and add a batch SRSTranslator and a hash SRSTranslator.
                let next_results = self.board_to_next_batch(&board, &sig_set, &input, &output);
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

                            state_outputs.push(thread_translator.evaluate(&new_board.1));
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
            //TODO: Genericize this
            //dfa_events.send((DFAStructure::Dense(trans_table.clone()),SSStructure::BooleanMap(table_reference.clone()))).unwrap();
        }
        self.terminate_workers(input);
        DFA {
            state_transitions: trans_table,
            accepting_states: state_outputs,
            starting_state: 0,
            symbol_set: self.symbol_set.clone(),
        }
    }
}

impl<State, Input, Output> GenericSolver<State, Input, Output> for BFSSolver<State, Input, Output>
where
    State: Clone + 'static + std::marker::Send + std::marker::Sync + Default,
    Input: std::marker::Send + Clone + 'static + std::marker::Sync,
    Output:
        std::marker::Send + Clone + 'static + std::marker::Sync + Default + std::hash::Hash + Eq,
{
    fn new(
        mutator: fn(&Self, State, SymbolIdx) -> State,
        evaluator: fn(&Self, &State) -> Output,
        symset: SymbolSet<Input>,
    ) -> Self {
        BFSSolver {
            rules: None,
            goal: None,
            symbol_set: symset,
            worker_threads: 32,
            mutator: mutator,
            evaluator: evaluator,
        }
    }
    fn set_evaluator(&mut self, evaluator: fn(&Self, &State) -> Output) {
        self.evaluator = evaluator;
    }
    fn set_mutator(&mut self, mutator: fn(&Self, state: State, input: SymbolIdx) -> State) {
        self.mutator = mutator;
    }
}

impl<State, Input, Output> BFSSolver<State, Input, Output>
where
    State: Clone + std::marker::Sync + std::marker::Send + Default + 'static,
    Input: std::marker::Send + std::marker::Sync + Clone + 'static,
    Output: std::marker::Send + Clone + 'static + std::marker::Sync + Default + Hash + Eq,
{
    fn create_workers(
        &self,
        thread_translator: Arc<Self>,
    ) -> (
        Arc<SegQueue<(State, usize)>>,
        Arc<SegQueue<(Output, usize)>>,
    ) {
        let input: Arc<SegQueue<(State, usize)>> = Arc::new(SegQueue::new());
        let output: Arc<SegQueue<(Output, usize)>> = Arc::new(SegQueue::new());

        for _ in 0..self.worker_threads {
            worker_thread(thread_translator.clone(), input.clone(), output.clone());
        }
        (input, output)
    }
    fn terminate_workers(&self, input: Arc<SegQueue<(State, usize)>>) {
        for _ in 0..self.worker_threads {
            input.push((State::default(), usize::MAX))
        }
    }
    fn board_to_next_batch(
        &self,
        board: &State,
        sig_set: &Vec<Vec<SymbolIdx>>,
        input: &Arc<SegQueue<(State, usize)>>,
        output: &Arc<SegQueue<(Output, usize)>>,
    ) -> Vec<(Vec<Output>, State)> {
        let mut results = Vec::with_capacity(self.symbol_set.length);
        for sym in 0..(self.symbol_set.length as SymbolIdx) {
            let mut new_board = board.clone();
            new_board = self.mutate(new_board, sym);
            results.push((
                self.sig_with_set_batch(&new_board, sig_set, input.clone(), output.clone()),
                new_board,
            ));
        }
        results
    }
    fn sig_with_set_batch(
        &self,
        board: &State,
        sig_set: &Vec<Vec<SymbolIdx>>,
        input: Arc<SegQueue<(State, usize)>>,
        output: Arc<SegQueue<(Output, usize)>>,
    ) -> Vec<Output> {
        let mut result = vec![Output::default(); sig_set.len()];

        for sig_element in sig_set.iter().enumerate() {
            let mut new_board = board.clone();
            for sig_in_element in sig_element.1 {
                new_board = self.mutate(new_board, *sig_in_element);
            }

            input.push((new_board, sig_element.0));
        }
        let mut results_recieved = 0;
        while results_recieved < sig_set.len() {
            match output.pop() {
                Some(output_result) => {
                    result[output_result.1] = output_result.0;
                    results_recieved += 1;
                }
                None => {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
        result
    }
}

impl BFSSolver<Vec<SymbolIdx>, String, bool>
where
    Self: SRSSolver,
{
    fn bfs_solver_batch(&self, start_board: &Vec<SymbolIdx>) -> bool {
        let mut new_boards: Vec<Vec<SymbolIdx>> = vec![start_board.clone()];
        let mut old_boards: Vec<Vec<SymbolIdx>> = vec![];
        let mut known_states = HashSet::<Vec<SymbolIdx>>::new();
        known_states.insert(start_board.clone());
        while new_boards.len() > 0 {
            std::mem::swap(&mut old_boards, &mut new_boards);
            new_boards.clear();
            for board in &old_boards {
                if self.goal.as_ref().unwrap().contains(board) {
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

fn worker_thread<State, Input, Output>(
    translator: Arc<BFSSolver<State, Input, Output>>,
    input: Arc<SegQueue<(State, usize)>>,
    output: Arc<SegQueue<(Output, usize)>>,
) -> thread::JoinHandle<()>
where
    State: Clone + std::marker::Sync + std::marker::Send + 'static,
    Input: std::marker::Send + std::marker::Sync + Clone + 'static,
    Output: std::marker::Send + std::marker::Sync + Clone + 'static,
{
    thread::spawn(move || {
        loop {
            match input.pop() {
                Some(input_string) => {
                    if input_string.1 == usize::MAX {
                        return;
                    }
                    let result = (translator.evaluator)(&translator, &input_string.0);
                    output.push((result, input_string.1));
                }
                None => {} //{std::thread::sleep(time::Duration::from_millis(10));}
            }
        }
    })
}
