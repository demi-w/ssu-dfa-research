use crate::{SymbolSet, SymbolIdx};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, HashMap};
use xml::writer::{EmitterConfig, XmlEvent};
use std::fs;
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


impl DFA {

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

    fn save_jflap_to_file(&self,file : &mut fs::File) {
        let mut w = EmitterConfig::new().perform_indent(true).create_writer(file);
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
    }

    pub fn jflap_save(&self, filename : &str) {
        let mut file = fs::File::create(filename.clone().to_owned() + ".jff").unwrap();
        self.save_jflap_to_file(&mut file);
    }
    pub fn save(&self, filename : &str) {
        let mut file = fs::File::create(filename.clone().to_owned() + ".dfa").unwrap();
        file.write(serde_json::to_string(self).unwrap().as_bytes()).unwrap();
    }
    pub fn load(filename : &str) -> Result::<Self> {
        let contents = fs::read_to_string(filename.clone().to_owned() + ".dfa").unwrap();
        serde_json::from_str(&contents)
    }
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