use serde::{Serialize,Deserialize};
pub type SymbolIdx = u8;

#[derive(Clone,Serialize,Deserialize, PartialEq, Eq,Debug)]
pub struct SymbolSet<PrettyInput = String> {
    pub length : usize,
    pub representations : Vec<PrettyInput>
}

impl<PrettyInput> SymbolSet<PrettyInput> where PrettyInput : std::fmt::Display {
    pub fn symbols_to_string(&self, symbols : &Vec<SymbolIdx>) -> String{
        let mut string = "".to_owned();
        for sym in symbols {
            string += &format!("{} ", self.representations[*sym as usize]);
        }
        string.pop();
        string
    }
    pub fn string_to_symbols(&self, symbols : &Vec<&str>) -> Result<Vec<SymbolIdx>,usize>{
        let mut syms = vec![];
        for (idx,str) in symbols.iter().enumerate() {
            if str == &"" {
                continue
            }
            match self.representations.iter().position(|r| &format!("{}",r) == str) {
                Some(sym) => {syms.push(sym as SymbolIdx)},
                None => {return Err(idx)}
            }
            
        }
        Ok(syms)
    }
}
impl<PrettyInput> SymbolSet<PrettyInput> {
    pub fn new(mut representations : Vec<String>) -> SymbolSet{
        representations.sort();
        SymbolSet { length: representations.len(), representations: representations }
    }

    //Returns the appropriate index for a certain string
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

}

impl<PrettyInput> SymbolSet<PrettyInput> where PrettyInput : std::cmp::PartialEq {
    pub fn is_subset(&self, other : &SymbolSet<PrettyInput>) -> bool {
        //Isn't this wonderful? Exactly how set theory defines it :3
        self.representations.iter().all(|x| other.representations.iter().any(|y| x == y))
    }
}