use std::{
    collections::{HashMap, HashSet, VecDeque},
    hash::Hash,
};


use crate::util::{Ruleset, SymbolIdx, SymbolSet, DFA};


//mod generic_bases;

use petgraph::{
    graph::{DiGraph, NodeIndex},
    visit::EdgeRef,
};
#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;


use super::solver::Solver;

pub trait SRSSolver
where
    Self: Solver<Vec<SymbolIdx>, String, bool> + Sized,
{
    fn new(ruleset: Ruleset, goal: DFA) -> Result<Self, DomainError>;

    fn get_evaluator(&self) -> impl Fn(Vec<SymbolIdx>) -> bool {
        let goal_clone = self.get_goal().clone();
        move |x| goal_clone.contains(&x)
    }

    fn default_evaluator(&self, state: &Vec<SymbolIdx>) -> bool {
        todo!();
    }

    fn mutate(&self, mut state: Vec<SymbolIdx>, input: SymbolIdx) -> Vec<SymbolIdx> {
        state.push(input);
        state
    }
    fn evaluate(&self, state: Vec<SymbolIdx>) -> bool {
        //Will most likely just import bfs_solve as a baseline? could also be nice to actually make each solver implement this uniquely
        todo!();
    }

    fn get_mutator(&self) -> impl Fn(Vec<SymbolIdx>, SymbolIdx) -> Vec<SymbolIdx> {
        |mut state, input| {
            state.push(input);
            state
        }
    }
    const MUTATOR: fn(&Self, Vec<SymbolIdx>, SymbolIdx) -> Vec<SymbolIdx> =
        |&_, mut state, input| {
            state.push(input);
            state
        };

    fn ensure_expansion(ruleset: &mut Ruleset, goal: &mut DFA) {
        if ruleset.symbol_set == goal.symbol_set {
            return;
        }
        if ruleset.symbol_set.is_subset(&goal.symbol_set) {
            ruleset.expand_to_symset(goal.symbol_set.clone());
        } else if goal.symbol_set.is_subset(&ruleset.symbol_set) {
            goal.expand_to_symset(ruleset.symbol_set.clone());
        } else {
            let mut merge_symbols = goal.symbol_set.representations.clone();
            merge_symbols.append(&mut ruleset.symbol_set.representations.clone());
            let merge_symset = SymbolSet::<String>::new(merge_symbols);
            ruleset.expand_to_symset(merge_symset.clone());
            goal.expand_to_symset(merge_symset);
        }
    }

    fn get_ruleset(&self) -> &Ruleset;

    fn get_goal(&self) -> &DFA;

    fn single_rule_hash(&self, start_board: &Vec<SymbolIdx>) -> Vec<Vec<SymbolIdx>> {
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
            for slice_length in self.get_ruleset().min_input
                ..core::cmp::min(self.get_ruleset().max_input, start_board.len() - lftmst_idx) + 1
            {
                if let Some(new_swaps) = self
                    .get_ruleset()
                    .rules
                    .get(&start_board[lftmst_idx..(lftmst_idx + slice_length)])
                {
                    let new_board = start_board[0..lftmst_idx].to_vec();

                    for new_swap in new_swaps {
                        let mut newest_board = new_board.clone();
                        newest_board.extend(new_swap);
                        newest_board.extend(
                            start_board[lftmst_idx + slice_length..start_board.len()].to_vec(),
                        );
                        result.push(newest_board);
                    }
                }
            }
        }
        result
    }
    //returns an annotated list of all possible moves from a string
    //annotation is as follows: starting idx of rule application, len of lhs of rule used, len of rhs of rule used, resulting board.
    fn single_rule_hash_annotated(
        &self,
        start_board: &Vec<SymbolIdx>,
    ) -> Vec<(usize, usize, usize, Vec<SymbolIdx>)> {
        let mut result = vec![];
        if start_board.is_empty() {
            if let Some(new_swaps) = self.get_ruleset().rules.get(&start_board[..]) {
                for new_swap in new_swaps {
                    result.push((0, 0, new_swap.len(), new_swap.clone()));
                }
            }
            return result;
        }
        for lftmst_idx in 0..start_board.len() {
            for slice_length in self.get_ruleset().min_input
                ..core::cmp::min(self.get_ruleset().max_input, start_board.len() - lftmst_idx) + 1
            {
                if let Some(new_swaps) = self
                    .get_ruleset()
                    .rules
                    .get(&start_board[lftmst_idx..(lftmst_idx + slice_length)])
                {
                    let new_board = start_board[0..lftmst_idx].to_vec();
                    for new_swap in new_swaps {
                        let mut newest_board = new_board.clone();
                        newest_board.extend(new_swap);
                        newest_board.extend(
                            start_board[lftmst_idx + slice_length..start_board.len()].to_vec(),
                        );
                        result.push((lftmst_idx, slice_length, new_swap.len(), newest_board));
                    }
                }
            }
        }
        result
    }

    fn sized_init(rules: &Ruleset) -> (usize, usize) {
        let mut min_input: usize = usize::MAX;
        let mut max_input: usize = 0;
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
    fn solve_string(
        &self,
        possible_dfa: &DFA,
        input_str: &Vec<SymbolIdx>,
    ) -> Result<Vec<Vec<SymbolIdx>>, ()> {
        if !possible_dfa.contains(input_str) {
            return Err(());
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

    fn solve_string_annotated(
        &self,
        possible_dfa: &DFA,
        input_str: &Vec<SymbolIdx>,
    ) -> Result<Vec<(usize, usize, usize, Vec<SymbolIdx>)>, ()> {
        if !possible_dfa.contains(input_str) {
            return Err(());
        }
        if self.get_goal().contains(input_str) {
            return Ok(vec![]);
        }
        let mut visited = HashMap::new();

        let mut old_options = vec![];
        let mut new_options = vec![input_str.clone()];
        visited.insert(input_str.clone(), (0, 0, 0, vec![]));
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
                            result.push((
                                ancestor_info.0,
                                ancestor_info.1,
                                ancestor_info.2,
                                ancestor.clone(),
                            ));
                            ancestor = &ancestor_info.3;
                        }
                        result.reverse();
                        return Ok(result);
                    }
                    if !visited.contains_key(&option.3) && possible_dfa.contains(&option.3) {
                        //println!("{}",symbols_to_string(&intrepid_str));
                        new_options.push(option.3.clone());
                        visited.insert(
                            option.3.clone(),
                            (option.0, option.1, option.2, option_str.clone()),
                        );
                    }
                }
            }
        }
    }
    fn build_rule_graph<'a>(&'a self, possible_dfa: &'a DFA) -> DiGraph<usize, RuleGraphRoot<'a>> {
        let mut rule_graph = DiGraph::<usize, RuleGraphRoot>::new();
        //Add a node in the rule graph for each state
        for index in 0..possible_dfa.state_transitions.len() {
            rule_graph.add_node(index);
        }
        //See what single rule applications are possible when starting from any state in the DFA
        //Example -- create a path between (q0, 110) and (q0,001) if one does not exist
        for origin in 0..possible_dfa.state_transitions.len() {
            //For each state
            for rule_list in &self.get_ruleset().rules {
                //For each rule
                let lhs = rule_list.0;
                //Drawing an arrow from LHS to RHS
                //State at LHS = parent
                //State at RHS = child

                //Determine the state the LHS goes to from the origin (e.g. (q0, 110))
                let mut parent = origin;
                for i in 0..lhs.len() {
                    parent = possible_dfa.state_transitions[parent][lhs[i] as usize];
                }
                //Multiple rhs for one lhs is possible, this accommodates for that
                for rhs in rule_list.1 {
                    let mut child = origin;
                    //Determine the state the RHS goes to from the origin (e.g. (q0,001))
                    for i in 0..rhs.len() {
                        child = possible_dfa.state_transitions[child][rhs[i] as usize];
                    }
                    //update_edge adds an edge if one does not already exist
                    rule_graph.update_edge(
                        NodeIndex::new(parent),
                        NodeIndex::new(child),
                        RuleGraphRoot::new(
                            lhs.clone(),
                            rhs.clone(),
                            parent,
                            child,
                            origin,
                            &possible_dfa.symbol_set,
                        ),
                    );
                }
            }
        }
        //Now we have built all single rule applications where the LHS and RHS are the last N characters of the strings they belong to
        //e.g. (q0, 110) -> (q0,001)
        //But there should still be a connection using these rules, even if the LHS and RHS are not the last characters used
        //e.g. (q0, 1101) -> (q0, 0011)
        //The code below performs those additions
        let mut old_len = 0;
        //If there are no new edges since the last time, there are no new possible edges
        //as the code would run the exact same as last time, and not add anything
        while old_len < rule_graph.edge_count() {
            let new_len = rule_graph.edge_count();
            //Iterating through all of the new edges (this is not endorsed by petgraph as a method, but it works 100% of the time)
            for edge_idx in old_len..new_len {
                //the weight is the metadata related to the path
                let old_weight = rule_graph.raw_edges()[edge_idx].weight.clone();
                let old_parent = rule_graph[rule_graph.raw_edges()[edge_idx].source()];
                let old_child = rule_graph[rule_graph.raw_edges()[edge_idx].target()];

                //For each symbol after the old LHS and RHS state, check to see if adding one symbol to both adds a new path
                //e.g. If the old rule is (q0,x) -> (q0,y)
                //then we add the edges (q0, x0) -> (q0, y0), (q0,x1) -> (q0, y1)
                //x0/y0 is concatenated, e.g. if x = 100 then x0 = 1001
                for sym in 0..self.get_ruleset().symbol_set.length {
                    let new_parent = possible_dfa.state_transitions[old_parent][sym as usize];
                    let new_child = possible_dfa.state_transitions[old_child][sym as usize];
                    if !rule_graph
                        .contains_edge(NodeIndex::new(new_parent), NodeIndex::new(new_child))
                    {
                        rule_graph.add_edge(
                            NodeIndex::new(new_parent),
                            NodeIndex::new(new_child),
                            old_weight.clone(),
                        );
                    }
                }
            }
            old_len = new_len;
        }
        rule_graph
    }
    fn is_superset<'a>(
        &'a self,
        test_dfa: &'a DFA,
    ) -> Result<(), Option<(RuleGraphRoot<'a>, usize, usize)>> {
        if !(test_dfa >= self.get_goal()) {
            return Err(None);
        }
        let rule_graph = self.build_rule_graph(test_dfa);
        for edge in rule_graph.edge_references() {
            if !test_dfa.accepting_states[edge.source().index()]
                && test_dfa.accepting_states[edge.target().index()]
            {
                return Err(Some((
                    edge.weight().clone(),
                    edge.source().index(),
                    edge.target().index(),
                )));
            }
        }
        Ok(())
    }
    fn build_no_rule_dfa(&self) -> DFA {
        if self.get_ruleset().rules.contains_key(&vec![]) {
            DFA {
                accepting_states: Vec::new(),
                starting_state: 0,
                state_transitions: vec![vec![0; self.get_goal().symbol_set.length]],
                symbol_set: self.get_goal().symbol_set.clone(),
            }
        } else {
            let mut state_buffers: Vec<Vec<SymbolIdx>> = vec![vec![SymbolIdx::MAX], vec![]];

            let mut state_transitions = vec![vec![0; self.get_goal().symbol_set.length]; 2];
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
                                if lhs.len() > new_buffer.len()
                                    && lhs[..new_buffer.len()] == new_buffer
                                {
                                    match_found = true;
                                    break;
                                }
                            }
                            if !match_found {
                                new_buffer.remove(0);
                            }
                        }
                        match_found = false;
                        for (idx, buffer) in state_buffers.iter().enumerate() {
                            if &new_buffer == buffer {
                                match_found = true;
                                state_transitions[new_state][new_buffer_sym] = idx;
                                break;
                            }
                        }
                        if !match_found {
                            state_buffers.push(new_buffer);
                            state_transitions[new_state][new_buffer_sym] = state_transitions.len();
                            state_transitions.push(vec![0; self.get_goal().symbol_set.length])
                        }
                    }
                }
            }

            let mut temp_accepting = vec![true; state_transitions.len()];
            temp_accepting[0] = false;
            DFA {
                accepting_states: temp_accepting,
                starting_state: 1,
                state_transitions: state_transitions,
                symbol_set: self.get_goal().symbol_set.clone(),
            }
        }
    }
    fn build_path_graph(&self, possible_dfa: &DFA) -> Vec<Vec<Path>> {
        let hack_lambda = |list: &Vec<usize>| {
            let mut start = "[".to_owned();
            for (idx, i) in list.iter().enumerate() {
                start.push('q');
                start.push_str(&i.to_string());
                if idx + 1 < list.len() {
                    start.push(',');
                    start.push(' ');
                }
            }
            start.push(']');
            start
        };
        let mut new_paths = vec![(possible_dfa.starting_state, 0)];
        let mut old_paths = vec![];
        let mut paths = vec![vec![]; possible_dfa.state_transitions.len()];
        let mut iteration_num = 0;
        paths[possible_dfa.starting_state].push(Path {
            buffer: vec![],
            rhs_connections: vec![],
            buffer_origin: possible_dfa.starting_state,
            goal_state: self.get_goal().starting_state,
        });
        while !new_paths.is_empty() {
            std::mem::swap(&mut old_paths, &mut new_paths);
            new_paths.clear();
            println!("# Iteration {}", iteration_num);
            if iteration_num > 0 {
                println!(
                    "Everything created in Iteration {} will now be tested.",
                    iteration_num - 1
                )
            } else {
                println!("We start with the starting state and no buffer.");
            }

            iteration_num += 1;
            for old_path in &old_paths {
                println!(
                    "## Testing {{Origin:q{}, Buffer:{:?}, LHS: q{}, RHS: {}}}",
                    paths[old_path.0][old_path.1].buffer_origin,
                    paths[old_path.0][old_path.1].buffer,
                    old_path.0,
                    hack_lambda(&paths[old_path.0][old_path.1].rhs_connections)
                );
                for symbol in 0..possible_dfa.symbol_set.length {
                    println!(
                        "### On {}:",
                        self.get_goal().symbol_set.representations[symbol]
                    );
                    let mut new_buffer = paths[old_path.0][old_path.1].buffer.clone();
                    new_buffer.push(symbol as SymbolIdx);

                    let new_goal_state = self.get_goal().state_transitions
                        [paths[old_path.0][old_path.1].goal_state][symbol];

                    let mut new_path = Path {
                        buffer: new_buffer,
                        rhs_connections: vec![],
                        buffer_origin: paths[old_path.0][old_path.1].buffer_origin,
                        goal_state: new_goal_state,
                    };

                    //Find where this path would be added
                    let dest_idx = possible_dfa.state_transitions[old_path.0][symbol];
                    println!(
                        "    New LHS is q{} ( (q{}, {}) -> q{} )",
                        dest_idx, old_path.0, symbol, dest_idx
                    );
                    //Follow all old pure links

                    if paths[old_path.0][old_path.1].rhs_connections.len() > 0 {
                        print!("#### Adding {} to all RHS connections: ", symbol)
                    }
                    for old_rhs_connection in &paths[old_path.0][old_path.1].rhs_connections {
                        let new_connection =
                            possible_dfa.state_transitions[*old_rhs_connection][symbol];
                        print!(
                            "( (q{}, {}) -> q{} ), ",
                            old_rhs_connection, symbol, new_connection
                        );
                        if !new_path.rhs_connections.contains(&new_connection) {
                            new_path.rhs_connections.push(new_connection);
                            new_path.rhs_connections.sort();
                        }
                    }
                    if paths[old_path.0][old_path.1].rhs_connections.len() > 0 {
                        println!("");
                    }
                    println!(
                        "    The new buffer is {:?} ({:?} + {})",
                        &new_path.buffer[..],
                        &new_path.buffer[..(new_path.buffer.len() - 1)],
                        symbol
                    );
                    //If any group of our buffer is the lhs side of a rule, then obviously a rule can be performed
                    //I.e. if buffer is 1,1,0,0,2
                    //we check 1,1,0,0,2 & 1,0,0,2 & 0,0,2 & 0,2 & 2
                    for i in 0..new_path.buffer.len() {
                        //If a buffer matches a left-hand side, determine all rhs states could be reached by the origin + buffer after a single rule application
                        match self.get_ruleset().rules.get(&new_path.buffer[i..]) {
                            Some(rhs_list) => {
                                //Find where the origin where the lhs substring would begin
                                let mut relevant_origin =
                                    paths[old_path.0][old_path.1].buffer_origin;
                                for j in 0..i {
                                    relevant_origin = possible_dfa.state_transitions
                                        [relevant_origin]
                                        [new_path.buffer[j] as usize];
                                }
                                for rhs in rhs_list {
                                    let mut rhs_end_idx = relevant_origin;
                                    println!("     Last {} elements of buffer are {:?}, which can become {:?}:",new_path.buffer.len() - i, &new_path.buffer[i..], rhs);
                                    print!("     Following RHS side from origin: ");
                                    for rhs_element in rhs {
                                        print!("(q{},{}) -> ", rhs_end_idx, rhs_element);
                                        rhs_end_idx = possible_dfa.state_transitions[rhs_end_idx]
                                            [*rhs_element as usize];
                                        print!("q{} ", rhs_end_idx);
                                    }
                                    println!("\n");
                                    if !new_path.rhs_connections.contains(&rhs_end_idx) {
                                        print!(
                                            "      Because (q{},{:?}) -> q{}",
                                            relevant_origin,
                                            &new_path.buffer[i..],
                                            dest_idx
                                        );
                                        println!(" and (q{},{:?}) -> q{}, we know that q{} -> q{} is a part of this path (adding to RHS)",relevant_origin,&rhs_list,rhs_end_idx, dest_idx, rhs_end_idx);
                                        new_path.rhs_connections.push(rhs_end_idx);
                                        new_path.rhs_connections.sort();
                                    } else {
                                        println!("      q{} is already in the RHS connections, so this is redundant",rhs_end_idx);
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
                            if lhs.len() > new_path.buffer.len()
                                && lhs[..new_path.buffer.len()] == new_path.buffer
                            {
                                match_found = true;
                                break;
                            }
                        }
                        if !match_found {
                            //Move
                            print!("    First element of buffer is unnecessary (no LHS starts with {:?} + a single character) ",new_path.buffer);
                            let unnecesary_char = new_path.buffer.remove(0);
                            print!("-- reducing buffer to {:?}", new_path.buffer);
                            let temp = new_path.buffer_origin;
                            new_path.buffer_origin = possible_dfa.state_transitions
                                [new_path.buffer_origin][unnecesary_char as usize];
                            println!(
                                ", making origin q{} ( (q{},{}) -> q{} )",
                                new_path.buffer_origin,
                                temp,
                                unnecesary_char,
                                new_path.buffer_origin
                            );
                        }
                    }

                    if !paths[dest_idx].contains(&new_path) {
                        println!("\n##### {{Origin:q{}, Buffer:{:?}, LHS: q{}, RHS: {}}} is new! Added to paths",new_path.buffer_origin,new_path.buffer,dest_idx,hack_lambda(&new_path.rhs_connections));
                        new_paths.push((dest_idx, paths[dest_idx].len()));
                        paths[dest_idx].push(new_path);
                    } else {
                        println!("\n##### {{Origin:q{}, Buffer:{:?}, LHS: q{}, RHS: {}}} already exists -- nothing new created here",new_path.buffer_origin,new_path.buffer,dest_idx,hack_lambda(&new_path.rhs_connections));
                    }
                }
            }
        }
        paths
    }
    //I'm hoping to give more nuanced proofs/proof failures soon
    //An audit trail (maybe integrated with that massive excel sheet I made) would be ideal
    fn is_correct(&self, possible_dfa: &DFA) -> bool {
        let no_rule_dfa = self.build_no_rule_dfa();
        //If the set of terminal strings is not correct in the possible_dfa
        if &no_rule_dfa & self.get_goal() != &no_rule_dfa & possible_dfa {
            //Throw the whole thing out!
            return false;
        }
        //Make sure that terminal states have their own state associated with them.
        let expanded_dfa = possible_dfa.dfa_product(&no_rule_dfa, |s, _o| *s);

        //Ensure that there are no cycles in the DFA (if they exist, proof fails & it is guaranteed that DFA is not minimal)
        let rule_graph = self.build_rule_graph(&expanded_dfa);
        if petgraph::algo::is_cyclic_directed(&rule_graph) {
            return false;
        }

        let path_graph = self.build_path_graph(&expanded_dfa);

        //Because we only care about whether or not there's a single issue, the ordering of the proof doesn't matter
        //All states can assume that the others are all correct -- if one fails, it's not correct either way
        //And if all are correct, then we right to do it in an unordered fashion
        for (state_idx, state_paths) in path_graph.iter().enumerate() {
            //If the state is supposed to be accepting
            if expanded_dfa.accepting_states[state_idx] {
                for path in state_paths {
                    if !self.get_goal().accepting_states[path.goal_state] //If the path is not a part of the goal regex
                       && path.rhs_connections.iter().all(|f| *f != state_idx) //And the path is not looping
                       && path.rhs_connections.iter().all(|f| !expanded_dfa.accepting_states[*f])
                    //And can only go to provably rejecting strings
                    {
                        return false;
                    }
                }
            } else {
                for path in state_paths {
                    if self.get_goal().accepting_states[path.goal_state] || //If the path is a part of the goal regex
                       path.rhs_connections.iter().any(|f|  expanded_dfa.accepting_states[*f])
                    //or can go to an accepting state
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn correct_audit<'a>(&self, possible_dfa: &DFA, emit_steps: bool) -> ProofAudit {
        //Make sure that all terminal states have their own state associated with them.
        let no_rule_dfa = self.build_no_rule_dfa();
        let expanded_dfa = possible_dfa.dfa_product(&no_rule_dfa, |s, _o| *s);

        let path_graph = self.build_path_graph(&expanded_dfa);

        let mut audit = ProofAudit {
            steps: vec![],
            properties: vec![],
        };

        //Add in all of the states into our proof
        for i in 0..expanded_dfa.state_transitions.len() {
            audit.add_step(
                ProofStep {
                    element: ProofElement::State(i),
                    sources: HashSet::new(),
                    reason: ProofStepRationale::Assertion,
                },
                emit_steps,
            );
        }
        //Record nature of all accepting states
        audit.add_step(
            ProofStep {
                element: ProofElement::AddProperty(
                    ProofProperty::Accepting(true),
                    expanded_dfa
                        .accepting_states
                        .iter()
                        .enumerate()
                        .filter(|(_, &r)| r)
                        .map(|(index, _)| index)
                        .collect::<Vec<_>>(),
                ),
                sources: HashSet::new(),
                reason: ProofStepRationale::Assertion,
            },
            emit_steps,
        );

        let mut rejecting_states = vec![];
        for i in 0..expanded_dfa.state_transitions.len() {
            if !expanded_dfa.accepting_states[i] {
                rejecting_states.push(i);
            }
        }
        //Record nature of all rejecting states
        audit.add_step(
            ProofStep {
                element: ProofElement::AddProperty(
                    ProofProperty::Accepting(false),
                    rejecting_states,
                ),
                sources: HashSet::new(),
                reason: ProofStepRationale::Assertion,
            },
            emit_steps,
        );

        //Add in all of the paths into our proof
        for i in 0..expanded_dfa.state_transitions.len() {
            for path in &path_graph[i] {
                let mut sources_set = HashSet::from_iter(path.rhs_connections.clone());
                sources_set.insert(i);
                //The path graph produces what would be considered duplicates in this context. This is to make sure no duplicates exist
                if let None =
                    audit.find_element(ProofElement::Path(i, path.rhs_connections.clone()))
                {
                    audit.add_step(
                        ProofStep {
                            reason: ProofStepRationale::Assertion,
                            element: ProofElement::Path(i, path.rhs_connections.clone()),
                            sources: sources_set,
                        },
                        emit_steps,
                    );
                    //Add rejecting/accepting nature into proof (technically, this isn't an assertion, but w/e)
                    audit.add_step(
                        ProofStep {
                            reason: ProofStepRationale::Assertion,
                            element: ProofElement::AddProperty(
                                ProofProperty::Accepting(expanded_dfa.accepting_states[i]),
                                vec![audit.len() - 1],
                            ),
                            sources: HashSet::from_iter(vec![i]),
                        },
                        emit_steps,
                    );
                }
            }
        }

        //Next, find all states that can be proven correct bc they either 1) match the goal regex perfectly or
        // 2) are terminal, and have no strings that match the goal regex
        let mut regex_clone = expanded_dfa.clone(); //This is a modifiable version of the DFA used for checking individual states
        let mut regex_correct_states = vec![]; //This stores a list of all proven states for later
        for i in 0..expanded_dfa.state_transitions.len() {
            let mut temp_accepting = vec![false; expanded_dfa.state_transitions.len()];
            temp_accepting[i] = true;
            regex_clone.accepting_states = temp_accepting;
            //If the state is accepting and all strings in that state are accepted by the goal regex
            let accepting_correct =
                expanded_dfa.accepting_states[i] && &regex_clone <= self.get_goal();
            //If the state is rejecting, terminal and all strings in that states are not accepted by the goal regex
            let is_terminal = path_graph[i].iter().all(|f| f.rhs_connections.len() == 0);
            let rejecting_correct = is_terminal
                && !expanded_dfa.accepting_states[i]
                && &(self.get_goal() & &!&regex_clone) == self.get_goal();
            //&(self.get_goal() - &regex_clone) == self.get_goal() is equivalent to regex_clone.intersection(self.get_goal) == empty_set
            //But I haven't made an empty set constant for the DFA yet.

            //If the state can be guaranteed correct
            if accepting_correct || rejecting_correct {
                regex_correct_states.push(i);
                //Inform that it's correct
                audit.add_step(
                    ProofStep {
                        reason: ProofStepRationale::MatchesRegex,
                        element: ProofElement::AddProperty(ProofProperty::Correct, vec![i]),
                        sources: HashSet::new(),
                    },
                    emit_steps,
                );

                //Inform that all paths are also correct (by virtue of the state being correct)
                let mut path_idxs = HashSet::new();
                for path in &path_graph[i] {
                    path_idxs.insert(
                        audit
                            .find_element(ProofElement::Path(i, path.rhs_connections.clone()))
                            .unwrap(),
                    );
                }
                audit.add_step(
                    ProofStep {
                        reason: ProofStepRationale::EquivalentSet,
                        element: ProofElement::AddProperty(
                            ProofProperty::Correct,
                            path_idxs.into_iter().collect(),
                        ),
                        sources: HashSet::from_iter(vec![audit.len() - 1]),
                    },
                    emit_steps,
                );
            }
        }
        //The idea here is that when we prove something correct, we check to see if anything can be proven based on that by adding it
        //This will then iterate through all provable elements, even if some part of the DFA is unprovable.
        //We start by getting all paths where at least one of the RHS connections has been proven correct in the previous step.
        let mut possibly_provable: VecDeque<ProofIndex> = VecDeque::new();
        for (state_path_idx, state_paths) in path_graph.iter().enumerate() {
            for path in state_paths {
                if path
                    .rhs_connections
                    .iter()
                    .any(|f| regex_correct_states.contains(f))
                {
                    let path_index = audit
                        .find_element(ProofElement::Path(
                            state_path_idx,
                            path.rhs_connections.clone(),
                        ))
                        .unwrap();
                    if !possibly_provable.contains(&path_index) {
                        possibly_provable.push_back(path_index)
                    }
                }
            }
        }

        while let Some(cur_index) = possibly_provable.pop_back() {
            //If this has been proven elsewhere, forget about it!
            if audit.properties[cur_index].contains(&ProofProperty::Correct) {
                continue;
            }
            //If the ProofElement in question is a state
            if let ProofElement::State(idx) = audit.steps[cur_index].element {
                //The only way a state can be proven is if all constituent paths have been proven.
                //This section of the code checks to see if all paths have been proven correct.

                //Check which paths have been proven for the state
                let mut proven_path_idxs = vec![];
                for path in &path_graph[idx] {
                    let path_idx = audit
                        .find_element(ProofElement::Path(idx, path.rhs_connections.clone()))
                        .unwrap();
                    if audit.get_props(path_idx).contains(&ProofProperty::Correct) {
                        proven_path_idxs.push(path_idx);
                    } else {
                        break;
                    }
                }
                //If every path for the state has been proven
                if proven_path_idxs.len() == path_graph[idx].len() {
                    //Add the fact that the state was proven to the record
                    audit.add_step(
                        ProofStep {
                            reason: ProofStepRationale::EquivalentSet,
                            element: ProofElement::AddProperty(
                                ProofProperty::Correct,
                                vec![cur_index],
                            ),
                            sources: HashSet::from_iter(proven_path_idxs),
                        },
                        emit_steps,
                    );

                    //Add every path connected to this state to the possibly provable elements
                    for (state_path_idx, state_paths) in path_graph.iter().enumerate() {
                        for path in state_paths {
                            if path.rhs_connections.contains(&idx) {
                                let path_index = audit
                                    .find_element(ProofElement::Path(
                                        state_path_idx,
                                        path.rhs_connections.clone(),
                                    ))
                                    .unwrap();
                                //If it's unproven and not already in the stack
                                if !audit
                                    .get_props(path_index)
                                    .contains(&ProofProperty::Correct)
                                    && !possibly_provable.contains(&path_index)
                                {
                                    possibly_provable.push_back(path_index)
                                }
                            }
                        }
                    }
                }
            } else if let ProofElement::Path(lhs, rhs_connections) =
                audit.steps[cur_index].element.clone()
            {
                let mut is_proven = false;
                //There are a couple ways that a state can be correct:
                // 1. It is accepting, and at least one of the states it can go is provably accepting
                // 2. It is a rejecting exit path, and it can only go to provably rejecting states
                // 3. It is an accepting looping path, and all exit paths are correct
                // 4. It is a rejecting looping path, all exit paths are correct, and all looping paths can only go to provably rejecting states (or itself)

                //This code's control flow takes advantage of a couple things:
                // a. 1 can be true for any accepting state (and is simpler to check) so we default to that when possible
                // b. 4 is simply a more restrictive version of 3
                // c. if 3. or 4. is proven for one looping path, it will also be true for all unproven looping paths

                //If it is accepting (Checking for 1.)
                if expanded_dfa.accepting_states[lhs] {
                    //For all states it can go to
                    for state in &rhs_connections {
                        let state_props = audit.get_props(*state);
                        //Are any accepting?
                        if state_props.contains(&ProofProperty::Correct)
                            && state_props.contains(&ProofProperty::Accepting(true))
                        {
                            //If so, add to record and break
                            is_proven = true;
                            audit.add_step(
                                ProofStep {
                                    reason: ProofStepRationale::AcceptingExit,
                                    element: ProofElement::AddProperty(
                                        ProofProperty::Correct,
                                        vec![cur_index],
                                    ),
                                    sources: HashSet::from_iter(vec![*state]),
                                },
                                emit_steps,
                            );
                            break;
                        }
                    }
                } else if !rhs_connections.contains(&lhs) {
                    //If it is a rejecting exit path (Checking for 2.)
                    let mut proven_rejecting_states = 0;
                    for state in &rhs_connections {
                        let state_props = audit.get_props(*state);
                        if state_props.contains(&ProofProperty::Correct)
                            && state_props.contains(&ProofProperty::Accepting(false))
                        {
                            proven_rejecting_states += 1;
                        } else {
                            break;
                        }
                    }
                    if proven_rejecting_states == rhs_connections.len() {
                        audit.add_step(
                            ProofStep {
                                reason: ProofStepRationale::RejectingExit,
                                element: ProofElement::AddProperty(
                                    ProofProperty::Correct,
                                    vec![cur_index],
                                ),
                                sources: HashSet::from_iter(rhs_connections.clone()),
                            },
                            emit_steps,
                        );
                        is_proven = true;
                    }
                }
                //If it's an unproven looping path (3. and 4.)
                if !is_proven && rhs_connections.contains(&lhs) {
                    let mut exit_paths = 0;
                    let mut proven_exit_path_idxs = vec![];
                    let mut unproven_looping_path_idxs = vec![];

                    //Begin analyzing each path in the state
                    for possible_path in &path_graph[lhs] {
                        let path_idx = audit
                            .find_element(ProofElement::Path(
                                lhs,
                                possible_path.rhs_connections.clone(),
                            ))
                            .unwrap();

                        //If path is an exit path
                        if !possible_path.rhs_connections.contains(&lhs) {
                            exit_paths += 1;
                            if audit.get_props(path_idx).contains(&ProofProperty::Correct) {
                                proven_exit_path_idxs.push(path_idx);
                            } else {
                                break;
                            }
                        } else {
                            //If rejecting, requirement is that it can only go to provably rejecting states & self (Checking for 4.)
                            if !expanded_dfa.accepting_states[lhs] {
                                let mut all_provably_rejecting = true;
                                for possible_rhs in &possible_path.rhs_connections {
                                    if *possible_rhs == lhs {
                                        //Except for itself!
                                        continue;
                                    }
                                    let temp_props = audit.get_props(*possible_rhs);
                                    all_provably_rejecting &=
                                        temp_props.contains(&ProofProperty::Accepting(false));
                                    all_provably_rejecting &=
                                        temp_props.contains(&ProofProperty::Correct);
                                    if !all_provably_rejecting {
                                        //Hacky premature exit that guarantees it won't be considered correct
                                        exit_paths = 0;
                                        proven_exit_path_idxs.push(0);
                                        break;
                                    }
                                }
                                if !all_provably_rejecting {
                                    break;
                                }
                            }
                            if !audit.get_props(path_idx).contains(&ProofProperty::Correct) {
                                unproven_looping_path_idxs.push(path_idx)
                            }
                        }
                    }
                    //If all exit paths are proven (Checking for 3.) and our hacky premature exit hasn't occurred (Checking for 4.)
                    if exit_paths == proven_exit_path_idxs.len() {
                        is_proven = true;
                        let reason = if expanded_dfa.accepting_states[lhs] {
                            ProofStepRationale::AcceptingLooping
                        } else {
                            ProofStepRationale::RejectingLooping
                        };
                        //Add the corresponding path correctness to the record
                        audit.add_step(
                            ProofStep {
                                reason: reason,
                                element: ProofElement::AddProperty(
                                    ProofProperty::Correct,
                                    unproven_looping_path_idxs,
                                ),
                                sources: HashSet::from_iter(proven_exit_path_idxs),
                            },
                            emit_steps,
                        );
                    }
                }
                //If we have successfully proven the cur_index path correct, using any means (1.,2.,3., or 4.)
                if is_proven {
                    //If it's an exit path, make sure to check the looping paths
                    if !rhs_connections.contains(&lhs) {
                        for path in &path_graph[lhs] {
                            if path.rhs_connections.contains(&lhs) {
                                possibly_provable.push_back(
                                    audit
                                        .find_element(ProofElement::Path(
                                            lhs,
                                            path.rhs_connections.clone(),
                                        ))
                                        .unwrap(),
                                );
                            }
                        }
                    }
                    //Make sure to check to see if all paths in the state have been proven correct because of this
                    possibly_provable.push_back(lhs);
                }
            }
        }
        audit
    }
}

pub struct ProofAudit {
    steps: Vec<ProofStep>,
    properties: Vec<HashSet<ProofProperty>>,
}

#[derive(Debug, PartialEq, Eq)]
enum ProofStepRationale {
    Assertion,
    RejectingExit,
    AcceptingExit,
    RejectingLooping,
    AcceptingLooping,
    EquivalentSet,
    MatchesRegex,
}

type ProofIndex = usize;

#[derive(Debug, PartialEq, Eq, Clone)]
enum ProofElement {
    AddProperty(ProofProperty, Vec<ProofIndex>),
    State(usize),
    Path(ProofIndex, Vec<ProofIndex>),
}
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
enum ProofProperty {
    Accepting(bool),
    Correct,
}

#[derive(Debug, PartialEq, Eq)]
struct ProofStep {
    element: ProofElement,
    sources: HashSet<ProofIndex>,
    reason: ProofStepRationale,
}

impl std::fmt::Display for ProofAudit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in &self.steps {
            if let ProofElement::AddProperty(ref prop, ref affected) = i.element {
                write!(f, "Add Property {:?} to ", prop)?;
                for affect in affected {
                    write!(f, "{:?}", self.steps[*affect].element)?;
                    if affect != affected.last().unwrap() {
                        write!(f, ", ")?;
                    }
                }
                write!(f, " | ")?;
            } else {
                write!(f, "{:?} | ", i.element)?;
            };
            write!(f, "Using {:?} Rule | Reasons: ", i.reason)?;
            for source in &i.sources {
                write!(f, "{:?}, ", self.steps[*source].element)?;
            }
            write!(f, "\n")?;
        }
        Ok(())
    }
}

impl ProofAudit {
    fn add_step(&mut self, proof_step: ProofStep, should_emit: bool) {
        if let ProofElement::AddProperty(ref prop, ref affected) = proof_step.element {
            for i in affected {
                self.properties[*i].insert(prop.clone());
            }
        }
        if should_emit {
            println!("{:?}", proof_step);
        }
        self.steps.push(proof_step);
        self.properties.push(HashSet::new());
    }
    fn len(&self) -> usize {
        self.steps.len()
    }
    fn find_element(&self, desired_element: ProofElement) -> Option<usize> {
        self.steps.iter().position(|x| x.element == desired_element)
    }
    fn get_props(&self, step_idx: ProofIndex) -> HashSet<ProofProperty> {
        self.properties[step_idx].clone()
    }
}

#[derive(Debug, Clone)]
pub struct RuleGraphRoot<'a> {
    lhs: Vec<SymbolIdx>,
    rhs: Vec<SymbolIdx>,
    init_lhs_state: usize,
    init_rhs_state: usize,
    origin: usize,
    symset: &'a SymbolSet,
}

impl<'a> RuleGraphRoot<'a> {
    fn new(
        lhs: Vec<SymbolIdx>,
        rhs: Vec<SymbolIdx>,
        init_lhs_state: usize,
        init_rhs_state: usize,
        origin: usize,
        symset: &'a SymbolSet,
    ) -> Self {
        RuleGraphRoot {
            lhs: lhs,
            rhs: rhs,
            init_lhs_state: init_lhs_state,
            init_rhs_state: init_rhs_state,
            origin: origin,
            symset: symset,
        }
    }
}
impl<'a> std::fmt::Display for RuleGraphRoot<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LHS ")?;
        write!(f, "{}", self.symset.symbols_to_string(&self.lhs))?;
        write!(f, " | RHS ")?;
        write!(f, "{}", &self.symset.symbols_to_string(&self.rhs))?;
        write!(f, " | initial LHS state ")?;
        write!(f, "{}", &self.init_lhs_state.to_string())?;
        write!(f, " | initial RHS state ")?;
        write!(f, "{}", &self.init_rhs_state.to_string())?;
        write!(f, " | origin ")?;
        write!(f, "{}", &self.origin.to_string())
    }
}
#[derive(Debug)]
pub enum DomainError {
    Generating((Vec<SymbolIdx>, Vec<SymbolIdx>)),
    Deleting((Vec<SymbolIdx>, Vec<SymbolIdx>)),
    Cyclic((Vec<SymbolIdx>, Vec<SymbolIdx>)),
}

impl DomainError {
    pub fn to_string(&self, symset: &SymbolSet) -> String {
        let mut result = "Solver is incompatible with ".to_owned();
        match &self {
            DomainError::Generating((lhs, rhs)) => result.push_str(&format!(
                "generating rules. SRS contains generating rule \"{} - {}\"",
                symset.symbols_to_string(&lhs),
                symset.symbols_to_string(&rhs)
            )),
            DomainError::Deleting((lhs, rhs)) => result.push_str(&format!(
                "deleting rules. SRS contains deleting rule \"{} - {}\".",
                symset.symbols_to_string(&lhs),
                symset.symbols_to_string(&rhs)
            )),
            DomainError::Cyclic((lhs, rhs)) => result.push_str(&format!(
                "cyclic rules. Rules \"{0} - {1}\" and \"{1} - {0}\" create a cycle.",
                symset.symbols_to_string(&lhs),
                symset.symbols_to_string(&rhs)
            )),
        }
        result
    }
}

#[derive(PartialEq, Eq, Clone)]
pub struct Path {
    buffer: Vec<SymbolIdx>,
    rhs_connections: Vec<usize>,
    buffer_origin: usize,
    goal_state: usize,
}
