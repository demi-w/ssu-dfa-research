use std::{sync::mpsc::Receiver, thread::JoinHandle, time::Duration, fmt::format};

use egui::{Ui, Color32, RichText};

use crate::{solver::{DFAStructure, SSStructure, Solver, RuleGraphRoot}, util::{DFA, Ruleset, SymbolIdx, SymbolSet}};
use crate::solver::MinkidSolver;

use crate::ui::{Instant,execute};

use std::path::PathBuf;


pub struct DFAConstructor<S> where S : Solver {
    dfa_reciever : Option<Receiver<(DFAStructure,SSStructure)>>,
    phase_reciever : Option<Receiver<Duration>>,
    pub dfa_content : Option<(DFAStructure,SSStructure)>,
    last_solver : Option<S>,
    pub final_dfa : Option<DFA>,
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
    k : usize,
    verify_run : bool,
    is_superset : Result<(),(RuleGraphRoot,usize,usize)>,
}

impl<S> DFAConstructor<S> where S : Solver{
    pub fn update(&mut self, ui : &mut Ui) {

    if cfg!(not(target_arch = "wasm32")) {
        let mut handle = None;
        std::mem::swap(&mut handle, &mut self.handle);
        if let Some(h) = handle {
            if h.is_finished() {
                let new_dfa = h.join().unwrap();
                self.is_superset = self.last_solver.as_ref().unwrap().is_superset(&new_dfa);
                if self.verify_run && (self.final_dfa.is_none() || &new_dfa != self.final_dfa.as_ref().unwrap()) && !self.is_superset.is_ok() {
                    self.k += 1;
                    self.run_dfa(self.last_solver.as_ref().unwrap().clone(), self.k, true);
                } else {
                    
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
                                self.final_dfa = Some(crate::solver::event_to_dfa(&event.0,&event.1, self.last_solver.as_ref().unwrap().get_ruleset()));
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
        match &self.is_superset {
            Ok(_) => ui.label("passes superset test!"),
            Err((rgr,source,target)) => ui.label(format!("{} {} {}",rgr.to_string(&dfa.symbol_set),source,target))
        };
        if ui.button("Save DFA").clicked() {
            save_dfa(self.final_dfa.as_ref().unwrap().clone())
        }
        if let Some(solver) = &self.last_solver {
            ui.label("String to solve:");
            ui.text_edit_singleline(&mut self.solve_string);
            if ui.button("Solve String").clicked() {
                let input_str = dfa.symbol_set.string_to_symbols(&self.solve_string.to_string().split(" ").collect()).unwrap();
                self.solve_path = Some(solver.solve_string_annotated(dfa, &input_str));
                self.last_solve_string = Some(input_str);
            }
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
    fn run_dfa_arch(&mut self, solver : S, k : usize){
        let (dfa_rx, phase_rx, temp_h) = solver.run_debug(k); 
        self.dfa_reciever = Some(dfa_rx);
        self.phase_reciever = Some(phase_rx);
        self.handle = Some(temp_h);
    }
    #[cfg(target_arch = "wasm32")]
    fn run_dfa_arch(&mut self, solver : S, k : usize){
        let (dfa_rx, phase_rx) = solver.run_debug(k); 
        self.dfa_reciever = Some(dfa_rx);
        self.phase_reciever = Some(phase_rx);
    }
    pub fn run_dfa(&mut self, solver : S, k : usize, verify_run : bool){
        if self.has_started == true && self.has_finished == false {
            return;
        }
        self.verify_run = verify_run;
        self.final_dfa = None;
        self.k = k;
        self.phase_content = vec![vec![]; MinkidSolver::get_phases().len()];
        self.last_phase_msg = Instant::now();
        self.max_duration = 0.0;
        self.has_finished = false;
        self.has_started = true;
        self.last_solver = Some(solver.clone());
        self.run_dfa_arch(solver, k);
    }

}

impl<S> Default for DFAConstructor<S> where S : Solver{
    fn default() -> Self {
        Self { 
            dfa_reciever : None,
            phase_reciever : None,
            dfa_content : None,
            final_dfa : None,
            handle : None,
            phase_content : vec![vec![]; MinkidSolver::get_phases().len()],
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
            is_superset : Ok(()),
            k : 5
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_dfa(dfa : DFA) {

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