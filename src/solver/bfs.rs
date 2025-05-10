use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::ops::Range;
use std::sync::{mpsc::{Sender,Receiver}, Arc};
use std::thread;
use std::time::Duration;

use async_trait::async_trait;


use crate::solver::Solver;
use crate::util::{Ruleset, SymbolIdx, DFA};
use crate::SymbolSet;

use super::Instant;
use super::{DFAStructure, GenericSolver, SRSSolver, SSStructure};
use crate::solver::srssolver::DomainError;

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

    fn new(ruleset: Ruleset, goal: DFA) -> Result<Self, DomainError> {
        //Self::ensure_expansion(&mut ruleset,&mut goal);
        Ok(BFSSolver {
            rules: Some(ruleset),
            goal: Some(goal.clone()),
            symbol_set: goal.symbol_set.clone(),
            worker_threads: 32,
            evaluator: Self::bfs_solver_batch,
            mutator: SRSSolver::MUTATOR,
        })
    }
}


struct EvaluatedState<Output> {
    origin_idx : usize,
    sym_idx : SymbolIdx,
    results : Vec<Output>,
    chunks_received : usize,
}

#[async_trait]
impl<State, Input, Output> Solver<State, Input, Output> for BFSSolver<State, Input, Output>
where
    State: Clone + 'static + std::marker::Send + std::marker::Sync + Default + std::cmp::Eq + std::hash::Hash,
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
        let mut trans_table: Vec<Vec<usize>> = Vec::new(); //omg it's me !!!
        let mut table_reference = HashMap::<Vec<Output>, usize>::new();

        let mut new_boards: Vec<(usize, State)> = vec![(0, origin.clone())];

        let mut old_boards: Vec<(usize, State)> = Vec::new();

        let mut state_outputs: Vec<Output> = vec![self.evaluate(&origin)];

        let thread_translator: Arc<Self> = Arc::new(self.clone());

        let mut new_state_to_ctx = HashMap::<State,EvaluatedState<Output>>::new();

        let (mut input, output) = self.create_workers(thread_translator.clone());

        let mut empty_copy: Vec<usize> = Vec::new();
        for _ in 0..self.symbol_set.length {
            empty_copy.push(0);
        }

        input.send(Dispatch{origin, k: sig_k, range : 0..self.symbol_set.sig_set_size(sig_k)}).unwrap();

        let start_accepting = output.recv().unwrap().results;
        table_reference.insert(start_accepting.clone(), 0);
        trans_table.push(empty_copy.clone());

        //redundant bc of start_accepting already checking this but idc

        if is_debug {
            let _ = phase_events.send(Instant::now() - init_begin_time);
        }
        while new_boards.len() > 0 {
            new_state_to_ctx.clear();
            if is_debug {
                //TODO: Genericize this
                //dfa_events.send((DFAStructure::Dense(trans_table.clone()),SSStructure::BooleanMap(table_reference.clone()))).unwrap();
            }
            let iter_begin_time = Instant::now();
            std::mem::swap(&mut old_boards, &mut new_boards);
            new_boards.clear();

            //dispatch all processing for worker threads 
            let mut chunks_per_sig_set = old_boards.len() * self.symbol_set.length / self.worker_threads / 4;
            if chunks_per_sig_set == 0 {
                chunks_per_sig_set = 1;
            }
            let mut chunks_dispatched = 0;
            let chunk_length = self.symbol_set.sig_set_size(sig_k) / chunks_per_sig_set;
            for (start_idx, board) in &old_boards {
                for sym_idx in 0..(self.symbol_set.length as SymbolIdx) {
                    let new_board = self.mutate(board.clone(), sym_idx);

                    new_state_to_ctx.insert(new_board.clone(), EvaluatedState {origin_idx: *start_idx,
                        sym_idx,
                        results : vec![Output::default(); self.symbol_set.sig_set_size(sig_k)],
                        chunks_received : 0
                    });
                    
                    //First chunk takes the brunt of any divisibility issues so sig set elements aren't missing
                    let mut chunk_cursor = chunk_length + self.symbol_set.sig_set_size(sig_k) % chunks_per_sig_set;
                    input.send(Dispatch {
                        origin : new_board.clone(),
                        k : sig_k,
                        range : 0..chunk_cursor
                    }).unwrap();
                    chunks_dispatched+=1;
                    //dole out any additional chunks
                    for _chunk_idx in 1..chunks_per_sig_set {
                        input.send(Dispatch {
                            origin : new_board.clone(),
                            k : sig_k,
                            range : chunk_cursor..chunk_cursor + chunk_length
                        }).unwrap();
                        chunks_dispatched+=1;
                        chunk_cursor += chunk_length;
                    }
                    assert!(chunk_cursor == self.symbol_set.sig_set_size(sig_k), "Internal chunking issue");
                }
            }    
            // Collect results from worker threads + evaluate when appropriate
            let mut chunks_collected = 0;
            while chunks_collected < chunks_dispatched {
                let collected_chunk = output.recv().unwrap();
                chunks_collected += 1;
                let eval_state = new_state_to_ctx.get_mut(&collected_chunk.origin).unwrap();
                
                //Copy in chunk info
                eval_state.results[collected_chunk.range].clone_from_slice(&collected_chunk.results);
                eval_state.chunks_received += 1;


                if eval_state.chunks_received == chunks_per_sig_set {
                //Checking if the next board's sig set already exists in DFA
                    let dest_idx = match table_reference.get(&eval_state.results) {
                        //If it does, the arrow's obv going to the existing state in the DFA
                        Some(idx) => *idx,
                        //If it doesn't, add a new state to the DFA!
                        None => {
                            let new_idx = trans_table.len();
                            new_boards.push((new_idx, collected_chunk.origin.clone()));

                            table_reference.insert(eval_state.results.clone(), new_idx);
                            trans_table.push(empty_copy.clone());

                            state_outputs.push(thread_translator.evaluate(&collected_chunk.origin));
                            new_idx
                        }
                    };
                trans_table[eval_state.origin_idx][eval_state.sym_idx as usize] = dest_idx;
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
    State: Clone + 'static + std::marker::Send + std::marker::Sync + Default + std::hash::Hash + Eq,
    Input: std::marker::Send + Clone + 'static + std::marker::Sync,
    Output:
        std::marker::Send + Clone + 'static + std::marker::Sync + Default + std::hash::Hash + Eq,
{
    fn set_mutator(&mut self, mutator: fn(&Self, state: State, input: SymbolIdx) -> State) {
        self.mutator = mutator;
    }
    fn set_evaluator(&mut self, evaluator: fn(&Self, &State) -> Output) {
        self.evaluator = evaluator;
    }
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
}

impl<State, Input, Output> BFSSolver<State, Input, Output>
where
    State: Clone + std::marker::Sync + std::marker::Send + Default + 'static + Hash + Eq,
    Input: std::marker::Send + std::marker::Sync + Clone + 'static,
    Output: std::marker::Send + Clone + 'static + std::marker::Sync + Default + Hash + Eq,
{
    fn create_workers(
        &self,
        thread_translator: Arc<Self>,
    ) -> (
        spmc::Sender<Dispatch<State>>,
        Receiver<DispatchResponse<State, Output>>,
    ) {
        let (input_tx, input_rx) = spmc::channel::<Dispatch<State>>();
        let (output_tx, output_rx)= std::sync::mpsc::channel();

        for _ in 0..self.worker_threads {
            worker_thread(thread_translator.clone(), input_rx.clone(), output_tx.clone());
        }
        (input_tx, output_rx)
    }
    fn terminate_workers(&self, mut input: spmc::Sender<Dispatch<State>>) {
        for _ in 0..self.worker_threads {
            input.send(Dispatch{origin : State::default(), range : 0..usize::MAX, k : 0}).unwrap();
        }
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

struct Dispatch<State> where
State: Clone + std::marker::Sync + std::marker::Send + 'static {
    origin : State,
    k : usize,
    range : Range<usize>
}

struct DispatchResponse<State, Output> where State: Clone + std::marker::Sync + std::marker::Send + 'static, Output: std::marker::Send + std::marker::Sync + Clone + 'static {
    origin : State,
    results : Vec<Output>,
    range : Range<usize>
}


fn worker_thread<State, Input, Output>(
    translator: Arc<BFSSolver<State, Input, Output>>,
    input: spmc::Receiver<Dispatch<State>>,
    output: Sender<DispatchResponse<State,Output>>,
) -> thread::JoinHandle<()>
where
State: Clone + 'static + std::marker::Send + std::marker::Sync + Default + Hash + Eq,
Input: std::marker::Send + std::marker::Sync + Clone + 'static,
Output: std::marker::Send + std::marker::Sync + Clone + 'static + Default + std::hash::Hash + Eq,
{
    thread::spawn(move || {
    loop {
            let dispatch = input.recv().unwrap();
            if dispatch.range.end == usize::MAX {
                return;
            }
            let sig_set_iter = translator.get_sig_set(dispatch.origin.clone(),dispatch.k).clone();
            let mut sig_set_iter = sig_set_iter.skip(dispatch.range.start);
            let mut result_vec = vec![];
            for _ in dispatch.range.clone() {
                result_vec.push((translator.evaluator)(&translator, &sig_set_iter.next().unwrap()));
            }
            output.send(DispatchResponse {
                origin : dispatch.origin,
                results : result_vec,
                range : dispatch.range
            }).unwrap();
        }
    })
}
