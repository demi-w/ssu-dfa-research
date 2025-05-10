use std::{
    io::{Write},
    marker::PhantomData,
    sync::mpsc::{channel, Receiver, Sender},
};

use std::thread;

use crate::util::{SymbolIdx, SymbolSet, DFA};

use crate::solver::events::*;

//mod generic_bases;

#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;
pub trait Solver<State = Vec<SymbolIdx>, Input = String, Output = bool>
where
    Self: Sized + Clone + Send + 'static,
    State: std::marker::Send + 'static,
    Output: Clone + std::marker::Send + 'static,
    Input: Clone + std::marker::Send + 'static,
{
    const PHASES: &'static [&'static str];

    #[cfg(not(target_arch = "wasm32"))]
    fn run_debug(
        &self,
        sig_k: usize,
        origin: State,
    ) -> (
        Receiver<(DFAStructure, SSStructure)>,
        Receiver<std::time::Duration>,
        thread::JoinHandle<DFA<Input, Output>>,
    ) {
        let self_clone = self.clone();
        let (dfa_tx, dfa_rx) = channel();
        let (phase_tx, phase_rx) = channel();
        (
            dfa_rx,
            phase_rx,
            thread::spawn(move || self_clone.run_internal(sig_k, true, dfa_tx, phase_tx, origin)),
        )
    }

    //Changing the function signature based on the architecture is disgusting!
    //But ya know what -- so is the state of Rust WASM, so i'm making do.
    #[cfg(target_arch = "wasm32")]
    fn run_debug(
        &self,
        sig_k: usize,
        origin: State,
    ) -> (
        Receiver<(DFAStructure, SSStructure)>,
        Receiver<std::time::Duration>,
    ) {
        let self_clone = self.clone();
        let (dfa_tx, dfa_rx) = channel();
        let (phase_tx, phase_rx) = channel();
        wasm_bindgen_futures::spawn_local(async move {
            self_clone.run_internal(sig_k, true, dfa_tx, phase_tx);
        });
        (dfa_rx, phase_rx)
    }
    fn run_internal(
        self,
        sig_k: usize,
        is_debug: bool,
        dfa_events: Sender<(DFAStructure, SSStructure)>,
        phase_events: Sender<std::time::Duration>,
        origin: State,
    ) -> DFA<Input, Output>;
    fn run(&self, sig_k: usize, origin: State) -> DFA<Input, Output> {
        let (dfa_tx, _dfa_rx) = channel();
        let (phase_tx, _phase_rx) = channel();
        self.clone()
            .run_internal(sig_k, false, dfa_tx, phase_tx, origin)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn run_with_print(&self, sig_k: usize, origin: State) -> DFA<Input, Output> {
        use std::io;

        let (dfa_rx, phase_rx, run_handle) = self.run_debug(sig_k, origin);
        let mut phase_idx;
        let mut phase_lens = vec![];
        let mut iterations = 0;
        let mut last_len = 0;
        if let Ok(time) = phase_rx.recv() {
            println!("Initialization time: {}ms", time.as_millis());
        }
        while let Ok((partial_dfa, _sig_sets)) = dfa_rx.recv() {
            let mut update_string = format!(
                "Iteration {} | {} states solved, {} new",
                iterations,
                partial_dfa.len(),
                partial_dfa.len() - last_len
            );
            last_len = partial_dfa.len();
            phase_idx = 0;
            print!("{}\r", update_string);
            io::stdout().flush().unwrap();
            while phase_idx < Self::PHASES.len() {
                //Disconnection is guaranteed here -- should send final DFA then dc on both channels
                match phase_rx.recv() {
                    Ok(time) => {
                        phase_lens.push(time);
                    }
                    _ => break,
                }

                update_string.push_str(&format!(
                    " | {}: {}ms",
                    Self::PHASES[phase_idx],
                    phase_lens.last().unwrap().as_millis()
                ));
                print!("{}\r", update_string);
                io::stdout().flush().unwrap();
                phase_idx += 1;
            }
            iterations += 1;
            println!("{}", update_string);
        }
        run_handle.join().unwrap()
    }
    fn get_symset(&self) -> &SymbolSet<Input>;
    fn mutate(&self, state: State, input: SymbolIdx) -> State;
    fn evaluate<'a, 'b>(&'a self, state: &'b State) -> Output;
    fn get_sig_set<'a>(&'a self, origin: State, k: usize) -> StateIter<'a,Self, State, Input, Output> {
        StateIter {
            solver: self,
            k: k,
            cur_vec: vec![],
            cur_states: vec![origin],
            pi: Default::default(),
            po: Default::default(),
        }
    }
}

#[derive(Clone)]
pub struct StateIter<'a, Solver, State, Input, Output>
where
    State: std::marker::Send + 'static,
    Output: Clone + std::marker::Send + 'static,
    Input: Clone + std::marker::Send + 'static,
    Solver: crate::solver::Solver<State, Input, Output>,
{
    solver: &'a Solver,
    k: usize,
    cur_vec: Vec<SymbolIdx>,
    cur_states: Vec<State>,
    pi: PhantomData<Input>,
    po: PhantomData<Output>,
}

impl<'a, Solver, State, Input, Output> Iterator for StateIter<'a, Solver, State, Input, Output>
where
    State: std::marker::Send + 'static + Clone,
    Output: Clone + std::marker::Send + 'static,
    Input: Clone + std::marker::Send + 'static,
    Solver: crate::solver::Solver<State, Input, Output>,
{
    type Item = State;
    fn next(&mut self) -> Option<Self::Item> {
        let result = if self.cur_vec.len() > self.k {
            None
        } else {
            Some(self.cur_states.last().unwrap().clone())
        };
        let mut rollover = self.cur_vec.len();
        while rollover > 0
            && self.cur_vec[rollover - 1] == (self.solver.get_symset().length - 1) as u8
        {
            self.cur_vec[rollover - 1] = 0;
            rollover -= 1;
        }
        if rollover > 0 {
            self.cur_vec[rollover - 1] += 1;
        } else {
            self.cur_vec.push(0);

            //This gets overwritten so we can fill it with w/e
            self.cur_states.push(self.cur_states[0].clone());
        }
        let start = if rollover > 0 { rollover - 1 } else { 0 };
        for i in start..self.cur_vec.len() {
            self.cur_states[i + 1] = self
                .solver
                .mutate(self.cur_states[i].clone(), self.cur_vec[i]);
        }

        result
    }
}
