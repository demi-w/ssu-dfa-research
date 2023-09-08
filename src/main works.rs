use std::fmt::{self, write};
use std::io::empty;
use strum::IntoEnumIterator; 
use strum_macros::EnumIter;
//use bit_set::Vec<bool>;
use std::mem;
use std::collections::{HashMap,HashSet};

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

fn bfs_solver(starting_board : Vec<bool>) -> bool{
    let mut cur_boards =Vec::new();
    let mut next_boards = vec![starting_board.clone()];
    let mut known_states = HashSet::<Vec<bool>>::new();
    let mut ones = 0;
    let len = starting_board.len();
    known_states.insert(starting_board);
    return match len >= 3{
        true => 
        {
        while next_boards.len() > 0 {
            std::mem::swap(&mut cur_boards,&mut next_boards);
            next_boards = Vec::new();
            let mut index = 0;
            ones = 0;
            for board in &cur_boards{
                let mut twobehind = false;
                let mut onebehind = board[0];
                if onebehind {ones += 1}
                let mut notbehind = board[1];
                if notbehind {ones += 1}
                while index+2 < board.len(){
                    twobehind = onebehind;
                    onebehind = notbehind;
                    notbehind = board[index+2];
                    if notbehind {ones += 1}
                    if twobehind && onebehind && !notbehind {
                        let mut new_board = board.clone();
                        new_board[index] = false;
                        new_board[index+1] = false;
                        new_board[index+2] = true;
                        next_boards.push(new_board);
                    } else if !twobehind && onebehind && notbehind {
                        let mut new_board = board.clone();
                        new_board[index] = true;
                        new_board[index+1] = false;
                        new_board[index+2] = false;
                        next_boards.push(new_board);
                    }
                    index+=1;
                }
            }
            if ones == 1{
                return true;
            }
        }
        false 
        },
        
        false =>
        {
        for i in &next_boards[0] {
            if *i {
                ones += 1;
            }
        }
        ones == 1
        }
    }
}

fn which_prefixes_solvable(board : &Vec<bool>) -> [bool; 8] {
    let mut results = [false;8];
    let mut new_board = board.clone();
    let board_len = board.len();
    new_board.push(false);
    new_board.push(false);
    new_board.push(false);
    for i in 0..8 {
        new_board[board_len] = (i / 4) % 2 == 1;
        new_board[board_len+1] = (i / 2) % 2 == 1;
        new_board[board_len+2] = i % 2 == 1;
        results[i] = bfs_solver(new_board.clone());
    }
    results
}

fn board_to_string(board : &Vec<bool>) -> String{
    let mut str = String::new();
    for i in board {
        match i {
            true => str.push('█'),
            false => str.push('░')
        }
    }
    str
}

fn group_to_string(group : &[bool; 8]) -> String{
    let mut str = "(".to_owned();
    let mut matches = 0;
    for i in 0..8 {
        if group[i] {
            if matches >= 1 {
                str.push(',');
            }
            matches +=1;
            for j in vec![4,2,1] {
                match (i / j) % 2 == 1 {
                    true => str.push('█'),
                    false => str.push('░')
                }
            }
            
        }
    }
    str.push(')');
    str
}

fn prefix_test(){
    //Group boards by what three-peg-combo they win/lose under
    //Example:
    //The empty set and 000 would be in the same group because they are solvable/unsolvable with the
    //same set of three bits afterward (options from now on) (succeed: 001, 010, 100, 110, 011) (fail: 000, 111)
    let mut cur_boards = HashMap::<[bool;8],Vec::<Vec<bool>>>::new();

    let mut new_boards : Vec::<([bool; 8],Vec<bool>)> = Vec::new();

    let mut new_board = Vec::<bool>::new();
    new_board.push(false);
    new_board.push(false);
    new_board.push(false);
    for i in 0..8 {
        new_board[0] = (i / 4) % 2 == 1;
        new_board[1] = (i / 2) % 2 == 1;
        new_board[2] = i % 2 == 1;
        let board_result = which_prefixes_solvable(&new_board);
        match cur_boards.get_mut(&board_result) {
            Some(board_vec) => board_vec.push(new_board.clone()),
            None => {cur_boards.insert(board_result,vec![new_board.clone()]);}
        }
    }

    //Starting at 0... didn't work. gonna start at size 3 and pray!
    //cur_boards.insert([false,true,true,true,true,true,true,false], vec![Vec::new()]);
    //This tests to make sure that no prefixes that are lumped together diverge at any point.
    //For example, if there exists some option Sigma* w such that 000w is solvable but w is not (or vice versa),
    //this proof idea breaks down and I go to cry.

    for substr in 0..20 {
        println!("new iteration (includes up to length {})", substr+3);
        println!("currently {} groups",cur_boards.len());
        //For each group
        for (prefix,boards) in &cur_boards{
            println!("len = {},win conditions = {}",boards.len(), group_to_string(prefix));
            //For each board in a group, let's see what the next options will be!
            let next_prefixes = match boards.first() {
                Some(board) => {
                    let mut new_board = board.clone();
                    println!("Reference board: {}",board_to_string(&new_board));
                    new_board.push(false);
                    let empty_add = which_prefixes_solvable(&new_board);
                    new_boards.push((empty_add.clone(),new_board.clone()));
                    *new_board.last_mut().expect("impossible") = true;
                    let full_add = which_prefixes_solvable(&new_board);
                    new_boards.push((full_add.clone(),new_board.clone()));
                    [empty_add,full_add]
                },
                None => [[false;8],[false;8]]
            };
            for board in boards{
                //The crying step
                let mut new_board = board.clone();
                new_board.push(false);
                let empty_add = which_prefixes_solvable(&new_board);
                new_boards.push((empty_add.clone(),new_board.clone()));
                *new_board.last_mut().expect("impossible") = true;
                let full_add = which_prefixes_solvable(&new_board);
                new_boards.push((full_add.clone(),new_board.clone()));
                
                if next_prefixes != [empty_add,full_add]{
                    println!("Damn.");
                    println!("Reference board: {}",board_to_string(boards.first().expect("impossible")));
                    println!("Problem board: {}",board_to_string(board));

                    println!("Reference: {:?}", next_prefixes);
                    println!("Problem: {:?}", [empty_add,full_add]);

                    return;
                }
            }
        }
        //Add every new board gained to our monolithic board-recorder
        while let Some(board) = new_boards.pop() {
            match cur_boards.get_mut(&board.0) {
                Some(board_vec) => board_vec.push(board.1),
                None => {cur_boards.insert(board.0,vec![board.1]);}
            }
        }
        println!("");
    }

}

fn main() {
    println!("Hello, world!");
    prefix_test();
    /*let p2n = Ruleset::<(Need,EF),Need> {
        name : "Puzzle To Needs".to_owned(),
        rules : Vec::new()
    };
    for group in EF::iter() {

    }
    println!("{}",p2n);*/
}
