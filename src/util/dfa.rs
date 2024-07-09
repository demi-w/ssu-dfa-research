use crate::{SymbolSet, SymbolIdx};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::{HashMap, HashSet}, io::Read, marker::PhantomData, ops::{Deref, IndexMut}, slice::SliceIndex};
use xml::{writer::{EmitterConfig, XmlEvent},reader::EventReader};

use serde_json::{value::Index, Result};
use bitvec::prelude::*;
use std::io::Write;


#[derive(Clone,Serialize,Deserialize)]
pub struct DFA<Input = String, Output = bool>
{
    pub starting_state : usize,
    pub state_transitions : Vec<Vec<usize>>,
    pub accepting_states : Vec<Output>, //I really do like rust -- however, trying to allow for either bitvec or
                                        //vec stopped progress for 2 weeks
    pub symbol_set : SymbolSet<Input>
}

trait GetDiscriminant {
    fn discriminant (&self) -> usize;
    const DISCRIMINANT_LEN : usize;
}

impl GetDiscriminant for bool {
    fn discriminant (&self) -> usize {
        if *self {
            1
        } else {
            0
        }
    }
    const DISCRIMINANT_LEN : usize = 2;
}
enum JFLAPTrans {
    From,
    To,
    Read,
    Unknown
}

#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;

#[cfg(target_arch = "wasm32")]
use rfd::FileHandle;
#[cfg(target_arch = "wasm32")]
type File = FileHandle;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures;

use super::symset;

impl<Input, Output> PartialOrd for DFA<Input, Output> where Output : PartialOrd
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let mut stack = vec![(self.starting_state,other.starting_state)];
        let mut visited = HashSet::new();
        let mut self_more = false;
        let mut other_more = false;
        if self.symbol_set.length != other.symbol_set.length {
            return None;
        }
        visited.insert((self.starting_state,other.starting_state));
        while let Some(pair) = stack.pop() {
            if self.accepting_states[pair.0] != other.accepting_states[pair.1] {
                self_more |= self.accepting_states[pair.0] >= self.accepting_states[pair.1];
                other_more |= self.accepting_states[pair.0] <= self.accepting_states[pair.1];
                if self_more && other_more {
                    return None;
                }
            }
            for i in 0..self.symbol_set.length {
                let test = (self.state_transitions[pair.0][i],other.state_transitions[pair.1][i]);
                if !visited.contains(&test) {
                    visited.insert(test.clone());
                    stack.push(test);
                }
            }
        }
        if !self_more && !other_more {
            Some(std::cmp::Ordering::Equal)
        } else if self_more && !other_more {
            Some(std::cmp::Ordering::Greater)
        } else if !self_more && other_more {
            Some(std::cmp::Ordering::Less)
        } else {
            None
        }
    }
}

impl<I,O> std::ops::Not for &DFA<I,O> where O : Clone + std::ops::Not<Output = O>, DFA<I, O> : Clone{
    type Output = DFA<I,O>;
    fn not(self) -> Self::Output {
        let mut clone = self.clone();
        let mut inverted_accepting_states = Vec::new();
        for i in 0..self.state_transitions.len() {
            inverted_accepting_states[i] = !self.accepting_states[i].clone();
        }
        clone.accepting_states = inverted_accepting_states;
        clone
    }
}

impl<I,O> std::ops::BitAnd for &DFA<I,O> where I : Clone, O : std::ops::BitAnd<Output = O> + Clone + Ord {
    type Output = DFA<I, O>;

    fn bitand(self, rhs: Self) -> Self::Output {
        self.dfa_product(rhs, |s,o|{s.clone() & o.clone()})
    }
}
impl<I,O> std::ops::BitOr for &DFA<I,O> where I : Clone, O : std::ops::BitOr<Output = O> + Clone + Ord {
    type Output = DFA<I,O>;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.dfa_product(rhs, |s,o|{s.clone() | o.clone()})
    }
}
impl<I,O> std::ops::BitXor for &DFA<I,O> where I : Clone, O : std::ops::BitXor<Output = O> + Clone + Ord  {
    type Output = DFA<I,O>;

    fn bitxor(self, rhs: Self) -> Self::Output {
        self.dfa_product(rhs, |s,o|{s.clone() ^ o.clone()})
    }
}


impl<I, O> std::ops::Sub for &DFA<I, O> where I : Clone, O : std::ops::Sub<Output = O> + Clone + Ord {
    type Output = DFA<I, O>;

