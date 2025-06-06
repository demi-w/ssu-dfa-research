use std::collections::HashSet;

use std::sync::{Arc, Mutex, RwLock};

use rayon::prelude::*;

use bitvec::prelude::*;
use petgraph::{
    algo::{condensation, toposort},
    graph::{DiGraph, NodeIndex},
    visit::{Dfs, EdgeRef},
    Direction::{Incoming, Outgoing},
    Graph,
};

use crate::util::{Ruleset, SymbolIdx, DFA};

use super::{DFAStructure, Instant, SRSSolver, SSStructure, Solver};
use crate::solver::srssolver::DomainError;

#[derive(Debug, Clone, Default)]
struct SignatureSetElement {
    //Original elements of the signature set that this one node now represents
    original_idxs: Vec<usize>,
    //Pre-computed set of ancestors -- used under the assumption that pre-calculating this will ultimately make things way faster
    //assumption is wrong -- memory complexity is ridiculous lol
    //precomputed_ancestors : HashSet<NodeIndex>,
    //DFA states that lead to an accepting string after walking through !!any!! of the original elements for this node
    //Deprecated in favor of goal_minkids in SRS translator
    //accepting_states : Vec<usize>
}

#[derive(Clone)]
pub struct MinkidSolver {
    pub goal: DFA,
    pub rules: Ruleset,
    pub max_input: usize,
    pub min_input: usize,
    //For each state of the goal DFA, what would its hypothetical minkid set look like?
    //Used as the basis for propagation in the minkid method
    goal_minkids: Vec<HashSet<NodeIndex>>,

    //Link graph of signature set elements
    ss_link_graph: DiGraph<SignatureSetElement, ()>,

    //Lookup table of where individual ss elements ended up in the graph
    ss_idx_to_link: Vec<NodeIndex>,
}

struct MKDFAState {
    minkids: HashSet<NodeIndex>,
    goal_states: Vec<usize>,
}

impl SRSSolver for MinkidSolver {
    fn get_goal(&self) -> &DFA {
        &self.goal
    }

    fn new(mut ruleset: Ruleset, mut goal: DFA) -> Result<Self, DomainError> {
        Self::ensure_expansion(&mut ruleset, &mut goal);
        let (min_input, max_input) = MinkidSolver::sized_init(&ruleset);
        Ok(MinkidSolver {
            goal: goal,
            rules: ruleset,
            max_input: max_input,
            min_input: min_input,
            goal_minkids: vec![],
            ss_link_graph: Graph::new(),
            ss_idx_to_link: vec![],
        })
    }

    fn get_ruleset(&self) -> &Ruleset {
        &self.rules
    }
}

