use serde::{Deserialize, Serialize};
pub type SymbolIdx = u8;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct SymbolSet<PrettyInput = String> {
    pub length: usize,
    pub representations: Vec<PrettyInput>,
}

pub struct SymbolSetIter<'a, PrettyInput = String> {
    symset: &'a SymbolSet<PrettyInput>,
    k: usize,
    cur_vec: Vec<SymbolIdx>,
}

impl<'a, PrettyInput> Iterator for SymbolSetIter<'a, PrettyInput> {
    type Item = Vec<SymbolIdx>;

    fn next(&mut self) -> Option<Self::Item> {
        let result = if self.cur_vec.len() > self.k {
            None
        } else {
            Some(self.cur_vec.clone())
        };
        let mut rollover = self.cur_vec.len();
        while rollover > 0 && self.cur_vec[rollover - 1] == (self.symset.length - 1) as u8 {
            self.cur_vec[rollover - 1] = 0;
            rollover -= 1;
        }
        if rollover > 0 {
            self.cur_vec[rollover - 1] += 1;
        } else {
            self.cur_vec.push(0);
        }
        result
    }
}

impl<PrettyInput> SymbolSet<PrettyInput>
where
    PrettyInput: std::fmt::Display,
{
    pub fn symbols_to_string(&self, symbols: &Vec<SymbolIdx>) -> String {
        let mut string = "\"".to_owned();
        for sym in symbols {
            string += &format!("{} ", self.representations[*sym as usize]);
        }
        string.pop();
        string.push('"');
        string
    }
    pub fn string_to_symbols(&self, symbols: &Vec<&str>) -> Result<Vec<SymbolIdx>, usize> {
        let mut syms = vec![];
        for (idx, str) in symbols.iter().enumerate() {
            if str == &"" {
                continue;
            }
            match self
                .representations
                .iter()
                .position(|r| &format!("{}", r) == str)
            {
                Some(sym) => syms.push(sym as SymbolIdx),
                None => return Err(idx),
            }
        }
        Ok(syms)
    }
}
impl<PrettyInput> SymbolSet<PrettyInput> {
    pub fn new(mut representations: Vec<String>) -> SymbolSet {
        representations.sort();
        SymbolSet {
            length: representations.len(),
            representations: representations,
        }
    }

    pub fn sig_set_size(&self, k: usize) -> usize {
        let mut result = 0;
        for i in 0..(k+1) {
            result += self.length.pow(i as u32);
        }
        result
    }

    pub fn sig_set_iter(&self, k: usize) -> SymbolSetIter<PrettyInput> {
        SymbolSetIter {
            symset: self,
            k,
            cur_vec: vec![],
        }
    }
    //Returns the appropriate index for a certain string
    pub fn find_in_sig_set<'a>(&self, string: impl Iterator<Item = &'a SymbolIdx>) -> usize {
        let mut result = 0;
        for sym in string {
            result *= self.length;
            result += *sym as usize + 1;
        }
        result
    }
    pub fn idx_to_element(&self, mut idx: usize) -> Vec<SymbolIdx> {
        let mut result = vec![];
        while idx > 0 {
            idx -= 1;
            result.push((idx % self.length) as SymbolIdx);
            idx /= self.length;
        }
        result.reverse();
        result
    }
    pub fn build_sig_k(&self, k: usize) -> Vec<Vec<SymbolIdx>> {
        //let start_sig_len : usize = (cardinality::<S>() << k)-1;
        let mut start_index = 0;
        let mut signature_set: Vec<Vec<SymbolIdx>> = vec![vec![]];
        let mut end_index = 1;
        let mut new_index = 1;
        for _ in 0..k {
            for i in start_index..end_index {
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

impl<PrettyInput> SymbolSet<PrettyInput>
where
    PrettyInput: std::cmp::PartialEq,
{
    pub fn is_subset(&self, other: &SymbolSet<PrettyInput>) -> bool {
        //Isn't this wonderful? Exactly how set theory defines it :3
        self.representations
            .iter()
            .all(|x| other.representations.iter().any(|y| x == y))
    }
}
