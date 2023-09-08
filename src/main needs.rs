use std::fmt::{self, write};
use strum::IntoEnumIterator; 
use strum_macros::EnumIter;
#[derive(Debug,EnumIter,Clone,Copy)]
enum EF {
    Zero,
    One,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven
}

#[derive(Debug,EnumIter)]
enum Need {
    Start,
    Error,
    AllEmpty,
    Moated,
    FirstTwoR,
    FirstFourR,
    FirstOneR,
    FirstTwoL,
    AlternatingEndFull,
    AlternatingEndEmptyFirst,
    AlternatingEndFullFirst,
    AlternatingEndEmpty,
    Seven,
    SevenNeedsOne
}

enum Requirement {
    Empty,
    Full,
    Irrelevant
}

fn single_group_transform(value : EF) -> Vec<EF> {
    match value {
        EF::Six => vec![EF::Six,EF::One],
        EF::Three => vec![EF::Three,EF::Four],
        _ => vec![value]
    }
}

//Accounts for single-group transformation
fn probe(value : EF, query : [Requirement; 3]) -> bool {
    let mut f_result = false;
    
    for val in single_group_transform(value) {
        let mut intval = val as u8;
        let mut result = true;
        for i in 0..3 {
            result &= match &query[i] {
                Requirement::Empty => intval%2==0,
                Requirement::Full => intval%2==1,
                Requirement::Irrelevant => true
            };
            intval /= 2;
        }
        f_result |= result;
    }
    f_result
}

struct Ruleset<In,Out> where In : fmt::Debug, Out : fmt::Debug {
    rules : Vec<(In,Out)>,
    name : String
}

impl<In,Out> fmt::Display for Ruleset<In,Out> where In : fmt::Debug, Out : fmt::Debug{
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        write!(f, "Ruleset: {}", self.name);
        for rule in &self.rules {
            writeln!(f, "{0:?} -> {1:?}", rule.0, rule.1);
        }
        Ok(())
    }
}

fn main() {
    println!("Hello, world!");
    let p2n = Ruleset::<(Need,EF),Need> {
        name : "Puzzle To Needs".to_owned(),
        rules : Vec::new()
    };
    for group in EF::iter() {
        for priorNeed in Need::iter() {
            let out = match &priorNeed {
                Need::Start | Need::AllEmpty => {
                    match &group {
                        EF::Zero => Need::AllEmpty,
                        EF::One | EF::Six => Need::FirstOneR,
                        EF::Two => Need::AlternatingEndEmptyFirst,
                        EF::Three => Need::FirstTwoR,
                        EF::Four => Need::Moated,
                        EF::Five => Need::AlternatingEndFullFirst,
                        EF::Six => Need::FirstTwoL,
                        EF::Seven => Need::Seven
                        
                    }
                },
                Need::Error => Need::Error,
                Need::Moated => match &group {
                    EF::Zero => Need::Moated,
                    _ => Need::Error
                }
                Need::FirstOneR => match &group {
                    EF::Zero | EF::One => Need::Moated,
                    EF::Two => Need::AlternatingEndEmpty,
                    EF::Three => Need::AlternatingEndEmpty,
                    EF::Four => Need::AlternatingEndEmpty,
                    EF::Five => Need::FirstTwoR,
                    EF::Six => Need::Error, //Feel shaky on this -- best of luck out there
                    EF::Seven => Need::FirstFourR
                }
                Need::AlternatingEndEmpty => match &group {
                    EF::Zero | EF::One | EF::Two | EF::Three | EF::Four => Need::Moated,
                    EF::Five => Need::AlternatingEndFull,
                    EF::Six => Need::Moated,
                    EF::Seven => Need::SevenNeedsOne,
                }
                Need::AlternatingEndFull => match &group {
                    EF::Zero | EF::One => Need::Moated,
                    EF::Two => Need::AlternatingEndEmpty,
                    EF::Three => Need::Moated,
                    

                }
            }
        }
    }
    println!("{}",p2n);
}