impl Solver for MinkidSolver {
    fn get_symset(&self) -> &crate::SymbolSet {
        &self.rules.symbol_set
    }
    const PHASES: &'static [&'static str] = &[
        "Build rule graph",
        "Propagate pure links",
        "Propagate minkids",
        "Remove duplicates",
    ];

    fn evaluate<'a, 'b>(&'a self, state: &'b Vec<u8>) -> bool {
        todo!();
    }

    fn mutate(&self, state: Vec<u8>, input: SymbolIdx) -> Vec<u8> {
        SRSSolver::mutate(self, state, input)
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
        //graph of connections based on LHS->RHS links for all states
        //Usize is index in trans_table

        assert!(
            core::cmp::PartialEq::<Vec<u8>>::eq(&origin, &vec![]),
            "doesn't support non-null origin (sorry!)"
        );

        let sig_set = &self.rules.symbol_set.build_sig_k(sig_k);
        self.build_ss_link_graph(sig_set);
        let real_self = Arc::new(self);
        let mut dfa_graph = DiGraph::<RwLock<MKDFAState>, SymbolIdx>::new();
        let mut link_graph = DiGraph::<(), (Vec<SymbolIdx>, Vec<SymbolIdx>)>::new();
        dfa_graph.add_node(RwLock::new(MKDFAState {
            minkids: real_self.goal_minkids[real_self.goal.starting_state].clone(),
            goal_states: vec![real_self.goal.starting_state],
        }));
        link_graph.add_node(());
        //number of nodes after an iteration.
        //Each iteration only works if there are two lengths -- so we start with two.
        let mut iteration_lens = vec![0, 1];

        if is_debug {
            let _ = phase_events.send(Instant::now() - init_begin_time);
        }

        //While new elements are actually getting added to the DFA
        while iteration_lens[iteration_lens.len() - 2] < iteration_lens[iteration_lens.len() - 1] {
            if is_debug {
                dfa_events
                    .send(real_self.translate_to_debug(&dfa_graph, &sig_set))
                    .unwrap();
            }
            let mut last_time = Instant::now();
            //First, adding all prospective DFA elements
            //This only adds nodes to the most recent iteration of DFA elements
            for start_idx in
                iteration_lens[iteration_lens.len() - 2]..iteration_lens[iteration_lens.len() - 1]
            {
                //Root node that prospective state will be connected to
                let start_node = NodeIndex::new(start_idx);
                for next_sym in 0..(real_self.rules.symbol_set.length as SymbolIdx) {
                    //states that the prospective state can reach into the goal DFA
                    let mut goal_connections = vec![];
                    //Set of minimum kids that can be added without any SRS applications.
                    let mut minkids = HashSet::new();
                    //For each goal DFA state that the root node can reach,
                    for start_connection in &dfa_graph[start_node].write().unwrap().goal_states {
                        //Add where the DFA goes after the input symbol that defines the connection between root and prospective DFA state
                        let new_connect =
                            real_self.goal.state_transitions[*start_connection][next_sym as usize];
                        //Don't add anything twice. Seems like that'd be trouble.
                        if !goal_connections.contains(&new_connect) {
                            //If that connection to the goal DFA hasn't already been made,
                            //add to our vec of reachable states,
                            goal_connections.push(new_connect);

                            //And add the minkids that don't require SRS applications.
                            if minkids.is_empty() {
                                minkids = real_self.goal_minkids[new_connect].clone();
                            } else {
                                real_self.add_set_to_minkids(
                                    &mut minkids,
                                    &real_self.goal_minkids[new_connect],
                                );
                            }
                        }
                    }
                    let new_node = dfa_graph.add_node(RwLock::new(MKDFAState {
                        minkids: minkids,
                        goal_states: goal_connections,
                    }));
                    link_graph.add_node(());
                    dfa_graph.add_edge(start_node, new_node, next_sym);
                }
            }

            //Next, we BUILD the LINK GRAPH !!! (this should inspire fear)
            //This also has some major room for efficiency improvements imo
            //but it wasn't really noticeable for the subset implementation?
            //will check perf later (ofc)

            //The range here can only possibly include elements max_input away from the diameter,
            //as otherwise we know that any connections they possess must have been added before.
            //In fact, this should probably be abused to cull old elements that cannot possibly add new info
            //But again... that's for later! ... and it assumes I don't end up building something completely new again :/
            let mut underflow_dodge = 0;
            if real_self.max_input < iteration_lens.len() - 1 {
                underflow_dodge = iteration_lens.len() - real_self.max_input - 1;
            }
            for start_idx in
                iteration_lens[underflow_dodge]..iteration_lens[iteration_lens.len() - 1]
            {
                //Root node that prospective state will be connected to
                let start_node = NodeIndex::new(start_idx);
                for rule_list in &real_self.rules.rules {
                    let lhs_str = rule_list.0;
                    for rhs_str in rule_list.1 {
                        let mut lhs = vec![start_node];
                        let mut rhs = vec![start_node];
                        let mut p_rule_len = 1;
                        while p_rule_len <= lhs_str.len() || p_rule_len <= rhs_str.len() {
                            let mut potential_lhs_e = vec![];

                            for lhs_e in &lhs {
                                if p_rule_len <= lhs_str.len() {
                                    if let Some(e) = dfa_graph
                                        .edges_directed(*lhs_e, Outgoing)
                                        .find(|x| *x.weight() == lhs_str[p_rule_len - 1])
                                    {
                                        potential_lhs_e.push(e)
                                    }
                                } else {
                                    potential_lhs_e.append(
                                        &mut dfa_graph.edges_directed(*lhs_e, Outgoing).collect(),
                                    );
                                };
                            }

                            let mut potential_rhs_e = vec![];

                            for rhs_e in &rhs {
                                if p_rule_len <= rhs_str.len() {
                                    if let Some(e) = dfa_graph
                                        .edges_directed(*rhs_e, Outgoing)
                                        .find(|x| *x.weight() == rhs_str[p_rule_len - 1])
                                    {
                                        potential_rhs_e.push(e)
                                    }
                                } else {
                                    potential_rhs_e.append(
                                        &mut dfa_graph.edges_directed(*rhs_e, Outgoing).collect(),
                                    );
                                };
                            }
                            if potential_lhs_e.is_empty() || potential_rhs_e.is_empty() {
                                break;
                            }
                            lhs.clear();
                            rhs.clear();
                            let lhs_strip = if p_rule_len < lhs_str.len() {
                                &lhs_str[p_rule_len..]
                            } else {
                                &lhs_str[..0]
                            };
                            let rhs_strip = if p_rule_len < rhs_str.len() {
                                &rhs_str[p_rule_len..]
                            } else {
                                &rhs_str[..0]
                            };
                            for potential_lhs_edge in &potential_lhs_e {
                                for potential_rhs_edge in &potential_rhs_e {
                                    real_self.add_link(
                                        &mut link_graph,
                                        potential_lhs_edge.target(),
                                        potential_rhs_edge.target(),
                                        lhs_strip,
                                        rhs_strip,
                                    );
                                }
                            }
                            for potential_lhs_edge in potential_lhs_e {
                                lhs.push(potential_lhs_edge.target());
                            }
                            for potential_rhs_edge in potential_rhs_e {
                                rhs.push(potential_rhs_edge.target());
                            }
                            p_rule_len += 1;
                        }
                    }
                }
            }
            if is_debug {
                let dur = last_time.elapsed();
                phase_events.send(dur).unwrap();
                last_time = Instant::now();
            }
            //Realized I am dumb as bricks! We need to propagate pure connections!!!
            //DUH!!!!!
            //Currently crawls the entire fucking link graph bc i am dumb and tired and really curious
            for start_idx in 0..iteration_lens[iteration_lens.len() - 1] {
                let start_node = NodeIndex::new(start_idx);
                let mut possible_edge = link_graph.first_edge(start_node, Outgoing);
                while let Some(real_edge) = possible_edge {
                    possible_edge = link_graph.next_edge(real_edge, Outgoing);
                    let target = link_graph.edge_endpoints(real_edge).unwrap().1;
                    if !(link_graph[real_edge].0.is_empty()
                        && link_graph[real_edge].1.is_empty()
                        && target.index() < iteration_lens[iteration_lens.len() - 1])
                    {
                        continue;
                    }
                    let lhs = start_node;
                    let rhs = target;
                    let mut propagation_pairs = vec![(lhs, rhs)];
                    while let Some(prop_pair) = propagation_pairs.pop() {
                        for sym in 0..real_self.rules.symbol_set.length {
                            let lhs_extension;
                            let rhs_extension;
                            if let Some(lhs_edge) = dfa_graph
                                .edges_directed(prop_pair.0, Outgoing)
                                .find(|x| *x.weight() == sym as SymbolIdx)
                            {
                                lhs_extension = lhs_edge.target();
                            } else {
                                continue;
                            }
                            if let Some(rhs_edge) = dfa_graph
                                .edges_directed(prop_pair.1, Outgoing)
                                .find(|x| *x.weight() == sym as SymbolIdx)
                            {
                                rhs_extension = rhs_edge.target();
                            } else {
                                continue;
                            }
                            if lhs == lhs_extension && rhs == rhs_extension {
                                continue;
                            }
                            if real_self
                                .add_link(
                                    &mut link_graph,
                                    lhs_extension,
                                    rhs_extension,
                                    &vec![][..],
                                    &vec![][..],
                                )
                                .0
                            {
                                link_graph.add_edge(lhs_extension, rhs_extension, (vec![], vec![]));
                                propagation_pairs.push((lhs_extension, rhs_extension));
                            }
                        }
                    }
                }
            }

            if is_debug {
                let dur = last_time.elapsed();
                phase_events.send(dur).unwrap();
                last_time = Instant::now();
            }
            /*
            let mut debug_link_graph : DiGraph<String,(Vec<SymbolIdx>,Vec<SymbolIdx>)> = Graph::new();
            for i in 0..link_graph.node_count() {
                if i < *iteration_lens.last().unwrap() {
                    debug_link_graph.add_node(format!("Known q{}",i));
                } else{
                    debug_link_graph.add_node(format!("{} from q{}",
                        (i- *iteration_lens.last().unwrap())%self.rules.symbol_set.length,
                        ((i- *iteration_lens.last().unwrap())/self.rules.symbol_set.length + iteration_lens[iteration_lens.len() - 2])));
                }
            }
            for edge in link_graph.raw_edges() {
                debug_link_graph.add_edge(edge.source(), edge.target(), edge.weight.clone());
            }

            let mut file = std::fs::File::create(format!("link_graph_debug/{}.dot",iteration_lens.len()-2)).unwrap();
            file.write_fmt(format_args!("{:?}",petgraph::dot::Dot::new(&debug_link_graph))); */

            /*
                code to initially go through changes in reverse topological order.
                This is tragically less efficient because in most examples, every single prospective element
                is grouped into one big strongly connected component, making topological order useless.
                let mut tarjan = TarjanScc::new();

                tarjan.run(&link_graph, |x| {
                    for prospective_node in x {
                        if prospective_node.index() < *iteration_lens.last().unwrap() {
                            continue;
                        }
                        for edge in link_graph.edges_directed(*prospective_node, Outgoing) {
                            //If it modifies its source
                            if self.partial_link(&mut dfa_graph, sig_set, edge.weight(), edge.source(), edge.target()) {
                                affected_nodes.insert(*prospective_node);
                            }
                        }
                        nodes_used += 1;
                    }
                });
            */

            //Alright, pretending/assuming that we've written that correctly, we move on to actually propagating ancestors!
            //this also sucks :(
            //Just to get the ball rolling, we run through everything new once
            let mut affected_nodes = Arc::new(Mutex::new(HashSet::new()));
            let dfa_arc = Arc::new(dfa_graph);
            let link_arc = Arc::new(link_graph);
            (*iteration_lens.last().unwrap()..dfa_arc.node_count())
                .into_par_iter()
                .for_each(|prospective_idx| {
                    let prospective_node = NodeIndex::new(prospective_idx);
                    for edge in link_arc.edges_directed(prospective_node, Outgoing) {
                        //If it modifies its source
                        if real_self.partial_link(
                            &dfa_arc,
                            sig_set,
                            edge.weight(),
                            edge.source(),
                            edge.target(),
                        ) {
                            affected_nodes.lock().unwrap().insert(prospective_node);
                        }
                    }
                });
            //Continue propagating changes until no more exist!
            //This propagation could be better (do not add things to new list if they haven't been executed in current loop is the main one off the dome)
            let mut old_affected_nodes = Arc::new(Mutex::new(HashSet::new()));
            while !affected_nodes.lock().unwrap().is_empty() {
                old_affected_nodes.lock().unwrap().clear();
                std::mem::swap(&mut old_affected_nodes, &mut affected_nodes);
                old_affected_nodes
                    .lock()
                    .unwrap()
                    .par_iter()
                    .for_each(|affected_node| {
                        for edge in link_arc.edges_directed(*affected_node, Incoming) {
                            //This should just be an optimization, as it implies an impossible thing. This is not why I have added it.
                            if edge.source().index() < *iteration_lens.last().unwrap() {
                                continue;
                            }
                            if real_self.partial_link(
                                &dfa_arc,
                                sig_set,
                                edge.weight(),
                                edge.source(),
                                *affected_node,
                            ) {
                                affected_nodes.lock().unwrap().insert(edge.source());
                            }
                        }
                    });
            }
            dfa_graph = Arc::into_inner(dfa_arc).unwrap();
            link_graph = Arc::into_inner(link_arc).unwrap();
            if is_debug {
                let dur = last_time.elapsed();
                phase_events.send(dur).unwrap();
                last_time = Instant::now();
            }

            //Now, prune duplicates. Notably, there's no implementation of Hash on HashSets (extremely surprising to me), so hopefully this garbo solution doesn't take forever
            let mut new_count = 0;
            let mut prospective_state = *iteration_lens.last().unwrap();
            while prospective_state < dfa_graph.node_count() {
                let pros_node = NodeIndex::new(prospective_state);
                let mut equivalent_known = None;

                for known_state in 0..(*iteration_lens.last().unwrap() + new_count) {
                    let state_node = NodeIndex::new(known_state);
                    if dfa_graph[pros_node].read().unwrap().minkids
                        == dfa_graph[state_node].read().unwrap().minkids
                    {
                        equivalent_known = Some(state_node);
                        break;
                    }
                }
                match equivalent_known {
                    Some(equiv) => {
                        //Re-link if there exists an equivalent state
                        let disappointed_parent_edge = dfa_graph
                            .edges_directed(pros_node, Incoming)
                            .last()
                            .unwrap();
                        dfa_graph.add_edge(
                            disappointed_parent_edge.source(),
                            equiv,
                            *disappointed_parent_edge.weight(),
                        );

                        //Remove the duplicate from the graph
                        dfa_graph.remove_node(pros_node);

                        //Make sure to preserve any connections in the link graph!
                        let mut potential_incoming = link_graph.first_edge(pros_node, Incoming);
                        while let Some(real_incoming) = potential_incoming {
                            potential_incoming = link_graph.next_edge(real_incoming, Incoming);
                            let source = link_graph.edge_endpoints(real_incoming).unwrap().0;
                            let rust_scared = link_graph[real_incoming].clone();
                            if real_self
                                .add_link(
                                    &mut link_graph,
                                    source,
                                    equiv,
                                    &rust_scared.0[..],
                                    &rust_scared.1[..],
                                )
                                .1
                            {
                                potential_incoming = link_graph.first_edge(pros_node, Incoming);
                            }
                        }
                        let mut potential_outgoing = link_graph.first_edge(pros_node, Outgoing);
                        while let Some(real_outgoing) = potential_outgoing {
                            potential_outgoing = link_graph.next_edge(real_outgoing, Outgoing);
                            let target = link_graph.edge_endpoints(real_outgoing).unwrap().1;
                            let rust_scared = link_graph[real_outgoing].clone();
                            if real_self
                                .add_link(
                                    &mut link_graph,
                                    equiv,
                                    target,
                                    &rust_scared.0[..],
                                    &rust_scared.1[..],
                                )
                                .1
                            {
                                potential_outgoing = link_graph.first_edge(pros_node, Outgoing);
                            }
                        }
                        link_graph.remove_node(pros_node);
                        prospective_state -= 1;
                    }
                    None => {
                        //Otherwise, ensure we factor the new guy into our math
                        new_count += 1;
                    }
                }
                prospective_state += 1;
            }
            iteration_lens.push(dfa_graph.node_count());
            if is_debug {
                let dur = last_time.elapsed();
                phase_events.send(dur).unwrap();
            }
            //Oh god is that it?
            //I am terrified of facing the music
            //Original pass finished 7/24
            //Actually working pass 8/5 (i'll admit, I took a weeklong break. Still pretty brutal tho)
        }
        if is_debug {
            dfa_events
                .send(real_self.translate_to_debug(&dfa_graph, &sig_set))
                .unwrap();
        }
        let mut trans_table =
            vec![vec![0; real_self.rules.symbol_set.length]; dfa_graph.node_count()];
        let mut accepting_states = Vec::new();
        for node in dfa_graph.node_indices() {
            for edge in dfa_graph.edges_directed(node, Outgoing) {
                trans_table[node.index()][*edge.weight() as usize] = edge.target().index();
            }
            //Checks if empty string set is a member of minkids or an ancestor of it
            accepting_states.push(real_self.check_if_ancestor(
                &dfa_graph[node].read().unwrap().minkids,
                real_self.ss_idx_to_link[0],
            ));
        }
        DFA {
            state_transitions: trans_table,
            accepting_states: accepting_states,
            starting_state: 0,
            symbol_set: real_self.rules.symbol_set.clone(),
        }
    }
}