    fn sub(self, rhs: Self) -> Self::Output {
        self.dfa_product(rhs, |s,o|{s.clone() - o.clone()})
    }
}



impl DFA {
    pub fn expand_to_symset(&mut self, expanded_ss : SymbolSet) {

        //Find all elements of the expanded symbol set that do not exist in the dfa's
        let mut expanded_idx = 0;
        let mut holes = vec![];
        for (idx,rep) in self.symbol_set.representations.iter().enumerate() {
            while rep != &expanded_ss.representations[expanded_idx] {
                holes.push(idx);
                expanded_idx+=1;
            }
            expanded_idx+=1;
        }
        while expanded_idx < expanded_ss.length {
            holes.push(usize::MAX);
            expanded_idx+=1;
        }

        //Find/create an error state that all of the new transitions will be routed to
        let mut error_state = None;
        for i in 0..self.state_transitions.len() {
            if self.accepting_states[i] {
                continue;
            }
            if self.state_transitions[i].iter().all(|x| x == &i) {
                error_state = Some(i);
            }
        } 
        let error_state = match error_state {
            Some(s) => {s}
            None => {
                self.state_transitions.push(vec![self.state_transitions.len();self.symbol_set.length]);
                self.state_transitions.len() - 1
            }
        };

        //Modify transition table to include 
        for i in 0..self.state_transitions.len() {
            let mut new_trans = Vec::with_capacity(expanded_ss.length);
            let mut holes_encountered = 0;
            for j in 0..self.state_transitions[i].len() {
                while holes_encountered < holes.len() && holes[holes_encountered] == j {
                    new_trans.push(error_state);
                    holes_encountered+=1;
                }
                new_trans.push(self.state_transitions[i][j]);
            }
            while holes_encountered < holes.len() {
                new_trans.push(error_state);
                holes_encountered+=1;
            }
            self.state_transitions[i] = new_trans;
        }
        self.symbol_set = expanded_ss;
    }

    pub fn load_jflap_from_string(input_xml : &str) -> Self {
        let mut trans_table ;
        let mut accepting_states = vec![];
        let mut num_states = 0;
        let mut starting_state=0 ;
        let mut e_reader = EventReader::from_str(input_xml);
        let mut cur_state=0;
        let mut trans_vec = vec![];
        let mut unique_reps = HashSet::new();
        let mut jflap_trans = JFLAPTrans::Unknown;

        let mut state_ids : HashMap<String, usize>= HashMap::new();

        while let Ok(cur_event) = e_reader.next() {
            match cur_event {
                xml::reader::XmlEvent::EndDocument => {break},
                xml::reader::XmlEvent::Whitespace(_) => {},
                xml::reader::XmlEvent::StartDocument { version : _, encoding : _, standalone : _ } => {},
                xml::reader::XmlEvent::Comment(_) => {},
                xml::reader::XmlEvent::StartElement { name, attributes, namespace : _ } => {
                    match &name.local_name[..] { 
                        "state" => {
                            let cur_str = &attributes.iter().find(|&x| x.name.local_name == "id").unwrap().value;
                            cur_state = cur_str.parse().unwrap();
                            state_ids.insert(cur_str.clone(), num_states);
                            num_states += 1;
                        },
                        "initial" => {starting_state = cur_state},
                        "final" => {accepting_states[num_states-1] = true;},
                        "transition" => {trans_vec.push((0,0,"".to_owned()))},
                        "from" => {jflap_trans = JFLAPTrans::From},
                        "to" => {jflap_trans = JFLAPTrans::To},
                        "read" => {jflap_trans = JFLAPTrans::Read}
                        _ => {},
                    }
                }
                xml::reader::XmlEvent::EndElement { name } => {
                    match &name.local_name[..] { 
                        "from" | "to" | "read" => {jflap_trans = JFLAPTrans::Unknown},
                        _ => {},
                    }
                },
                xml::reader::XmlEvent::Characters (chars) => {
                    match jflap_trans { 
                        JFLAPTrans::From => {trans_vec.last_mut().unwrap().0 = *state_ids.get(&chars).unwrap()},
                        JFLAPTrans::To => {trans_vec.last_mut().unwrap().1 = *state_ids.get(&chars).unwrap()},
                        JFLAPTrans::Read => {trans_vec.last_mut().unwrap().2 = chars.clone(); unique_reps.insert(chars);},
                        JFLAPTrans::Unknown => {}
                    }
                }
                _ => {}
            }
        }
        let mut reps_vec : Vec<String> = unique_reps.into_iter().collect();
        reps_vec.sort();

        trans_table = vec![vec![usize::MAX;reps_vec.len()];num_states];
        
        for transition in &trans_vec {
            trans_table[transition.0][reps_vec.iter().position(|x| x == &transition.2).unwrap()] = transition.1;
        }
        //If dfa is incomplete
        if trans_vec.len() < reps_vec.len() * trans_table.len() {
            let mut error_state_already = None;
            for state_idx in 0..trans_table.len() {
                if !accepting_states[state_idx] && trans_table[state_idx].iter().all(|f| {f == &state_idx || f == &usize::MAX}){
                    error_state_already = Some(state_idx);
                }
            }
            let error_state = match error_state_already {
                Some(e_state) => e_state,
                None => {trans_table.push(vec![trans_table.len();reps_vec.len()]); trans_table.len() - 1}
            };
            for state_trans in &mut trans_table {
                state_trans.iter_mut().for_each(|f| {if *f == usize::MAX {*f = error_state}});
            }
        }

        let temp = SymbolSet {
            length : reps_vec.len(),
            representations : reps_vec
        };
        DFA { starting_state: starting_state, state_transitions: trans_table, accepting_states: accepting_states, symbol_set: temp }
    }

