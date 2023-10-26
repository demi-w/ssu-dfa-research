use crate::{SymbolSet, SymbolIdx};
use serde::{Deserialize, Serialize};
use std::{collections::{HashSet, HashMap}, io::Read};
use xml::{writer::{EmitterConfig, XmlEvent},reader::EventReader};

use serde_json::Result;
use bitvec::prelude::*;
use std::io::Write;

#[derive(Clone,Serialize,Deserialize)]
pub struct DFA {
    pub starting_state : usize,
    pub state_transitions : Vec<Vec<usize>>,
    pub accepting_states : HashSet::<usize>,
    pub symbol_set : SymbolSet
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
            if self.accepting_states.contains(&i) {
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

    pub fn contains(&self, input : &Vec<SymbolIdx>) -> bool {
        let mut state = self.starting_state;
        for i in input {
            state = self.state_transitions[state][*i as usize];
        }
        self.accepting_states.contains(&state)
    }

    pub fn final_state(&self, input : &Vec<SymbolIdx>) -> usize{
        let mut state = self.starting_state;
        for i in input {
            state = self.state_transitions[state][*i as usize];
        }
        state
    }

    pub fn contains_from_start(&self, input : &Vec<SymbolIdx>, start : usize) -> bool {
        let mut state = start;
        for i in input {
            state = self.state_transitions[state][*i as usize];
        }
        self.accepting_states.contains(&state)
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


    pub fn load_jflap_from_string(input_xml : &str) -> Self {
        let mut trans_table ;
        let mut accepting_states = HashSet::new();
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
                        "final" => {accepting_states.insert(num_states-1);},
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
                if !accepting_states.contains(&state_idx) && trans_table[state_idx].iter().all(|f| {f == &state_idx || f == &usize::MAX}){
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
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(file : &mut File) -> Result::<Self> {
        let mut contents = "".to_owned();
        file.read_to_string(&mut contents).unwrap();
        
        serde_json::from_str(&contents)
    }
    pub fn save(&self, file : &mut File) {
        //let mut file = fs::File::create(filename.clone().to_owned() + ".dfa").unwrap();
        let _ = file.write(serde_json::to_string(self).unwrap().as_bytes());
    }

    //pub fn minify(&mut self) {}

    //pub fn one_rule_expand(&self, rules : &Ruleset) -> DFA{ return self.clone()}
}

impl PartialEq for DFA {
    fn eq(&self, other: &Self) -> bool {
        let mut stack = vec![(self.starting_state,other.starting_state)];
        let mut visited = HashSet::new();
        if self.symbol_set.length != other.symbol_set.length {
            return false;
        }
        visited.insert((self.starting_state,other.starting_state));
        while let Some(pair) = stack.pop() {
            if self.accepting_states.contains(&pair.0) != other.accepting_states.contains(&pair.1) {
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

