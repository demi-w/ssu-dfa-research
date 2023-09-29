use std::{sync::mpsc::Receiver, thread::JoinHandle, time::Duration};

use egui::Ui;

use crate::{solver::{DFAStructure, SSStructure, Solver}, util::{DFA, Ruleset}};
use crate::solver::MinkidSolver;

use crate::ui::Instant;


pub struct DFAConstructor {
    dfa_reciever : Option<Receiver<(DFAStructure,SSStructure)>>,
    phase_reciever : Option<Receiver<Duration>>,
    pub dfa_content : Option<(DFAStructure,SSStructure)>,
    last_rules : Option<Ruleset>,
    pub final_dfa : Option<DFA>,
    handle : Option<JoinHandle<DFA>>,
    pub phase_content : Vec<Vec<Duration>>,
    pub phase_idx : usize,
    pub last_phase_msg : Instant,
    pub max_duration : f64,
    pub has_started : bool,
    pub has_finished : bool
}

impl DFAConstructor {
    pub fn update(&mut self, ui : &mut Ui) {

    if cfg!(not(target_arch = "wasm32")) {
        let mut handle = None;
        std::mem::swap(&mut handle, &mut self.handle);
        if let Some(h) = handle {
            if h.is_finished() {
                self.final_dfa = Some(h.join().unwrap());
                self.has_finished = true;
            } else {
                self.handle = Some(h);
            }                
        }
    }
    //Phase messages loop
    loop {
        match &self.phase_reciever {
            Some(k_phase_recv) => match k_phase_recv.try_recv() {
                Ok(message) => {
                    self.max_duration = message.as_secs_f64().max(self.max_duration);
                    self.phase_content[self.phase_idx].push(message);
                    self.phase_idx = (self.phase_idx + 1) % MinkidSolver::get_phases().len();
                    self.last_phase_msg = Instant::now();
                    
                }
                Err(_) => {
                    break
                }
            },
            None => {
                break
            }
        }
    }
    //DFA messages loop
    loop {
        match &self.dfa_reciever {
            Some(k_dfa_recv) => match k_dfa_recv.try_recv() {
                Ok(message) => {
                    self.dfa_content = Some(message);
                }
                Err(reason) => {
                    match reason {
                        std::sync::mpsc::TryRecvError::Disconnected => {
                            if cfg!(target_arch = "wasm32") {
                                let event = self.dfa_content.as_ref().unwrap();
                                self.final_dfa = Some(crate::solver::event_to_dfa(&event.0,&event.1, self.last_rules.as_ref().unwrap()));
                            }
                            self.dfa_reciever = None;
                            self.has_finished = true;
                        },
                        std::sync::mpsc::TryRecvError::Empty => {
                            break; 
                        }
                    }
                }
            },
            None => {
                break;
            }
        }
    }

    if let Some(dfa) = &self.final_dfa {
        ui.label(format!("{} States",dfa.state_transitions.len()));
        if ui.button("Save DFA").clicked() {
            save_dfa(self.final_dfa.as_ref().unwrap().clone())
        }
        /*if cfg!(target_arch = "wasm32") {
            ui.hyperlink(&self.blob_link);
        }*/
    }


    }

    #[cfg(not(target_arch = "wasm32"))]
    fn run_dfa_arch<S>(&mut self, solver : S, k : usize) where S : Solver{
        let (dfa_rx, phase_rx, temp_h) = solver.run_debug(k); 
        self.dfa_reciever = Some(dfa_rx);
        self.phase_reciever = Some(phase_rx);
        self.handle = Some(temp_h);
    }
    #[cfg(target_arch = "wasm32")]
    fn run_dfa_arch<S>(&mut self, solver : S, k : usize) where S : Solver{
        let (dfa_rx, phase_rx) = solver.run_debug(k); 
        self.dfa_reciever = Some(dfa_rx);
        self.phase_reciever = Some(phase_rx);
    }
    pub fn run_dfa<S>(&mut self, solver : S, k : usize) where S : Solver{
        if self.has_started == true && self.has_finished == false {
            return;
        }
        self.final_dfa = None;
        self.phase_content = vec![vec![]; MinkidSolver::get_phases().len()];
        self.last_phase_msg = Instant::now();
        self.max_duration = 0.0;
        self.last_rules = Some(solver.get_ruleset().clone());
        self.has_finished = false;
        self.has_started = true;
        self.run_dfa_arch(solver, k);
    }

}

impl Default for DFAConstructor {
    fn default() -> Self {
        Self { 
            dfa_reciever : None,
            phase_reciever : None,
            dfa_content : None,
            final_dfa : None,
            handle : None,
            last_rules : None,
            phase_content : vec![vec![]; MinkidSolver::get_phases().len()],
            phase_idx : 0,
            last_phase_msg : Instant::now(),
            max_duration : 0.0,
            has_started: Default::default(), 
            has_finished: Default::default() }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_dfa(dfa : DFA) {
    use std::path::PathBuf;

    use crate::ui::execute;

    let task = rfd::AsyncFileDialog::new().add_filter("Recognized DFA types", &["dfa","jff"]).save_file();
    let async_f = async move {
        let opened_file_r = task.await;
        
        if let Some(opened_file) = opened_file_r {
            let path = PathBuf::from(opened_file.file_name());
            if path.extension().unwrap().to_str().unwrap() == "jff" {
                opened_file.write(&dfa.save_jflap_to_bytes()).await.unwrap();
            }else {
                opened_file.write(&serde_json::to_string(&dfa).unwrap().as_bytes()).await.unwrap();
            }
        }
    };
    execute(async_f);
}

#[cfg(target_arch = "wasm32")]
fn save_dfa(dfa : DFA) {
    let task = rfd::AsyncFileDialog::new().add_filter("Recognized DFA types", &["dfa","jff"]).pick_file();
    let async_f = async move {
        let opened_file_r = task.await;
        
        if let Some(opened_file) = opened_file_r {
            let path = PathBuf::from(opened_file.file_name());
            if path.extension().unwrap().to_str().unwrap() == "jff" {
                opened_file.write(&dfa.save_jflap_to_bytes()).await.unwrap();
            }else {
                opened_file.write(&serde_json::to_string(&dfa).unwrap().as_bytes()).await.unwrap();
            }
        }
    };
    execute(async_f);
}