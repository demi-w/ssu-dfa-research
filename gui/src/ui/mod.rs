use std::{sync::mpsc::{Receiver, Sender}, fmt::Display};
use rfd::FileHandle;

use async_std::task;
mod prep_panel;
pub use prep_panel::PrepPanel;

mod constructor;
pub use constructor::DFAConstructor;

mod visualizer;
pub use visualizer::CVisualizer;

mod error;
pub use error::{ErrorReporter,Error};


pub type PathSender = Sender<(String,FileHandle,OpenItem)>;

pub type PathReciever = Receiver<(String,FileHandle,OpenItem)>;



#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;


#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

use srs_to_dfa::solver::*;

pub enum OpenItem {
    Goal,
    SRS
}

#[derive(Clone,Copy, PartialEq)]
pub enum AvailableSolver {
    Minkid,
    Subset,
    Hash,
    BFS
}

//This is an unfortunate hack as dynamic dispatch would completely change the way that solvers are run
impl AvailableSolver {
    fn get_phases(&self) -> &[&str] {
        match self {
            AvailableSolver::Minkid => MinkidSolver::PHASES,
            AvailableSolver::Subset => SubsetSolver::PHASES,
            AvailableSolver::BFS => <BFSSolver as Solver>::PHASES,
            AvailableSolver::Hash => HashSolver::PHASES
        }
    }
}

impl Display for AvailableSolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AvailableSolver::Minkid => write!(f,"Minkid"),
            AvailableSolver::Subset => write!(f,"Subset"),
            AvailableSolver::BFS => write!(f,"Multithreaded BFS"),
            AvailableSolver::Hash => write!(f,"Hash")
        }
        
    }
}

fn open_file(target : OpenItem, file_s: Sender<(String,FileHandle,OpenItem)>) {
    let task = match target {
        OpenItem::SRS => rfd::AsyncFileDialog::new().pick_file(),
        OpenItem::Goal => rfd::AsyncFileDialog::new().add_filter("Recognized DFA types", &["dfa","jff"]).pick_file(),
    };
    
    let async_f = async move {
        let opened_file_r = task.await;
        
        if let Some(opened_file) = opened_file_r {
            let funk = opened_file.read().await;
            let contents = String::from_utf8_lossy(&funk[..]).into_owned();
            file_s.send((contents,opened_file,target)).unwrap();
        }
    };
    execute(async_f);
}

//TODO: Spawn a new thread here to dramatically improve responsiveness
#[cfg(not(target_arch = "wasm32"))]
pub fn execute<F: std::future::Future<Output = ()> + 'static + Send>(f: F) {
    task::spawn(f);
}

#[cfg(target_arch = "wasm32")]
pub fn execute<F: std::future::Future<Output = ()> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(f);
}