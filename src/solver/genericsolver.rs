use crate::{SymbolIdx, SymbolSet};

use super::Solver;

pub trait GenericSolver<State, Input = String, Output = bool>
where
    Self: Sized,
    Output: Clone + std::marker::Send + 'static,
    Input: Clone + std::marker::Send + 'static,
    State: std::marker::Send + 'static,
    Self: Solver<State, Input, Output>,
{
    fn set_mutator(&mut self, mutator: fn(&Self, State, SymbolIdx) -> State);
    fn set_evaluator(&mut self, evaluator: fn(&Self, &State) -> Output);
    fn new(
        mutator: fn(&Self, State, SymbolIdx) -> State,
        evaluator: fn(&Self, &State) -> Output,
        symset: SymbolSet<Input>,
    ) -> Self;
}
