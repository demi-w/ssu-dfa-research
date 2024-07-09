use std::collections::HashMap;

use super::{SymbolIdx, SymbolSet};


#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Ruleset {
    pub rules : HashMap<Vec<SymbolIdx>,Vec<Vec<SymbolIdx>>>,
    pub symbol_set : SymbolSet,
    pub max_input : usize,
    pub min_input : usize
}

impl Ruleset {
    pub fn new(rules : HashMap<Vec<SymbolIdx>,Vec<Vec<SymbolIdx>>>, symbol_set : SymbolSet) -> Self {
        Ruleset {
            symbol_set : symbol_set, 
            max_input : rules.keys().max_by_key(|x|{x.len()}).unwrap().len(),
            min_input : rules.keys().min_by_key(|x|{x.len()}).unwrap().len(),
            rules : rules, 
        }
    }

    pub fn from_vec(rules : Vec<(Vec<SymbolIdx>,Vec<SymbolIdx>)>, symbol_set : SymbolSet) -> Self {
        let mut rule_hash : HashMap<Vec<SymbolIdx>,Vec<Vec<SymbolIdx>>> = HashMap::new();
        //Should use a fancy map function here I admit
        for i in &rules {
            match rule_hash.get_mut(&i.0) {
                Some(result_vec) => {result_vec.push(i.1.clone())},
                None => {rule_hash.insert(i.0.clone(), vec![i.1.clone()]);}
            }
        }
        Ruleset { rules: rule_hash, symbol_set: symbol_set,max_input : rules.iter().max_by_key(|x|{x.0.len()}).unwrap().0.len(),
        min_input : rules.iter().min_by_key(|x|{x.0.len()}).unwrap().0.len() }
    }
    pub fn from_string(input_str : &str) -> Self {
        let mut rules_str : HashMap<Vec<&str>,Vec<Vec<&str>>> = HashMap::new();
        let mut symbols_rep : Vec<String> = Vec::new();
    
        //Add un-indexed list of rules
        for line in input_str.split('\n') {
            let uncommented_line = line.split('#').next().unwrap();
            //If line is exclusively whitespace
            if uncommented_line.split_whitespace().collect::<Vec<_>>().is_empty() {
                continue
            }
            let mut split_hs = uncommented_line.split("-");
    
            //safe unwrap as there must be at least one section in the split
            let lhs_raw = split_hs.next().unwrap();
    
            //If there is no -, assume the lhs simply goes into nothing
            let rhs_raw = match split_hs.next() {
                Some(rhs) => {rhs},
                None => {""}
            };
    
            //Remove any empty strings and collect rest of strings (divided by whitespace) into lhs & rhs
            let lhs : Vec<_> = lhs_raw.split_whitespace().filter(|&x| !x.is_empty()).collect();
            let rhs : Vec<_> = rhs_raw.split_whitespace().filter(|&x| !x.is_empty()).collect();
    
            for str in &lhs {
                if !symbols_rep.contains(&(*str).to_owned()) {
                    symbols_rep.push((*str).to_owned());
                }
            }

            for str in &rhs {
                if !symbols_rep.contains(&(*str).to_owned()) {
                    symbols_rep.push((*str).to_owned());
                }
            }
    
            match rules_str.get_mut(&lhs) {
                Some(rhs_list) => {
                    rhs_list.push(rhs);
                }
                None => {
                    rules_str.insert(lhs, vec![rhs]);
                }
            }
        }
        //Sort symbols according to rust's str system for consistency between dfa & ruleset
        symbols_rep.sort();
    
        let sym_set = SymbolSet {
            length : symbols_rep.len(),
            representations : symbols_rep
        };
    
        //Convert rules to indexed equivalents
    
        let mut rules : HashMap<Vec<SymbolIdx>,Vec<Vec<SymbolIdx>>> = HashMap::new();
    
        for (key, val) in rules_str {
            let mut rhs = vec![];
            for result in val {
               rhs.push(sym_set.string_to_symbols(&result).unwrap());
            }
            rules.insert(sym_set.string_to_symbols(&key).unwrap(), rhs);
        }
    
    
        Ruleset {symbol_set: sym_set,max_input : rules.keys().max_by_key(|x|{x.len()}).unwrap().len(),
        min_input : rules.keys().min_by_key(|x|{x.len()}).unwrap().len(), rules: rules}
    }

    pub fn expand_to_symset(&mut self,  expanded_ss : SymbolSet) {
        let mut translate_map = HashMap::new();
        let mut expanded_idx = 0;
        for (idx,rep) in self.symbol_set.representations.iter().enumerate() {
            while rep != &expanded_ss.representations[expanded_idx] {
                expanded_idx+=1;
            }
            translate_map.insert(idx as u8, expanded_idx as u8);
        }
        let mut new_rules = HashMap::new();
        for (lhs,rhs) in self.rules.iter_mut() {
            let mut new_lhs = lhs.clone();
            new_lhs.iter_mut().for_each(|x| *x = *translate_map.get(x).unwrap());
            rhs.iter_mut().for_each(|i_vec| i_vec.iter_mut().for_each(|x| *x = *translate_map.get(x).unwrap()));
            new_rules.insert(new_lhs, rhs.to_owned());
        }
        self.rules = new_rules;
        self.symbol_set = expanded_ss;
    }

    pub fn to_string(&self) -> String {
        let mut result = "".to_owned();
        for rule in &self.rules {
            //Map each element to its string rep then join all string reps together by a space.
            let lhs = rule.0.iter().map(|&x| self.symbol_set.representations[x as usize].clone()).collect::<Vec<String>>().join(" ");
            for rhs_vec in rule.1 {
                let rhs = rhs_vec.iter().map(|&x| self.symbol_set.representations[x as usize].clone()).collect::<Vec<String>>().join(" ");
                result.push_str(&lhs);
                result.push_str(" - ");
                result.push_str(&rhs);
                result.push('\n');
            }
        }
        result
    }
    pub fn has_generating_rule(&self) -> Option<(Vec<SymbolIdx>,Vec<SymbolIdx>)> {
        for rule in &self.rules {
            let lhs_len = rule.0.len();
            for rhs_vec in rule.1 {
                if rhs_vec.len() > lhs_len {
                    return Some((rule.0.clone(),rhs_vec.clone()));
                }
            }
        }
        None
    }
    pub fn has_deleting_rule(&self) -> Option<(Vec<SymbolIdx>,Vec<SymbolIdx>)> {
        for rule in &self.rules {
            let lhs_len = rule.0.len();
            for rhs_vec in rule.1 {
                if rhs_vec.len() < lhs_len {
                    return Some((rule.0.clone(),rhs_vec.clone()));
                }
            }
        }
        None
    }
    pub fn has_non_length_preserving_rule(&self) -> Option<(Vec<SymbolIdx>,Vec<SymbolIdx>)> {
        for rule in &self.rules {
            let lhs_len = rule.0.len();
            for rhs_vec in rule.1 {
                if rhs_vec.len() != lhs_len {
                    return Some((rule.0.clone(),rhs_vec.clone()));
                }
            }
        }
        None
    }
    pub fn has_definitely_cyclic_rule(&self) -> Option<(Vec<SymbolIdx>,Vec<SymbolIdx>)> {
        for rule in &self.rules {
            for rhs_vec in rule.1 {
                if let Some(rhs_as_lhs_rules) = self.rules.get(rhs_vec) {
                    for rhs_from_rhs in rhs_as_lhs_rules {
                        if rule.0 == rhs_from_rhs {
                            return Some((rule.0.clone(),rhs_vec.clone()))
                        }
                    }
                }
            }
        }
        None
    }
}