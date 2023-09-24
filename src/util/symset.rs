use std::collections::HashMap;
use serde::{Serialize,Deserialize};
pub type SymbolIdx = u8;

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Ruleset {
    pub rules : HashMap<Vec<SymbolIdx>,Vec<Vec<SymbolIdx>>>,
    pub symbol_set : SymbolSet
}

impl Ruleset {
    pub fn new(rules : HashMap<Vec<SymbolIdx>,Vec<Vec<SymbolIdx>>>, symbol_set : SymbolSet) -> Self {
        Ruleset {rules : rules, symbol_set : symbol_set}
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
        Ruleset { rules: rule_hash, symbol_set: symbol_set }
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
               rhs.push(sym_set.string_to_symbols(&result));
            }
            rules.insert(sym_set.string_to_symbols(&key), rhs);
        }
    
    
        Ruleset {rules: rules, symbol_set: sym_set}
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
}

#[derive(Clone,Serialize,Deserialize, PartialEq, Eq,Debug)]
pub struct SymbolSet {
    pub length : usize,
    pub representations : Vec<String>
}
impl SymbolSet {
    pub fn new(representations : Vec<String>) -> SymbolSet{
        SymbolSet { length: representations.len(), representations: representations }
    }

    pub fn find_in_sig_set<'a>(&self, string : impl Iterator<Item = &'a SymbolIdx>) -> usize
    {
        let mut result = 0;
        for sym in string {
            result *= self.length;
            result += *sym as usize + 1;
        }
        result
    }
    pub fn idx_to_element(&self, mut idx : usize) -> Vec<SymbolIdx>
    {
        let mut result = vec![];
        while idx > 0 {
            idx -= 1;
            result.push((idx % self.length) as SymbolIdx);
            idx /= self.length;
        }
        result.reverse();
        result
    }
    pub fn build_sig_k(&self, k : usize) -> Vec<Vec<SymbolIdx>> {
        //let start_sig_len : usize = (cardinality::<S>() << k)-1;
        let mut start_index = 0;
        let mut signature_set : Vec<Vec<SymbolIdx>> = vec![vec![]];
        let mut end_index = 1;
        let mut new_index = 1;
        for _ in 0..k {
            for i in start_index..end_index{
                for symbol in 0..(self.length as SymbolIdx) {
                    signature_set.push(signature_set[i].clone());
                    signature_set[new_index].push(symbol);
                    new_index += 1;
                }
            }
            start_index = end_index;
            end_index = new_index;
        }
        signature_set
    }
    pub fn symbols_to_string(&self, symbols : &Vec<SymbolIdx>) -> String{
        let mut string = "".to_owned();
        for sym in symbols {
            string += &format!("{} ", self.representations[*sym as usize]);
        }
        string.pop();
        string
    }
    pub fn string_to_symbols(&self, symbols : &Vec<&str>) -> Vec<SymbolIdx>{
        let mut syms = vec![];
        for str in symbols {
            syms.push(self.representations.iter().position(|r| r == str).unwrap() as SymbolIdx);
        }
        syms
    }
}