impl MinkidSolver {
    fn build_ss_link_graph(&mut self, sig_set: &Vec<Vec<SymbolIdx>>) {
        let mut ss_link_graph = DiGraph::<usize, ()>::with_capacity(sig_set.len(), 10);
        //irritated that there is not an immediately obvious better way but w/e

        //build initial link graph
        for i in 0..sig_set.len() {
            ss_link_graph.add_node(i);
        }
        for i in 0..sig_set.len() {
            for result in self.single_rule_hash(&sig_set[i]) {
                let target_idx = self.rules.symbol_set.find_in_sig_set(result.iter());
                if target_idx < sig_set.len() {
                    ss_link_graph.add_edge(NodeIndex::new(i), NodeIndex::new(target_idx), ());
                }
            }
        }
        //Get rid of strongly-connected components
        let ss_link_graph = condensation(ss_link_graph, true);

        self.ss_idx_to_link = vec![NodeIndex::new(0); sig_set.len()];

        //Convert into actually-used data structure
        self.ss_link_graph = DiGraph::new();
        for i in ss_link_graph.node_indices() {
            let mut idxs_clone = ss_link_graph[i].clone();
            idxs_clone.shrink_to_fit();
            for idx in &idxs_clone {
                self.ss_idx_to_link[*idx] = i;
            }
            self.ss_link_graph.add_node(SignatureSetElement {
                original_idxs: idxs_clone,
            });
        }
        //I would love to care about this. will not yet!
        //self.ss_link_graph.extend_with_edges(self.ss_link_graph.raw_edges().iter());
        for i in ss_link_graph.raw_edges() {
            self.ss_link_graph.add_edge(i.source(), i.target(), ());
        }

        //time to pre-compute ancestors & calculate valid DFA states
        let mut reversed_graph = self.ss_link_graph.clone();
        reversed_graph.reverse();

        //Building minkids for each state in the goal DFA
        //Done by performing DFS
        self.goal_minkids = vec![HashSet::new(); self.goal.state_transitions.len()];
        //There is a fancier DFS-based way to do this. Do I look like the type to care?
        //(jk again just not pre-emptively optimizing)
        for goal_state in 0..self.goal_minkids.len() {
            //Toposort used so no childer checks needed
            for element in toposort(&reversed_graph, None).unwrap() {
                //Are any of the strings represented by this node accepting?
                let is_accepted = self.ss_link_graph[element]
                    .original_idxs
                    .iter()
                    .any(|x| self.goal.contains_from_start(&sig_set[*x], goal_state));
                //If it's an accepting state that is not the ancestor of any of the current minkids
                if is_accepted && !self.check_if_ancestor(&self.goal_minkids[goal_state], element) {
                    self.goal_minkids[goal_state].insert(element);
                }
            }
        }
        /*
        let mut ss_debug_graph : DiGraph<String,()> = Graph::new();
        for node_idx in self.ss_link_graph.node_indices() {
            let node = &self.ss_link_graph[node_idx];
            let mut final_str = format!("{}:",node_idx.index());
            for i in &node.original_idxs {
                final_str.push_str(&self.rules.symbol_set.symbols_to_string(&sig_set[*i]));
            }
            ss_debug_graph.add_node(final_str);
        }
        for edge in self.ss_link_graph.raw_edges() {
            ss_debug_graph.add_edge(edge.source(), edge.target(), ());
        }
        let mut file = std::fs::File::create("link_graph_debug/ss.dot").unwrap();
        file.write_fmt(format_args!("{:?}",petgraph::dot::Dot::new(&ss_debug_graph))); */
        /*
        for i in self.ss_link_graph.node_indices() {
            //Calculating all ancestors
            //Notably, this includes itself. Burns some memory, but allows us to skip what would otherwise be an additional check
           // let mut dfs = Dfs::new(&reversed_graph,i);
            //while let Some(nx) = dfs.next(&reversed_graph) {
            //    self.ss_link_graph[i].precomputed_ancestors.insert(nx);
            //}
            //Calculating valid DFA states
            //old method for building accepting states for each string -- disliked bc worse for both time/memory complexity

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
    fn check_if_ancestor(&self, min_children: &HashSet<NodeIndex>, potential: NodeIndex) -> bool {
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
    fn check_if_childer(
        &self,
        min_children: &HashSet<NodeIndex>,
        potential: NodeIndex,
    ) -> HashSet<NodeIndex> {
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
    //notably sub-optimal -- i am keeping things readable first because I am gonna go cross-eyed if I preemptively optimize THIS
    //Returns true if minkids is modified
    /*
    fn add_to_minkids(&self, min_children : &mut HashSet<NodeIndex>, potential : NodeIndex) -> bool {
        if self.check_if_ancestor(min_children, potential) {
            return false;
        }
        let redundant_kids = self.check_if_childer(min_children, potential);
        min_children.insert(potential);
        //This could be dumb!
        *min_children = min_children.difference(&redundant_kids).map(|x| *x).collect::<HashSet<_>>();
        return !redundant_kids.is_empty();
    } */
    //This could probably be a lot faster... oh well!
    fn add_set_to_minkids(
        &self,
        min_children: &mut HashSet<NodeIndex>,
        potential_kids: &HashSet<NodeIndex>,
    ) -> bool {
        let mut modified = false;
        for potential in potential_kids {
            if self.check_if_ancestor(min_children, *potential) {
                continue;
            }
            let redundant_kids = self.check_if_childer(min_children, *potential);
            /*
            if !redundant_kids.is_empty() {
                println!("Childer is actually useful!");
            }*/
            min_children.insert(*potential);
            //This could be dumb!
            *min_children = min_children
                .difference(&redundant_kids)
                .map(|x| *x)
                .collect::<HashSet<_>>();
            modified = true;
        }
        modified
    }
    //Call to apply a partial link between two nodes
    fn partial_link(
        self: &Arc<Self>,
        dfa_graph: &DiGraph<RwLock<MKDFAState>, SymbolIdx>,
        sig_set: &Vec<Vec<SymbolIdx>>,
        connection: &(Vec<SymbolIdx>, Vec<SymbolIdx>),
        lhs: NodeIndex,
        rhs: NodeIndex,
    ) -> bool {
        //To do this, we need to build an intermediary set of potential minkids that could be provided
        let reversed_graph = petgraph::visit::Reversed(&self.ss_link_graph);
        let mut dfs = Dfs::empty(&reversed_graph);

        let mut intermediary_minkids = HashSet::new();

        let temp_lock = dfa_graph[rhs].read().unwrap();

        let rhs_minkids = temp_lock.minkids.clone();

        drop(temp_lock);
        //For each bottom minkid,
        for rhs_minkid in &rhs_minkids {
            //Build a list of minkids that are ancestors of the rhs_minkid and possess a ss element that complies with the obligation
            dfs.move_to(*rhs_minkid);
            while let Some(nx) = dfs.next(&reversed_graph) {
                for ss_idx in &self.ss_link_graph[nx].original_idxs {
                    //If the ss element is actually big enough to comply with the obligation, and does
                    if sig_set[*ss_idx].len() >= connection.1.len()
                        && sig_set[*ss_idx][0..connection.1.len()] == connection.1
                    {
                        //Build what the new element would look like
                        let mut new_ss = connection.0.clone();
                        new_ss.extend(&sig_set[*ss_idx][connection.1.len()..]);
                        let ss_idx = self.rules.symbol_set.find_in_sig_set(new_ss.iter());
                        //If new_ss.len() > k
                        if ss_idx >= sig_set.len() {
                            continue;
                        }

                        //Find its index, translate it to a node, and add that node to our list of intermediary minkids
                        intermediary_minkids.insert(
                            self.ss_idx_to_link
                                [self.rules.symbol_set.find_in_sig_set(new_ss.iter())],
                        );
                        //Prevent looking further into this area's ancestors
                        //dfs adds all of the unvisited children of the thing to the stack. this stops that
                        /*
                        for i in reversed_graph.neighbors(nx).collect::<Vec<_>>().iter().rev() {
                            if dfs.discovered.visit(*i) {
                                dfs.stack.pop();
                            }
                        }*/

                        break;
                    }
                }
            }
        }
        self.add_set_to_minkids(
            &mut dfa_graph[lhs].write().unwrap().minkids,
            &intermediary_minkids,
        )
    }

    fn add_link(
        self: &Arc<Self>,
        link_graph: &mut DiGraph<(), (Vec<SymbolIdx>, Vec<SymbolIdx>)>,
        lhs: NodeIndex,
        rhs: NodeIndex,
        lhs_obligation: &[SymbolIdx],
        rhs_obligation: &[SymbolIdx],
    ) -> (bool, bool) {
        let mut death_row = vec![];
        let mut should_add = true;

        //checking every pre-existing edge to see if
        //1. we make any of them redundant by offering a more flexible alternative
        //2. any of them make our potential link redundant by already being more flexible
        for edge in link_graph.edges_connecting(lhs, rhs) {
            //redundancy check!

            let lhs_min = std::cmp::min(lhs_obligation.len(), edge.weight().0.len());
            let rhs_min = std::cmp::min(rhs_obligation.len(), edge.weight().1.len());
            //Check our proposed obligation against the current one. If ours is shorter, compare the prefixes of both such that they maintain equal length
            if &edge.weight().0[..lhs_min] == &lhs_obligation[..lhs_min]
                && &edge.weight().1[..rhs_min] == &rhs_obligation[..rhs_min]
            {
                //and the current edge has a greater obligation
                if (lhs_obligation.len() == lhs_min && rhs_obligation.len() == rhs_min)
                    && (edge.weight().0.len() > lhs_min || edge.weight().1.len() > rhs_min)
                {
                    death_row.push(edge.id());
                //otherwise, there's no benefit to adding this edge!
                } else {
                    should_add = false;
                    break;
                }
            }
        }
        if should_add {
            //This has made me realize these could definitely just be references... but whatever!
            link_graph.add_edge(lhs, rhs, (lhs_obligation.to_vec(), rhs_obligation.to_vec()));
        }
        for dead_edge in &death_row {
            link_graph.remove_edge(*dead_edge);
        }
        //TODO: There are currently circumstances where death row isn't empty and the link isn't added
        //This should only be possible if there's extraneous elements.
        (should_add, !death_row.is_empty())
    }
    fn minkids_to_tt(&self, sig_set: &Vec<Vec<SymbolIdx>>, minkids: &HashSet<NodeIndex>) -> BitVec {
        let mut result = bitvec![0;sig_set.len()];
        let reversed_graph = petgraph::visit::Reversed(&self.ss_link_graph);
        let mut dfs = Dfs::empty(&reversed_graph);
        for minkid in minkids {
            dfs.move_to(*minkid);
            while let Some(nx) = dfs.next(&reversed_graph) {
                for ss_idx in &self.ss_link_graph[nx].original_idxs {
                    result.set(*ss_idx, true);
                }
            }
        }
        result
    }
    fn translate_to_debug(
        &self,
        dfa_graph: &DiGraph<RwLock<MKDFAState>, SymbolIdx>,
        sig_set: &Vec<Vec<SymbolIdx>>,
    ) -> (DFAStructure, SSStructure) {
        let mut minkids_debug = vec![];
        for node in dfa_graph.node_indices() {
            minkids_debug
                .push(self.minkids_to_tt(&sig_set, &dfa_graph[node].read().unwrap().minkids));
        }
        (
            DFAStructure::Graph(dfa_graph.map(|_, _| (), |_, x| *x).clone()),
            SSStructure::Boolean(minkids_debug),
        )
    }
}
