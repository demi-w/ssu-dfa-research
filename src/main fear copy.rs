use std::fmt::{self, write, format};
use crossbeam::queue::{SegQueue, ArrayQueue};
use petgraph::graph::NodeIndex;
use petgraph::dot::{Dot, Config};
use strum_macros::EnumIter;
//use bit_set::Vec<bool>;
use std::mem;
use crossbeam;
use std::collections::{HashMap,HashSet};
use petgraph::Graph;
use std::fs;
use std::time;
use std::sync::Arc;
extern crate xml;

use std::fs::File;
use std::io::{self, Write};

use xml::writer::{EventWriter, EmitterConfig, XmlEvent, Result};


use std::thread;
#[macro_use]
extern crate lazy_static;

const SIGNATURE_K : usize = 6;

const SIGNATURE_LENGTH : usize = (2 << SIGNATURE_K)-1; // (2 ^ (SIGNATURE_K+1)) - 1. 
// e.g. K = 5; (2^6) - 1; the +1 comes from the way bitshifting works
//Total number of items in the signature, currently hardcoded to all binary options of len 0 <= x <= SIGNATURE_K

lazy_static! {
    
static ref SIGNATURE_ELEMENTS : [Vec<bool>;SIGNATURE_LENGTH] = {
    let mut start_index = 0;
    const EMPTY_VEC : Vec<bool> = vec![];
    let mut result : [Vec<bool>;SIGNATURE_LENGTH] = [EMPTY_VEC;SIGNATURE_LENGTH];
    let mut end_index = 1;
    let mut new_index = 1;
    for _ in 0..SIGNATURE_K {
        for i in start_index..end_index{
            result[new_index] = result[i].clone();
            result[new_index].push(false);
            new_index += 1;
            result[new_index] = result[i].clone();
            result[new_index].push(true);
            new_index += 1;
        }
        start_index = end_index;
        end_index = new_index;
    }
    result
};
}

struct DFA {
    starting_state : usize,
    state_transitions : Vec<[usize;2]>,
    accepting_states : HashSet::<usize>
}

#[derive(Clone, Debug)]
struct SignatureElement {
    board : Vec<bool>,
    signature : Vec<bool>
}

struct Ruleset<S> where S : Enum {
    max_input : usize,
    rules : Vec<(Vec<S>,Vec<S>),
}

