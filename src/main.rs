#![warn(unused_crate_dependencies)]
#![warn(unused_import_braces)]
#![warn(unused_qualifications)]
use eframe::egui;
use egui::plot::{Plot, Line, Legend};
use std::fmt::format;
use std::fs::File;
use std::io::prelude::*;
use std::thread::JoinHandle;
use std::time::Duration;
use std::path::PathBuf;

use gloo_file::ObjectUrl;



use srs_to_dfa::solver::{BFSSolver, MinkidSolver, Solver, DFAStructure, SSStructure};
use srs_to_dfa::util::*;
use srs_to_dfa::builder::build_default1dpeg;
use srs_to_dfa::{wbf_fix,execute};
use std::sync::mpsc::{Receiver, Sender};
use rfd::{self, FileHandle};

#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;


#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;


use srs_to_dfa::test::*;


#[cfg(target_arch = "wasm32")]
use web_sys;


#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let oh_dear = DFA::jflap_load(&mut File::open("jffresults/lessbadcollatz.jff").unwrap());

    let goal = DFA::jflap_load(&mut File::open("example_goals/collatz_cycle.jff").unwrap());

    let mut ruleset_str = "".to_owned();
    File::open("srs/collatz").unwrap().read_to_string(&mut ruleset_str);
    let ruleset = Ruleset::from_string(&ruleset_str);

    let solver = MinkidSolver::new(ruleset, goal);

    solver.run_with_print(5);

    match solver.is_superset(&oh_dear) {
        Ok(_) => {println!("yippie")}
        Err(e) => {println!("{}",e.0.to_string(&solver.rules.symbol_set))}
    }

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "SRS Box",
        options,
        Box::new(|_cc| Box::new(MyApp::default())),
    ).unwrap();
}

#[cfg(target_arch = "wasm32")]
fn main() {

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "cs496", // hardcode it
                web_options,
                Box::new(|_cc| Box::new(MyApp::default())),
            )
            .await
            .expect("failed to start eframe");
    });
}

enum OpenItem {
    Goal,
    SRS
}

struct MyApp {
    dfa_reciever : Option<Receiver<(DFAStructure,SSStructure)>>,
    phase_reciever : Option<Receiver<Duration>>,
    dfa_content : Option<(DFAStructure,SSStructure)>,
    final_dfa : Option<DFA>,
    handle : Option<JoinHandle<DFA>>,
    phase_content : Vec<Vec<Duration>>,
    phase_idx : usize,
    last_phase_msg : Instant,
    srs_text : String,
    sig_k : usize,
    max_duration : f64,
    goal : DFA,
    goal_name : String,
    rules : Ruleset,
    path_channel: (
        Sender<(String,FileHandle,OpenItem)>,
        Receiver<(String,FileHandle,OpenItem)>,
    ),
    blob_link : String
}

impl Default for MyApp {
    fn default() -> Self {
        let temp_solver = build_default1dpeg::<BFSSolver>();
        Self {
            dfa_reciever : None,
            phase_reciever : None,
            dfa_content : None,
            final_dfa : None,
            handle : None,
            phase_content : vec![vec![]; MinkidSolver::get_phases().len()],
            phase_idx : 0,
            sig_k : 5,
            srs_text : temp_solver.rules.to_string(),
            goal : temp_solver.goal,
            goal_name : "onlyone1.dfa".to_string(),
            rules : temp_solver.rules,
            path_channel : std::sync::mpsc::channel(),
            last_phase_msg : Instant::now(),
            max_duration : 0.0,
            blob_link : "".to_owned()
        }
    }
}


