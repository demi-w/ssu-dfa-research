use std::{sync::mpsc::{Sender,Receiver}, thread::JoinHandle, time::Duration};

use egui::{Ui, Color32, RichText};

use crate::{solver::{DFAStructure, SSStructure, Solver, SubsetSolver}, util::{DFA, Ruleset, SymbolIdx, SymbolSet}};
use crate::solver::MinkidSolver;

use crate::ui::{Instant,execute};

use std::path::PathBuf;

use super::{AvailableSolver, PrepPanel, Error};


pub struct DFAConstructor {
    dfa_reciever : Option<Receiver<(DFAStructure,SSStructure)>>,
    phase_reciever : Option<Receiver<Duration>>,
    pub dfa_content : Option<(DFAStructure,SSStructure)>,
    pub last_solver : Option<SolverContents>,
    pub final_dfa : Option<DFA>,
    pub iteration_state_lens : Vec<usize>,
    handle : Option<JoinHandle<DFA>>,
    solve_string : String,
    last_solve_string : Option<Vec<SymbolIdx>>,
    solve_path : Option<Result<Vec<(usize,usize,usize,Vec<SymbolIdx>)>,()>>,
    pub phase_content : Vec<Vec<Duration>>,
    pub phase_idx : usize,
    pub last_phase_msg : Instant,
    pub max_duration : f64,
    pub has_started : bool,
    pub has_finished : bool,
    pub initialization_dur : Option<Duration>,
    verify_run : bool,
    e_reporter : Sender<Error>
}

pub struct SolverContents {
    pub rules : Ruleset,
    pub goal : DFA,
    pub solve_type : AvailableSolver
}

impl DFAConstructor{

    pub fn new(e_reporter : Sender<Error>) -> Self {
        Self { 
            dfa_reciever : None,
            phase_reciever : None,
            dfa_content : None,
            final_dfa : None,
            handle : None,
            phase_content : vec![],
            phase_idx : 0,
            last_phase_msg : Instant::now(),
            max_duration : 0.0,
            has_started: Default::default(), 
            has_finished: Default::default(),
            solve_string : "".to_owned(),
            solve_path : None,
            last_solver : None,
            last_solve_string : None,
            verify_run : true,
            initialization_dur : None,
            iteration_state_lens : vec![],
            e_reporter : e_reporter
        }
    }

