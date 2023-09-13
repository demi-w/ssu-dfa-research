use std::collections::HashMap;
use serde::{Serialize,Deserialize};

pub type SymbolIdx = u8;

#[derive(Debug,Clone)]
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
}

#[derive(Clone,Serialize,Deserialize,Debug)]
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
            string += &format!("{}", self.representations[*sym as usize]);
        }
        string
    }
}