impl eframe::App for MyApp {

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        if cfg!(not(target_arch = "wasm32")) {
            let mut handle = None;
            std::mem::swap(&mut handle, &mut self.handle);
            if let Some(h) = handle {
                if h.is_finished() {
                    self.final_dfa = Some(h.join().unwrap());
                    self.blob_link = generate_obj_link(self.final_dfa.as_ref().unwrap());
                } else {
                    self.handle = Some(h);
                }                
            }
        }
        //File opening loop
        loop {
            match self.path_channel.1.try_recv() {
                Ok((contents,fh, item_t)) => {
                    match item_t {
                        OpenItem::Goal => {
                            let path = PathBuf::from(fh.file_name());
                            println!("responding to new goal");
                            self.goal_name = path.file_name().unwrap().to_os_string().into_string().unwrap();
                            match path.extension().unwrap().to_str().unwrap() {
                                "dfa" => {self.goal = serde_json::from_str(&contents).unwrap()},
                                "jff" => {self.goal = DFA::load_jflap_from_string(&contents)},
                                _ => {}
                            }
                        }
                        OpenItem::SRS => {
                            self.rules = Ruleset::from_string(&contents);
                            println!("responded to new srs");
                            self.srs_text = contents;
                        }
                    }
                }
                Err(_) => {break}
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
                                    self.final_dfa = Some(srs_to_dfa::solver::event_to_dfa(&event.0,&event.1, &self.rules));
                                    self.blob_link = generate_obj_link(self.final_dfa.as_ref().unwrap());
                                }
                                self.dfa_reciever = None
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
        let symset_err = self.goal.symbol_set != self.rules.symbol_set;


        if let Some(_) = self.phase_reciever {
            let mut title = "Construction Progress - ".to_owned();
            title.push_str(&MinkidSolver::get_phases()[self.phase_idx]);
            egui::Window::new(title).show(ctx, |ui| {
                Plot::new("my_plot")
                    .legend(Legend::default())
                    .view_aspect(2.0)
                    .auto_bounds_x()
                    .auto_bounds_y()
                    .include_x(0.0)
                    .include_y(0.0)
                    .include_x(self.phase_content[0].len() as f32)
                    .include_y(self.max_duration)
                    .show(ui, |plot_ui| 
                    {
                        
                        for (i, name) in MinkidSolver::get_phases().iter().enumerate() {
                            let mut points = vec![[0.0,0.0];self.phase_content[i].len()];
                            for j in 0..self.phase_content[i].len() {
                                points[j] = [j as f64,self.phase_content[i][j].as_secs_f64()]
                            }
                            if self.dfa_reciever.is_some() && self.phase_idx == i {
                                points.push([points.len() as f64,(Instant::now() - self.last_phase_msg).as_secs_f64()]);
                            }
                            plot_ui.line(Line::new(points).name(name));
                        }
                    }
                
                );
            });
        }

        egui::CentralPanel::default().show(ctx, |ui| {

  
            if ui.text_edit_multiline(&mut self.srs_text).changed() {
                self.rules = Ruleset::from_string(&self.srs_text);
            }

            ui.label(format!("Current goal DFA: {}",self.goal_name));
            if ui.button("Open Goal DFA").clicked() {
                open_file(OpenItem::Goal, self.path_channel.0.clone());
            }
            if ui.button("Open SRS file").clicked() {
                open_file(OpenItem::SRS, self.path_channel.0.clone());
            }

            let solve_button = ui.add(egui::widgets::Button::new("Solve"));
            if solve_button.clicked() {
                let temp_solver = MinkidSolver::new(self.rules.clone(), self.goal.clone());
                self.final_dfa = None;
                self.phase_content = vec![vec![]; MinkidSolver::get_phases().len()];
                self.last_phase_msg = Instant::now();
                self.max_duration = 0.0;
                self.run_dfa(temp_solver, self.sig_k);
                Plot::new("my_plot").reset();
            }
            let mut tmp_value = if self.sig_k == 0 {"".to_owned()} else {format!("{}", self.sig_k)};
            ui.label("Signature Set Size: ");
            let res = ui.text_edit_singleline(&mut tmp_value);
            if tmp_value == "" {
                self.sig_k = 0;
            }else if let Ok(result) = tmp_value.parse() {
                self.sig_k = result;
            }
            
            if let Some(dfa) = &self.final_dfa {
                ui.label(format!("{} States",dfa.state_transitions.len()));
                if ui.button("Save DFA").clicked() {
                    save_dfa(self.final_dfa.as_ref().unwrap().clone())
                }
                if cfg!(target_arch = "wasm32") {
                    ui.hyperlink(&self.blob_link);
                }
            }


        });
    }
}
impl MyApp {
    #[cfg(not(target_arch = "wasm32"))]
    fn run_dfa<S>(&mut self, solver : S, k : usize) where S : Solver{
        let (dfa_rx, phase_rx, temp_h) = solver.run_debug(k); 
        self.dfa_reciever = Some(dfa_rx);
        self.phase_reciever = Some(phase_rx);
        self.handle = Some(temp_h);
    }
    #[cfg(target_arch = "wasm32")]
    fn run_dfa<S>(&mut self, solver : S, k : usize) where S : Solver{
        let (dfa_rx, phase_rx) = solver.run_debug(k); 
        self.dfa_reciever = Some(dfa_rx);
        self.phase_reciever = Some(phase_rx);
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
/* 
#[cfg(not(target_arch = "wasm32"))]
fn generate_obj_link(dfa : &DFA) -> String {
    "".to_owned()
}*/



#[cfg(target_arch = "wasm32")]
fn generate_obj_link(dfa : &DFA) -> String {
    let jeez = dfa.save_jflap_to_bytes();
    let ew = String::from_utf8_lossy(&jeez);
    let awk = ew.as_ref();
    let blob = gloo_file::File::new_with_options("result.jff",awk,Some("text/plain"),None);
    ObjectUrl::from(blob).to_string()
}

#[cfg(not(target_arch = "wasm32"))]
fn generate_obj_link(dfa : &DFA) -> String {
    "this shouldn't be visible.".to_owned()
}


/*#[cfg(not(target_arch = "wasm32"))]

pub fn wbf_fix<S : 'static, F: std::future::Future<Output = S> + 'static>(f: F) -> S {
    futures::executor::block_on(f)
}*/