impl DFA {
    fn is_accepting(&self, input : &Vec<bool>) -> bool {
        let mut state = self.starting_state;
        for i in input {
            state = self.state_transitions[state][*i as usize];
        }
        self.accepting_states.contains(&state)
    }
    fn save_to_file(&self,file : &mut File) {
        let mut w = EmitterConfig::new().perform_indent(true).create_writer(file);
        w.write(XmlEvent::start_element("structure")).unwrap();
        w.write(XmlEvent::start_element("type")).unwrap();
        w.write(XmlEvent::characters("fa")).unwrap();
        w.write(XmlEvent::end_element()).unwrap();
        w.write(XmlEvent::start_element("automaton")).unwrap();
        
        for (idx,i) in self.state_transitions.iter().enumerate() {
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
        for (idx,i) in self.state_transitions.iter().enumerate() {
            for (idx2,target) in i.iter().enumerate() {
                w.write(XmlEvent::start_element("transition")).unwrap();
                w.write(XmlEvent::start_element("from")).unwrap();
                w.write(XmlEvent::characters(&idx.to_string())).unwrap();
                w.write(XmlEvent::end_element()).unwrap();
                w.write(XmlEvent::start_element("to")).unwrap();
                w.write(XmlEvent::characters(&target.to_string())).unwrap();
                w.write(XmlEvent::end_element()).unwrap();
                w.write(XmlEvent::start_element("read")).unwrap();
                w.write(XmlEvent::characters(&idx2.to_string())).unwrap();
                w.write(XmlEvent::end_element()).unwrap();
                w.write(XmlEvent::end_element()).unwrap();
            }

        }
        w.write(XmlEvent::end_element()).unwrap();
        w.write(XmlEvent::end_element()).unwrap();
    }

    fn save(&self, filename : &str) {
        let mut file = File::create(filename.clone().to_owned() + ".jff").unwrap();
        self.save_to_file(&mut file);
    }

    fn verify_all_to_len(&self,n:usize){
        let total_boards =(2 << n)-1;
        println!("Starting DFA verification for strings <= {}. {} total boards",n, total_boards);

        let mut start_index = 0;
        let mut result : Vec::<Vec<bool>> = Vec::with_capacity(total_boards);
        result.push(vec![]);
        let mut end_index = 1;
        let mut new_index = 1;
        for _ in 0..n {
            for i in start_index..end_index{
                result.push(result[i].clone());
                result[new_index].push(false);
                new_index += 1;
                result.push(result[i].clone());
                result[new_index].push(true);
                new_index += 1;
            }
            start_index = end_index;
            end_index = new_index;
        }

        let input_queue: ArrayQueue<Vec<bool>> = ArrayQueue::new(total_boards);
        let a_input_queue = Arc::new(input_queue);
    
        let output_queue: ArrayQueue<(bool,Vec<bool>)> = ArrayQueue::new(total_boards);
        let a_output_queue = Arc::new(output_queue);

        while let Some(board) = result.pop() {
            a_input_queue.push(board).unwrap();
        }
        let mut handlers = Vec::with_capacity(64);
        for _ in 0..64 {
            handlers.push(
                board_solvability_consumer(a_input_queue.clone(), a_output_queue.clone())
            );
        }
        let mut num_recieved = 0;
        let mut num_accepting = 0;
        let mut num_waited = 0;
        while num_recieved < total_boards {
            let q_output = match a_output_queue.pop() {
                Some(output) => Some(output),
                None => {
                    match a_input_queue.pop() {
                        Some(board) => Some((bfs_solver(&board),board)),
                        None => {
                            None
                        }
                    }
                }
            };
            match q_output {
                Some((result, board)) => {
                    num_recieved += 1;
                    if (num_recieved) % (total_boards / 10) == 0 {
                        println!("{}% complete! ({} boards completed)", 100 * num_recieved / total_boards, num_recieved);
                    }
                    if result {num_accepting += 1}
                    if self.is_accepting(&board) != result {
                        println!("Damn. DFA-solvability failed.");
                        println!("Problem board: {}",board_to_string(&board));
                        println!("DFA: {}, BFS: {}",!result,result);
                        return;
                    }
                }
                None => {
                    num_waited+=1;
                    thread::sleep(time::Duration::from_millis(250));
                }
            }
            
        }
        println!("All verified! {}% accepting, ms spent sleeping: {}",num_accepting * 100 / total_boards,num_waited*250);

    }
}

/* 
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
}*/

fn bfs_solver(starting_board : &Vec<bool>) -> bool{
    let mut cur_boards = Vec::new();
    let mut next_boards = vec![starting_board.clone()];
    let mut known_states = HashSet::<Vec<bool>>::new();
    let mut ones = 0;
    known_states.insert(starting_board.clone());
    return match starting_board.len() >= 3{
        true => 
        {
        while next_boards.len() > 0 {
            std::mem::swap(&mut cur_boards,&mut next_boards);
            next_boards = Vec::new();
            for board in &cur_boards{
                let mut index = 0;
                ones = 0;
                let mut onebehind = board[0];
                if onebehind {ones += 1}
                let mut notbehind = board[1];
                if notbehind {ones += 1}
                while index+2 < board.len(){
                    let twobehind = onebehind;
                    onebehind = notbehind;
                    notbehind = board[index+2];
                    if notbehind {ones += 1}
                    
                    if twobehind && onebehind && !notbehind {
                        let mut new_board = board.clone();
                        new_board[index] = false;
                        new_board[index+1] = false;
                        new_board[index+2] = true;
                        if !known_states.contains(&new_board){
                            next_boards.push(new_board);
                        }
                    } else if !twobehind && onebehind && notbehind {
                        let mut new_board = board.clone();
                        new_board[index] = true;
                        new_board[index+1] = false;
                        new_board[index+2] = false;
                        if !known_states.contains(&new_board){
                            next_boards.push(new_board);
                        }
                    } 
                    /* 
                    else if twobehind && !onebehind && notbehind {
                        let mut new_board = board.clone();
                        new_board[index] = false;
                        new_board[index+1] = true;
                        new_board[index+2] = false;
                        if !known_states.contains(&new_board){
                            next_boards.push(new_board);
                        }
                    }*/
                    /*
                    let mut new_board = board.clone();
                    new_board[index] = !twobehind;
                    new_board[index+1] = !onebehind;
                    new_board[index+2] = !notbehind;
                    if !known_states.contains(&new_board) {
                        next_boards.push(new_board.clone());
                        known_states.insert(new_board);
                    } */
                    
                    index+=1;
                }
                if ones == 1{
                    return true;
                }
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
        ones == 0
        }
    }
}

fn which_prefixes_solvable(board : &Vec<bool>) -> [bool; SIGNATURE_LENGTH] {
    let mut results = [false;SIGNATURE_LENGTH];
    for (i,addition) in SIGNATURE_ELEMENTS.iter().enumerate() {
        let mut new_board = board.clone();
        new_board.extend(addition);
        results[i] = bfs_solver(&new_board);
        //println!("{},{}",board_to_string(&new_board),results[i]);
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
 
fn group_to_string(group : &[bool; SIGNATURE_LENGTH]) -> String{
    let mut str = "(".to_owned();
    let mut matches = 0;
    for (i,sig_element) in SIGNATURE_ELEMENTS.iter().enumerate(){
        if group[i] {
            if matches >= 1 {
                str.push(',');
            }
            matches +=1;
            str = str + &board_to_string(&sig_element);
            
        }
    }
    str.push(')');
    str
}

/* 
fn group_to_string(group : &[bool; SIGNATURE_LENGTH]) -> String {
    "Too big to print".to_owned()
}*/

fn group_builder_consumer(
    input_q : Arc<ArrayQueue<Vec<bool>>>, 
    output_q :  Arc<ArrayQueue<(([bool;SIGNATURE_LENGTH],Vec<bool>),([bool;SIGNATURE_LENGTH],Vec<bool>))>>
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while let Some(mut new_board) = input_q.pop() {
                //let mut new_board : Vec<bool> = new_board_ref.clone();

                output_q.push(board_to_next(new_board)).unwrap();
            }
        })
    }
fn board_solvability_consumer(
    input_q : Arc<ArrayQueue<Vec<bool>>>, 
    output_q :  Arc<ArrayQueue<(bool,Vec<bool>)>>
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while let Some(mut new_board) = input_q.pop() {
                //let mut new_board : Vec<bool> = new_board_ref.clone();

                output_q.push((bfs_solver(&new_board),new_board)).unwrap();
            }
        })
    }

fn signature_append_consumer(
    signature_set : &Vec<SignatureElement>,
    input_q : Arc<ArrayQueue<(usize,SignatureElement)>>,
    output_q : Arc<ArrayQueue<(usize,SignatureElement)>>
    ) -> thread::JoinHandle<()> {
        let my_signature_set = signature_set.clone();
        thread::spawn(move || {
            while let Some((idx, mut sig_element)) = input_q.pop() {
                for j in &my_signature_set {
                    let mut combined_board = sig_element.board.clone();
                    combined_board.append(&mut j.board.clone());
                    sig_element.signature.push(bfs_solver(&combined_board));
                }
                output_q.push((idx,sig_element)).unwrap();
            }
        })
    }

fn board_to_next(mut board : Vec<bool>) -> (([bool;SIGNATURE_LENGTH],Vec<bool>),([bool;SIGNATURE_LENGTH],Vec<bool>)) {
    board.push(false);
    let empty_board = board.clone();
    let empty_add = which_prefixes_solvable(&board);
    *board.last_mut().expect("impossible") = true;
    let full_add = which_prefixes_solvable(&board);
    ((empty_add,empty_board),(full_add,board))
}

fn smart_group_builder() -> HashMap::<Vec<bool>,Vec<SignatureElement>>
{
    const word_size : usize = 3;
    //This needs to flip old and new signature pieces but doesn't yet lol (thankflly we're working with symetrical systems)
    const start_sig_len : usize = (2 << word_size)-1;
    let mut start_index = 0;
    let mut signature_set : Vec<SignatureElement> =  vec![SignatureElement{board:vec![], signature:vec![]}];
    let mut end_index = 1;
    let mut new_index = 1;
    for _ in 0..3 {
        for i in start_index..end_index{
            signature_set.push(SignatureElement {
                board : signature_set[i].board.clone(),
                signature : Vec::new()
            }
            );
            signature_set[new_index].board.push(false);
            new_index += 1;
            signature_set.push(SignatureElement {
                board : signature_set[i].board.clone(),
                signature : Vec::new()
            }
            );
            signature_set[new_index].board.push(true);
            new_index += 1;
        }
        start_index = end_index;
        end_index = new_index;
    }
    let mut iter_count = 0;
    let mut last_signature_set = signature_set.clone();
    let silly_rust = signature_set.clone();
    signature_set = append_solvability(signature_set, &silly_rust);
    let start_set = signature_set.clone();
    // building 1st-order knowledge base
    let mut board_groups = boards_to_groups(&signature_set);
    //let mut new_board_groups = boards_to_groups(&signature_set,&signature_set);
    let mut new_count = 1;
    while new_count > 0 {
        new_count = 0;
        //finding 2nd-order additions
        let mut combo_boards = Vec::new();
        
        for i in &last_signature_set {
            //let mut solvability : Vec<bool> = Vec::new();
            for j in &start_set {
                //anything less is redundant 
                //
                if i.board.len() + j.board.len() > (word_size*(iter_count+1)) {
                    let mut combined_board = i.board.clone();
                    combined_board.append(&mut j.board.clone());
                    combo_boards.push(SignatureElement { 
                        board : combined_board,
                        signature : Vec::new()
                    });
                }
                //solvability.push(bfs_solver(&combined_board));
            }
            /* 
            //The actual proof (very slow)
            for j in &signature_set {
                //anything less is redundant 
                //
                if i.board.len() + j.board.len() > (word_size << (iter_count)) {
                    let mut combined_board = i.board.clone();
                    combined_board.append(&mut j.board.clone());
                    combo_boards.push(SignatureElement { 
                        board : combined_board,
                        signature : Vec::new()
                    });
                }
                //solvability.push(bfs_solver(&combined_board));
            }*/
        }
        println!("{} combo boards to build signatures for, {} board solutions necessary",combo_boards.len(),combo_boards.len()*signature_set.len());
        combo_boards = append_solvability(combo_boards, &signature_set);
        println!("solutions finished, sorting into groups");
        let second_board_groups = boards_to_groups(&combo_boards);
        let mut new_signature_elements = Vec::new();
        for (signature, mut elements) in second_board_groups {
            match board_groups.get(&signature) {
                Some(_) => {},
                None => {new_signature_elements.append(&mut elements); new_count += 1;}
                //None => {signature_set.push(elements.first_mut().unwrap().clone()); new_count += 1;}
            }
        }
        //println!("{}  boards solved", )
        println!("{} new groups, {} in total",new_count, new_count + board_groups.len());
        let mut silly_rust = new_signature_elements.clone();
        if new_count > 0 {
            
            new_signature_elements = append_solvability(new_signature_elements, &silly_rust);
            signature_set = append_solvability(signature_set, &new_signature_elements);
            signature_set.append(&mut new_signature_elements);
            board_groups = boards_to_groups(&signature_set);
            println!("{} groups after integration, {} active boards",board_groups.len(),signature_set.len());
        }
        std::mem::swap(&mut last_signature_set, &mut silly_rust);
        iter_count += 1;
    }
    println!("Process complete! hopefully this matches up with what I rambled about, it does still feel a lil shaky");
    board_groups
}

fn append_solvability(mut boards : Vec<SignatureElement>, signature_set : &Vec<SignatureElement>) -> Vec<SignatureElement>{

    let board_len = boards.len();
    let input_queue: ArrayQueue<(usize,SignatureElement)> = ArrayQueue::new(boards.len());
    let a_input_queue = Arc::new(input_queue);

    let output_queue: ArrayQueue<(usize,SignatureElement)> = ArrayQueue::new(boards.len());
    let a_output_queue = Arc::new(output_queue);
    let mut resulting_boards = Vec::with_capacity(board_len);
    let mut idx = boards.len();
    while let Some(i) = boards.pop() {
        idx -= 1;
        a_input_queue.push((idx,i));
        
    }

    for _ in 0..16 {
            signature_append_consumer(signature_set,a_input_queue.clone(), a_output_queue.clone());
            }
    let mut num_recieved = 0;

    for _ in 0..board_len {
        resulting_boards.push(SignatureElement{board:vec![],signature:vec![]})
    }
    //println!("0 completed boards, 0 board solutions");
    let max_len = " completed boards,  solutions needed  -".len() + board_len.to_string().len() + (board_len*signature_set.len()).to_string().len();
    const char_cycle : [char;4]= ['\\','|','/','-'];
    let mut cycle_idx = 0;
    while num_recieved < board_len {
        
        match a_output_queue.pop() {
            Some(output) => {
                let (idx, sig_element) = output;
                resulting_boards[idx] = sig_element;
                num_recieved += 1;
            },
            None => {thread::sleep(time::Duration::from_millis(250)); cycle_idx = (cycle_idx + 1) % 4}
        }
        let formatted_string = format!("{} boards remaining, {} solutions needed {}",board_len - num_recieved, (board_len - num_recieved)*signature_set.len(), char_cycle[cycle_idx]);
        
        //print!("\r{:<1$}",formatted_string,max_len);
        //std::io::stdout().flush();

    }
    print!("\r{:<1$}\r","",max_len);
    std::io::stdout().flush();
    resulting_boards
}

fn boards_to_groups(signature_set : &Vec<SignatureElement>) ->  HashMap<Vec<bool>,Vec<SignatureElement>> {
    let mut groups : HashMap<Vec<bool>,Vec<SignatureElement>> = HashMap::new();
    for i in signature_set {
        match groups.get_mut(&i.signature) {
            Some(group_boards) => group_boards.push(i.clone()),
            None => {groups.insert(i.signature.clone(),vec![i.clone()]);}
        }
    }
    groups
} 

fn fast_group_builder() -> HashMap::<[bool;SIGNATURE_LENGTH],Vec<Vec<bool>>> {
    let mut cur_boards = HashMap::<[bool;SIGNATURE_LENGTH],Vec<Vec<bool>>>::new();

    let mut new_boards : Vec::<Vec<bool>> = vec![vec![]];;

    let mut old_boards : Vec::<Vec<bool>> = Vec::new();

    cur_boards.insert(which_prefixes_solvable(&Vec::<bool>::new()),vec![Vec::<bool>::new()]);

    while new_boards.len() > 0 {
        std::mem::swap(&mut old_boards,&mut new_boards);
        new_boards.clear(); 
        println!("{} {}",old_boards.len(),old_boards[0].len());
        //TODO: Change to popping from old_boards
        for board in &old_boards {
            let (empty,full) = board_to_next(board.clone());
            for new_board in vec![empty,full] {
                //new_boards.push(new_board.1.clone());
                match cur_boards.get_mut(&new_board.0) {
                    Some(_) => {},
                    None => {
                        new_boards.push(new_board.1.clone());
                        cur_boards.insert(new_board.0,vec![new_board.1]);
                        }
                }
            }
        }
    }
    println!("{} groups constructed",cur_boards.len());
    cur_boards
}

fn exhaustive_group_builder() -> HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>{
    //Group boards by what three-peg-combo they win/lose under
    //Example:
    //The empty set and 000 would be in the same group because they are solvable/unsolvable with the
    //same set of three bits afterward (options from now on) (succeed: 001, 010, 100, 110, 011) (fail: 000, 111)

    
    let mut cur_boards = HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>::new();

    let mut new_boards : Vec::<Vec<bool>> = vec![vec![]];;

    let mut old_boards : Vec::<Vec<bool>> = Vec::new();

    cur_boards.insert(which_prefixes_solvable(&Vec::<bool>::new()),vec![Vec::<bool>::new()]);
    /* 
    let mut new_board = Vec::<bool>::new();
    new_board.push(false);
    new_board.push(false);
    new_board.push(false);
    for i in 0..8 {
        new_board[0] = (i / 4) % 2 == 1;
        new_board[1] = (i / 2) % 2 == 1;
        new_board[2] = i % 2 == 1;
        let board_result = which_prefixes_solvable(&new_board);
        //println!("{} {}",board_to_string(&new_board),group_to_string(&board_result));
        match cur_boards.get_mut(&board_result) {
            
            Some(board_vec) => board_vec.push(new_board.clone()),
            None => {cur_boards.insert(board_result,vec![new_board.clone()]);}
        }
    }*/

    //Starting at 0... didn't work. gonna start at size 3 and pray!
    //cur_boards.insert([false,true,true,true,true,true,true,false], vec![Vec::new()]);
    //This tests to make sure that no prefixes that are lumped together diverge at any point.
    //For example, if there exists some option Sigma* w such that 000w is solvable but w is not (or vice versa),
    //this proof idea breaks down and I go to cry.

    for substr in 0..10 {
        let total_boards = (2 << substr) - 1;
        let old_board_total = new_boards.len();

        println!("new iteration (includes up to length {})", substr);
        println!("currently {} groups",cur_boards.len());
        println!("{} total boards",total_boards);
        let now = time::Instant::now();
        let input_queue: ArrayQueue<Vec<bool>> = ArrayQueue::new(old_board_total);
        let a_input_queue = Arc::new(input_queue);
    
        let output_queue: ArrayQueue<(([bool;SIGNATURE_LENGTH],Vec<bool>),([bool;SIGNATURE_LENGTH],Vec<bool>))> = ArrayQueue::new(old_board_total);
        let a_output_queue = Arc::new(output_queue);
        //For each group
        std::mem::swap(&mut old_boards,&mut new_boards);
        new_boards.clear();
        for board in &old_boards{  
            //println!("{}",count);
            //println!("{}",board_to_string(&board));
            //The crying step
            a_input_queue.push(board.clone()).unwrap();
            
        }
        let mut handlers = Vec::with_capacity(64);
        for _ in 0..64 {
            handlers.push(
                group_builder_consumer(a_input_queue.clone(), a_output_queue.clone())
            );
        }
        let mut num_recieved = 0;
        
        while num_recieved < old_board_total {
            match a_output_queue.pop() {
                Some(output) => {
                    let (empty, full) = output;
                    for new_board in vec![empty,full] {
                        new_boards.push(new_board.1.clone());
                        match cur_boards.get_mut(&new_board.0) {
                            Some(board_vec) => board_vec.push(new_board.1),
                            None => {cur_boards.insert(new_board.0,vec![new_board.1]);}
                        }
                    }
                    num_recieved += 1;
                }
                None => {thread::sleep(time::Duration::from_millis(250))}
            }

        }
        println!("ms per board: {}",((time::Instant::now()-now)/(old_board_total as u32)).as_millis());
        println!("");
    }
    cur_boards
}

fn prefix_test(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>){
    //Group boards by what three-peg-combo they win/lose under
    //Example:
    //The empty set and 000 would be in the same group because they are solvable/unsolvable with the
    //same set of three bits afterward (options from now on) (succeed: 001, 010, 100, 110, 011) (fail: 000, 111)
    for (prefix,boards) in groups{
        //println!("len = {},win conditions = {}",boards.len(), group_to_string(prefix));
        //For each board in a group, let's see what the next options will be!
        let next_prefixes = match boards.first() {
            Some(board) => {
                let mut new_board = board.clone();
                //println!("Reference board: {}",board_to_string(&new_board));
                new_board.push(false);
                let empty_add = which_prefixes_solvable(&new_board);
                *new_board.last_mut().expect("impossible") = true;
                let full_add = which_prefixes_solvable(&new_board);
                [empty_add,full_add]
            },
            None => [[false;SIGNATURE_LENGTH],[false;SIGNATURE_LENGTH]]
        };
        for board in boards{
            //The crying step
            let mut new_board = board.clone();
            new_board.push(false);
            let empty_add = which_prefixes_solvable(&new_board);
            *new_board.last_mut().expect("impossible") = true;
            let full_add = which_prefixes_solvable(&new_board);

            if next_prefixes != [empty_add,full_add]{
                println!("Damn. Prefix test failed.");
                println!("Prefix group: {}", group_to_string(&prefix));
                println!("Reference board: {}",board_to_string(boards.first().expect("impossible")));
                println!("Problem board: {}",board_to_string(board));

                println!("Reference: {},{}", group_to_string(&next_prefixes[0]),group_to_string(&next_prefixes[1]));
                println!("Problem: {},{}", group_to_string(&empty_add),group_to_string(&full_add));

                return;
            }
        }
        //println!("Win conditions when 0 added: {}",group_to_string(&next_prefixes[0]));
        //println!("Win conditions when 1 added: {}",group_to_string(&next_prefixes[1]));
    }
    println!("Prefix test passed!")
}

fn group_solvability(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>) {
    for (prefix,boards) in groups{
        println!("len = {},win conditions = {}",boards.len(), group_to_string(prefix));
        //For each board in a group, let's see what the next options will be!
        let is_solvable = match boards.first() {
            Some(board) => {
                println!("Reference board: {}",board_to_string(board));
                bfs_solver(board)
            },
            None => false
        };
        for board in boards{
            if is_solvable != bfs_solver(&board){
                println!("Damn. Shared solvability failed.");
                println!("Prefix signature: {}",group_to_string(prefix));
                println!("Reference board: {}",board_to_string(boards.first().expect("impossible")));
                println!("Problem board: {}",board_to_string(board));

                println!("Reference: {:?}", is_solvable);
                println!("Problem: {:?}", bfs_solver(&board));

                return;
            }
        }
        println!("Is it solvable? {}",is_solvable);
    }
    println!("Solvability shared between groups!");
}

fn prefix_graph(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>) -> Graph::<String, String> {
    let mut group_graph = Graph::<String, String>::new();
    let mut group_idxs = HashMap::<[bool;SIGNATURE_LENGTH],NodeIndex>::new();

    for (prefix,boards) in groups{
        match boards.first() {
            Some(board) => {
                let solvable_char = match bfs_solver(&board) {
                    true => "Y",
                    false => "N"
                };
                let idx = group_graph.add_node(board_to_string(board) + solvable_char);
                group_idxs.insert(prefix.clone(),idx);
            },
            None => println!("ZOINKS")
        };
    }

    for (prefix,boards) in groups{
        //For each board in a group, let's see what the next options will be!
        match boards.first() {
            Some(board) => {
                let cur_idx = group_idxs.get(prefix).expect("no way jose");
                let mut new_board = board.clone();
                //println!("Reference board: {}",board_to_string(&new_board));
                new_board.push(false);
                let empty_add = which_prefixes_solvable(&new_board);
                let empty_idx = group_idxs.get(&empty_add).expect("no way jose");

                *new_board.last_mut().expect("impossible") = true;
                let full_add = which_prefixes_solvable(&new_board);
                let full_idx = group_idxs.get(&full_add).expect("no way jose");
                
                group_graph.add_edge(*cur_idx, *empty_idx, board_to_string(&vec![false]));
                group_graph.add_edge(*cur_idx, *full_idx, board_to_string(&vec![true]));
            },
            None => println!("ZOINKS")
        };
    }
    group_graph
}

fn identical_signature_elements(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>) {
    let mut identical_set = [true; SIGNATURE_LENGTH];
    let (reference_set,_) = groups.iter().next().unwrap();
    for (set,_) in groups {
        for i in 0..SIGNATURE_LENGTH {
            identical_set[i] &= reference_set[i] == set[i];
        }
    }
    print!("Identical boards:");
    for i in 0..SIGNATURE_LENGTH {
        if identical_set[i] {
            print!(" {},",board_to_string(&SIGNATURE_ELEMENTS[i]));
        }
    }
    println!("");
}
/*
written while high -- O(n^3) (i think) when it can be more interesting & O(n)
fn signature_element_groups(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>) {
    const empty_vec : Vec<bool> = Vec::new();
    let mut omnitable = [empty_vec;SIGNATURE_LENGTH];
    for (set,_) in groups {
        for i in 0..SIGNATURE_LENGTH {
            omnitable[i].push(set[i]);
        }
    }
    let mut meta_groups : HashMap::<Vec::<bool>,Vec::<usize>> = HashMap::new();
    for (idx,meta_element) in omnitable.iter().enumerate() {
        match meta_groups.get_mut(meta_element) {
            Some(meta_group) => meta_group.push(idx),
            None => {meta_groups.insert(meta_element.clone(),vec![idx]);}
        }
    }
    let mut idx = 0;
    for (meta_sig,meta_elements) in meta_groups {
        print!("Group {}: ",idx);
        for meta_element in meta_elements {
            print!(" {},",board_to_string(&SIGNATURE_ELEMENTS[meta_element]));
        }
        println!("");
        idx += 1;
    }
}*/

fn signature_element_groups(groups : &HashMap::<[bool;SIGNATURE_LENGTH],Vec::<Vec<bool>>>) {
    let (init_values,_) : (&[bool;SIGNATURE_LENGTH],&Vec::<Vec<bool>>) = groups.iter().next().unwrap();
    let mut s_or_d : Vec::<Vec<bool>> = Vec::new(); //Same or different
    //[i][j]
    //i = which signature element
    //j = which group
    for _ in 0..SIGNATURE_LENGTH{
        s_or_d.push(Vec::new());
    }
    for (set,_) in groups {
        for (idx,b) in set.iter().enumerate() {
            s_or_d[idx].push(init_values[idx] == *b);
        }
    }
    
    let mut s_or_d_groups : HashMap<Vec<bool>,Vec<usize>> = HashMap::new();
    for (idx,sig_element) in s_or_d.iter().enumerate() {
        match s_or_d_groups.get_mut(sig_element) {
            Some(group_idxs) => group_idxs.push(idx),
            None => {s_or_d_groups.insert(sig_element.clone(),vec![idx]);}
        }
    }
    let mut idx = 0;
    for (_, s_or_d_elements) in s_or_d_groups {
        print!("Group {}: ",idx);
        for s_or_d_element in s_or_d_elements {
            print!(" {},",board_to_string(&SIGNATURE_ELEMENTS[s_or_d_element]));
        }
        println!("");
        idx += 1;
    }
}

fn dot_parser(dot_output : String) -> String {
    dot_output.replace("\\\"", "")
    .replace("Y\" ]","\" shape=\"doublecircle\" ]")
    .replace("N\" ]","\" shape=\"circle\" ]")
}

fn dfa_builder() -> DFA {
    let mut trans_table : Vec<[usize;2]> = Vec::new(); //omg it's me !!!
    let mut table_reference = HashMap::<[bool;SIGNATURE_LENGTH],usize>::new();

    let mut new_boards : Vec::<Vec<bool>> = vec![vec![]];;

    let mut old_boards : Vec::<Vec<bool>> = Vec::new();

    let mut accepting_states : HashSet<usize> = HashSet::new();

    let start_accepting = which_prefixes_solvable(&Vec::<bool>::new());
    table_reference.insert(start_accepting.clone(),0);
    trans_table.push([0,0]);
    if bfs_solver(&Vec::<bool>::new()) {
        accepting_states.insert(0);
    }

    while new_boards.len() > 0 {
        std::mem::swap(&mut old_boards,&mut new_boards);
        new_boards.clear(); 
        println!("{} {}",old_boards.len(),old_boards[0].len());

        for board in &old_boards {
            let board_sig = which_prefixes_solvable(&board);
            let start_idx = *table_reference.get(&board_sig).unwrap();
            

            let (empty,full) = board_to_next(board.clone());
            for (sym_idx,new_board) in vec![empty,full].iter().enumerate() {

                let dest_idx = match table_reference.get(&new_board.0) {
                    Some(idx) => {
                        *idx
                    },
                    None => {
                        new_boards.push(new_board.1.clone());
                        let new_idx = trans_table.len();
                        
                        table_reference.insert(new_board.0,new_idx);
                        trans_table.push([0,0]);

                        if bfs_solver(&new_board.1) {
                            accepting_states.insert(new_idx);
                        }
                        new_idx
                        }
                    };
                trans_table[start_idx][sym_idx] = dest_idx;
                }  
                
            }
        }
    DFA {
        state_transitions : trans_table,
        accepting_states : accepting_states,
        starting_state : 0
    }
}

fn main() {

    //let groups = exhaustive_group_builder();
    //prefix_test(&groups);

    let groups = fast_group_builder(); 
    signature_element_groups(&groups);

    //println!("{:?}",*SIGNATURE_ELEMENTS);
    //println!("{}",group_to_string(&which_prefixes_solvable(&vec![false,false,false,true,true])))
    //bfs_solver(&vec![true,true,true,false,true,true,true,true]);
    //let groups = fast_group_builder(); 
    //let groups = exhaustive_group_builder();
    //identical_signature_elements(&groups);
    //signature_element_groups(&groups);//29 meta-groups for 1dpeg threerule
    //prefix_test(&groups);
    //let dfa = dfa_builder();
    //dfa.save("threerule1dpeg");
    //println!("{} states",dfa.state_transitions.len());
    
    /*let mut test_board = Vec::<bool>::new();
    let str = "01111011110110110".to_owned();
    for i in str.chars() {
        if i == '1' {
            test_board.push(true);
        } else {
            test_board.push(false);
        }
    }
    println!("ravi board: {}", dfa.is_accepting(&test_board));
    dfa.verify_all_to_len(16); */
    //group_solvability(&groups);
    //let graph = prefix_graph(&groups);
    
    //fs::write("output.dot", dot_parser(format!("{:?}",Dot::new(&graph)))).expect("Unable to write file");
    /*let p2n = Ruleset::<(Need,EF),Need> {
        name : "Puzzle To Needs".to_owned(),
        rules : Vec::new()
    };
    for group in EF::iter() {

    }
    println!("{}",p2n);*/
}