    pub fn save_jflap_to_bytes(&self) -> Vec<u8> {
        let mut output_str = vec![];
        let mut w = EmitterConfig::new().perform_indent(true).create_writer(&mut output_str);
        w.write(XmlEvent::start_element("structure")).unwrap();
        w.write(XmlEvent::start_element("type")).unwrap();
        w.write(XmlEvent::characters("fa")).unwrap();
        w.write(XmlEvent::end_element()).unwrap();
        w.write(XmlEvent::start_element("automaton")).unwrap();
        
        for idx in 0..self.state_transitions.len() {
            w.write(XmlEvent::start_element("state")
                                                                .attr("id",&idx.to_string())
                                                                .attr("name",&("q".to_owned()+&idx.to_string()))
                                                            ).unwrap();
            if idx == self.starting_state {
                w.write(XmlEvent::start_element("initial")).unwrap();
                w.write(XmlEvent::end_element()).unwrap();
            }                                
            if self.accepting_states[idx] {
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
        output_str
    }

    pub fn jflap_save(&self, file : &mut File) {
        //let mut file = fs::File::create(filename.clone().to_owned() + ".jff").unwrap();
        let _ = file.write(&self.save_jflap_to_bytes());
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub fn jflap_load(file : &mut File) -> Self {
        let mut contents = "".to_owned();
        file.read_to_string(&mut contents).unwrap();
        Self::load_jflap_from_string(&contents)
    }
}
impl<I,O> DFA<I,O> where I : Clone, O : Clone + Ord {

    pub fn dfa_product<F> (&self, other : &DFA<I,O>, accepting_rule : F) -> Self where F : Fn(&O, &O) -> O {
        let mut stored_idxs = vec![(self.starting_state,other.starting_state)];
        let mut transition_table = vec![];
        let mut accepting_states = vec![];
        accepting_states.push(accepting_rule(&self.accepting_states[self.starting_state],&other.accepting_states[other.starting_state]));
        while stored_idxs.len() > transition_table.len() {
            for new_state_idx in transition_table.len()..stored_idxs.len() {
                transition_table.push(vec![0;self.symbol_set.length]);
                for symbol in 0..self.symbol_set.length {
                    let new_self_idx = self.state_transitions[stored_idxs[new_state_idx].0][symbol];
                    let new_other_idx = other.state_transitions[stored_idxs[new_state_idx].1][symbol];
                    let new_pair = (new_self_idx, new_other_idx);
                    match stored_idxs.iter().position(|f| f == &new_pair) {
                        Some(pos) => {
                            transition_table.last_mut().unwrap()[symbol] = pos;
                        }
                        None =>  {
                            accepting_states.push(accepting_rule(&self.accepting_states[new_pair.0],&other.accepting_states[new_pair.1]));
                            transition_table.last_mut().unwrap()[symbol] = stored_idxs.len();
                            stored_idxs.push(new_pair);
                        }
                    }
                }
            }
        }
        DFA {
            accepting_states : accepting_states,
            starting_state : 0,
            state_transitions : transition_table,
            symbol_set : self.symbol_set.clone(),
        }
    }

    pub fn minimize(&mut self) {

        let mut new_partition_membership = vec![0;self.state_transitions.len()];
        let mut old_partition_membership = vec![0;self.state_transitions.len()];
        let mut new_partitions : Vec<Vec<usize>> = vec![vec![],vec![]];
        let mut old_partitions = vec![];

        let mut old_outputs = Vec::new();
        for i in 0..self.state_transitions.len() {
            let idx = match (&mut old_outputs.clone().into_iter()).position(|x| {x == self.accepting_states[i]}) {
                Some(idx) => {
                    idx
                }
                None => {
                    old_outputs.push(self.accepting_states[i].clone());
                    old_outputs.len() - 1
                }
            };
            new_partition_membership[i] = idx;
            new_partitions[idx].push(i)
        }
        

        while new_partitions.len() > old_partitions.len() {
            std::mem::swap(&mut old_partitions, &mut new_partitions);
            std::mem::swap(&mut old_partition_membership, &mut new_partition_membership);
            new_partitions.clear();
            for partition in &old_partitions {
                if partition.len() == 1 {
                    new_partition_membership[partition[0]] = new_partitions.len();
                    new_partitions.push(partition.clone());
                    continue
                }
                let mut truth_vals : Vec<Vec<usize>> = vec![];
                let mut split_partitions : Vec<Vec<usize>> = vec![];
                for state in partition {
                    let mut truth_val = vec![0;self.symbol_set.length];
                    for symbol in 0..self.symbol_set.length {
                        truth_val[symbol] = old_partition_membership[self.state_transitions[*state][symbol]];
                    }
                    let mut match_found = false;
                    for tv_idx in 0..truth_vals.len() {
                        if truth_val == truth_vals[tv_idx] {
                            split_partitions[tv_idx].push(*state);
                            new_partition_membership[*state] = new_partitions.len() + tv_idx;
                            match_found = true;
                            break;
                        }
                    }
                    if !match_found {
                        new_partition_membership[*state] = new_partitions.len() + truth_vals.len();
                        truth_vals.push(truth_val);
                        split_partitions.push(vec![*state]);
                    }
                }
                new_partitions.append(&mut split_partitions);
            }
        }
        let mut new_accepting = Vec::new();
        for (p_index, partition) in new_partitions.iter().enumerate() {
            new_accepting.push(self.accepting_states[partition[0]].clone());
        }

        self.starting_state = new_partition_membership[self.starting_state];
        let mut new_transition_table = vec![];

        for partition in new_partitions {
            let mut new_transitions = vec![0;self.symbol_set.length];
            for symbol in 0..self.symbol_set.length {
                new_transitions[symbol] = new_partition_membership[self.state_transitions[partition[0]][symbol]];
            }
            
            new_transition_table.push(new_transitions);
        }
        self.state_transitions = new_transition_table;
        self.accepting_states = new_accepting;

    }

    pub fn ss_eq(&self, other: &Self, our_ss : &Vec<BitVec>, other_ss : &Vec<BitVec>) -> Vec<(usize,usize,Vec<usize>,Vec<usize>)> {
        let mut stack = HashSet::new();
        stack.insert((self.starting_state,other.starting_state));
        let mut old_stack = HashSet::new();
        let mut visited = HashSet::new();
        let mut result = vec![];
        visited.insert((self.starting_state,other.starting_state));
        while !stack.is_empty() {
            old_stack.clear();
            std::mem::swap(&mut old_stack, &mut stack);
            for pair in &old_stack {
                if our_ss[pair.0] != other_ss[pair.1] {
                    let mut too_nice = vec![];
                    let mut too_mean = vec![];
                    for bit in 0..our_ss[0].len() {
                        if our_ss[pair.0][bit] && !other_ss[pair.1][bit] {
                            too_nice.push(bit);
                        }else if !our_ss[pair.0][bit] && other_ss[pair.1][bit]{
                            too_mean.push(bit);
                        }
                    }
                    result.push((pair.0,pair.1,too_nice,too_mean));
                    continue
                }
                for i in 0..self.symbol_set.length {
                    let test = (self.state_transitions[pair.0][i],other.state_transitions[pair.1][i]);
                    if !visited.contains(&test) {
                        visited.insert(test.clone());
                        stack.insert(test);
                    }
                }
            }
        }
        result
    }

    pub fn contains(&self, input : &Vec<SymbolIdx>) -> O {
        let mut state = self.starting_state;
        for i in input {
            state = self.state_transitions[state][*i as usize];
        }
        self.accepting_states[state].clone()
    }

    pub fn final_state(&self, input : &Vec<SymbolIdx>) -> usize{
        let mut state = self.starting_state;
        for i in input {
            state = self.state_transitions[state][*i as usize];
        }
        state
    }

    pub fn contains_from_start(&self, input : &Vec<SymbolIdx>, start : usize) -> O {
        let mut state = start;
        for i in input {
            state = self.state_transitions[state][*i as usize];
        }
        self.accepting_states[state].clone()
    }

    pub fn shortest_path_to_state(&self, desired : usize) -> Vec<SymbolIdx> {
        if desired == self.starting_state {
            return vec![];
        }
        let mut backpath = vec![usize::MAX;self.state_transitions.len()];
        backpath[self.starting_state] = self.starting_state;
        let mut next_paths = vec![self.starting_state];
        let mut old_paths = vec![];
        let mut found_desired = false;
        while !found_desired && !next_paths.is_empty(){
            old_paths.clear();
            std::mem::swap(&mut next_paths, &mut old_paths);
            for frontier_state in &old_paths {
                for next_spot in &self.state_transitions[*frontier_state] {
                    if backpath[*next_spot] == usize::MAX {
                        backpath[*next_spot] = *frontier_state;
                        next_paths.push(*next_spot);
                        if *next_spot == desired {
                            found_desired = true;
                            break
                        }
                    }
                }
            }
        }
        let mut path : Vec<SymbolIdx> = vec![];
        let mut cur_state = desired;
        while cur_state != self.starting_state {
            let back_state = backpath[cur_state];
            for sym in 0..self.symbol_set.length {
                if self.state_transitions[back_state][sym] == cur_state {
                    path.push(sym as SymbolIdx);
                    break
                }
            }
            cur_state = back_state;
        }
        path.reverse();
        path

    }


    pub fn shortest_path_to_pair(&self, other: &Self, our_state : usize, other_state : usize) -> Vec<SymbolIdx> {
        let mut old_stack = HashSet::new();
        let mut stack = HashSet::new();
        stack.insert((self.starting_state,other.starting_state));
        let mut visited = HashMap::new();
        visited.insert((self.starting_state,other.starting_state),(self.starting_state,other.starting_state));
        while !stack.is_empty(){
            old_stack.clear();
            std::mem::swap(&mut old_stack, &mut stack);
            for pair in &old_stack {
                if *pair == (our_state, other_state) {
                    stack.clear();
                    break
                }
                for i in 0..self.symbol_set.length {
                    let test = (self.state_transitions[pair.0][i],other.state_transitions[pair.1][i]);
                    if !visited.contains_key(&test) {
                        visited.insert(test.clone(),pair.clone());
                        stack.insert(test);
                    }
                }
            }
        }

        let mut path : Vec<SymbolIdx> = vec![];
        let mut cur_state = (our_state,other_state);
        while cur_state != (self.starting_state,other.starting_state) {
            let back_state = visited[&cur_state];
            for sym in 0..self.symbol_set.length {
                if self.state_transitions[back_state.0][sym] == cur_state.0 && other.state_transitions[back_state.1][sym] == cur_state.1 {
                    path.push(sym as SymbolIdx);
                    break
                }
            }
            cur_state = back_state;
        }
        path.reverse();
        path
    }



    //pub fn minify(&mut self) {}

    //pub fn one_rule_expand(&self, rules : &Ruleset) -> DFA{ return self.clone()}
}

impl<'de,I,O> DFA<I,O> where I : Clone + Serialize + DeserializeOwned, O : Clone + Serialize + Ord + DeserializeOwned {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(file : &mut File) -> Result::<Self> {
        use std::io::BufReader;
        serde_json::from_reader(BufReader::new(file))

    }
    pub fn save(&self, file : &mut File) -> Result::<()> {
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer(writer,self)
    }
}

impl <I,O> PartialEq for DFA<I, O> where O : PartialEq  {
    fn eq(&self, other: &Self) -> bool {
        let mut stack = vec![(self.starting_state,other.starting_state)];
        let mut visited = HashSet::new();
        if self.symbol_set.length != other.symbol_set.length {
            return false;
        }
        visited.insert((self.starting_state,other.starting_state));
        while let Some(pair) = stack.pop() {
            if self.accepting_states[pair.0] != other.accepting_states[pair.1] {
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