    pub fn update(&mut self, prep_panel : &mut PrepPanel) {
        if cfg!(not(target_arch = "wasm32")) {
            let mut handle = None;
            std::mem::swap(&mut handle, &mut self.handle);
            if let Some(h) = handle {
                if h.is_finished() {
                    let new_dfa = h.join().unwrap();
                    if self.verify_run && (self.final_dfa.is_none() || &new_dfa != self.final_dfa.as_ref().unwrap()) {
                        prep_panel.sig_k += 1;
                        let solve_ref: &SolverContents = self.last_solver.as_ref().unwrap();
                        self.run_dfa(solve_ref.solve_type.clone(), solve_ref.rules.clone(), solve_ref.goal.clone(), prep_panel.sig_k, true);
                    } else {
                        if self.verify_run {
                            prep_panel.sig_k -= 1;
                        }
                        self.has_finished = true;
                    }
                    self.final_dfa = Some(new_dfa);
    
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
                        if self.phase_content[0].is_empty() {
                            self.initialization_dur = Some(Instant::now() - self.last_phase_msg);
                        }
                        self.max_duration = message.as_secs_f64().max(self.max_duration);
                        self.phase_content[self.phase_idx].push(message);
                        self.phase_idx = (self.phase_idx + 1) % self.last_solver.as_ref().unwrap().solve_type.get_phases().len();
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
                        self.iteration_state_lens.push(message.0.len());
                        self.dfa_content = Some(message);
                    }
                    Err(reason) => {
                        match reason {
                            std::sync::mpsc::TryRecvError::Disconnected => {
                                if cfg!(target_arch = "wasm32") {
                                    let event = self.dfa_content.as_ref().unwrap();
                                    self.final_dfa = Some(crate::solver::event_to_dfa(&event.0,&event.1, &self.last_solver.as_ref().unwrap().rules));
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
        if self.has_started && self.phase_content[0].is_empty() {
            self.initialization_dur = Some(Instant::now() - self.last_phase_msg);
        }
    }
    pub fn update_solve_window(&mut self, ui : &mut Ui) {

    

    if let Some(dfa) = &self.final_dfa {
        ui.separator();
        if ui.button("Save DFA").clicked() {
            save_dfa(self.final_dfa.as_ref().unwrap().clone(),self.e_reporter.clone());
        }
        if let Some(solver) = &self.last_solver {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
            ui.label("String to solve:");
            ui.text_edit_singleline(&mut self.solve_string);
            if ui.button("Solve String").clicked() {
                match dfa.symbol_set.string_to_symbols(&self.solve_string.to_string().split(" ").collect()) {
                    Ok(input_str) => {
                        self.solve_path = Some(MinkidSolver::new(solver.rules.clone(),solver.goal.clone()).unwrap().solve_string_annotated(dfa, &input_str));
                        self.last_solve_string = Some(input_str);
                    }
                    Err(idx) => {
                        let _ = self.e_reporter.send(Error { 
                            title: "Unrecognized symbol".to_string(), 
                            body: RichText::new(format!("\"{}\" not recognized. Make sure to put a space in between symbols!",self.solve_string.to_string().split(" ").nth(idx).unwrap())) });
                    }
                }

            }
            });
            if let Some(solution_path) = &self.solve_path {
                if let Ok(path) = solution_path {
                    if path.len() == 0 {
                        ui.label("This string matches the goal DFA without any SRS applications.");
                    }else {
                        egui::containers::scroll_area::ScrollArea::vertical().id_source("solve_path").show(ui, |ui| {
                        ui.group(|ui| {
                            let start_str = self.last_solve_string.as_ref().unwrap();

                            render_path_element(ui, &dfa.symbol_set, start_str, &path[0].3, path[0].0, path[0].1, path[0].2);

                            for path_idx in 1..path.len() {
                                ui.separator();
                                render_path_element(ui, &dfa.symbol_set, &path[path_idx-1].3, &path[path_idx].3, path[path_idx].0, path[path_idx].1, path[path_idx].2);
                            }
                        });
                        });
                    }
                } else {
                    ui.label("According to the generated DFA, this string is unsolvable!");
                }
            }

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
    pub fn run_dfa(&mut self, solver : AvailableSolver, rules : Ruleset, goal : DFA, k : usize, verify_run : bool){
        
        match solver {
            AvailableSolver::Minkid => {
                match MinkidSolver::new(rules.clone(),goal.clone()) {
                    Ok(solver) => {self.run_dfa_arch(solver, k);}
                    Err(d_error) => {
                        let _ = self.e_reporter.send(Error { 
                                title: "Incompatible Solver".to_owned(), 
                                body: RichText::new(d_error.to_string(&rules.symbol_set))
                        });
                        return;
                    }
                }
                
            }
            AvailableSolver::Subset => {
                match SubsetSolver::new(rules.clone(),goal.clone()) {
                Ok(solver) => {self.run_dfa_arch(solver, k);}
                Err(d_error) => {
                    let _ = self.e_reporter.send(Error { 
                            title: "Incompatible Solver".to_owned(), 
                            body: RichText::new(d_error.to_string(&rules.symbol_set))
                    });
                    return;    
                }
            }
            }
        };

        self.verify_run = verify_run;
        self.final_dfa = None;
        
        self.phase_idx = 0;
        self.iteration_state_lens.clear();
        self.phase_content = vec![vec![]; solver.get_phases().len()];
        self.last_phase_msg = Instant::now();
        self.max_duration = 0.0;
        self.has_finished = false;
        self.has_started = true;
        self.last_solver = Some(SolverContents { rules: rules.clone(), goal: goal.clone(), solve_type : solver });
        self.last_phase_msg = Instant::now();

        
    }

}

#[cfg(not(target_arch = "wasm32"))]
fn save_dfa(dfa : DFA, e_sender : Sender<Error>) {

    let task = rfd::AsyncFileDialog::new().set_file_name("result.jff").add_filter(".dfa (used with pyscripts)", &["dfa"]).add_filter(".jff (used with jflap)", &["jff"]).save_file();
    let async_f = async move {
        let opened_file_r = task.await;
        
        if let Some(opened_file) = opened_file_r {
            let path = PathBuf::from(opened_file.file_name());
            match path.extension() {
                Some(extension) => {
                    if extension.to_str().unwrap() == "jff" {
                        if let Err(e) = opened_file.write(&dfa.save_jflap_to_bytes()).await {
                            let _ = e_sender.send(Error { title: "Unable to save".to_owned(), body: RichText::new(format!("{}",e)) });
                        }
                    }else if extension.to_str().unwrap() == "dfa" {
                        if let Err(e) = opened_file.write(&serde_json::to_string(&dfa).unwrap().as_bytes()).await {
                            let _ = e_sender.send(Error { title: "Unable to save".to_owned(), body: RichText::new(format!("{}",e)) });
                        }
                    } else {
                        let _ = e_sender.send(Error { title: "Invalid file extension".to_owned(), body: RichText::new("Needs to be .dfa of .jff.") });
                    }
                }
                None => {
                    let _ = e_sender.send(Error { title: "Invalid file extension".to_owned(), body: RichText::new("Needs to be .dfa of .jff.") });
                }
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

fn render_path_element(ui : &mut Ui, symset : &SymbolSet, lhs : &Vec<SymbolIdx>, rhs : &Vec<SymbolIdx>, lftmst_idx : usize, lhs_len : usize, rhs_len : usize) {
    ui.horizontal(|ui| {
        //Awkward spacing otherwise
        
        let mut print_vec = vec![0;lftmst_idx];
        if lftmst_idx > 0 {
            print_vec.clone_from_slice(&lhs[..lftmst_idx]);
            ui.monospace(format!("{}",symset.symbols_to_string(&print_vec)));
        }
        if lhs_len > 0 {
            print_vec = vec![0;lhs_len];
            print_vec.clone_from_slice(&lhs[lftmst_idx..(lftmst_idx+lhs_len)]);
            let rt = RichText::new(format!("{}",symset.symbols_to_string(&print_vec))).color(Color32::RED);
            ui.monospace(rt);
        }

        print_vec = vec![0;lhs.len()-lftmst_idx-lhs_len];
        print_vec.clone_from_slice(&lhs[(lftmst_idx+lhs_len)..]);
        ui.monospace(format!("{}",symset.symbols_to_string(&print_vec)));
    });
    ui.horizontal(|ui| {
        
        let mut print_vec = vec![0;lftmst_idx];
        if lftmst_idx > 0 {
            print_vec.clone_from_slice(&rhs[..lftmst_idx]);
            ui.monospace(format!("{}",symset.symbols_to_string(&print_vec)));
        }

        if rhs_len > 0 {
            print_vec = vec![0;rhs_len];
            print_vec.clone_from_slice(&rhs[lftmst_idx..(lftmst_idx+rhs_len)]);
            let rt = RichText::new(format!("{}",symset.symbols_to_string(&print_vec))).color(Color32::RED);
            ui.monospace(rt);
        }

        print_vec = vec![0;rhs.len()-lftmst_idx-rhs_len];
        print_vec.clone_from_slice(&rhs[(lftmst_idx+rhs_len)..]);
        ui.monospace(format!("{}",symset.symbols_to_string(&print_vec)));
    });
}