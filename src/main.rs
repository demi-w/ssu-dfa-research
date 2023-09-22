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

pub mod util;
pub mod solver;
pub mod builder;
use crate::solver::{BFSSolver, MinkidSolver, Solver, DFAStructure, SSStructure};
use crate::util::*;
use crate::builder::build_default1dpeg;
use std::sync::mpsc::{Receiver, Sender};
use rfd::{self, FileHandle};

#[cfg(not(target_arch = "wasm32"))]
fn main() {
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
    Result,
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
    srs_text : String,
    goal : DFA,
    goal_name : String,
    rules : Ruleset,
    path_channel: (
        Sender<(String,rfd::FileHandle,OpenItem)>,
        Receiver<(String,rfd::FileHandle,OpenItem)>,
    )
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
            srs_text : temp_solver.rules.to_string(),
            goal : temp_solver.goal,
            goal_name : "onlyone1.dfa".to_string(),
            rules : temp_solver.rules,
            path_channel : std::sync::mpsc::channel(),
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
                } else {
                    self.handle = Some(h);
                }                
            }
        }
        //File opening loop
        loop {
            match self.path_channel.1.try_recv() {
                Ok(message) => {
                    match message.2 {
                        OpenItem::Goal => {
                            let path = PathBuf::from(message.1.file_name());
                            
                            self.goal_name = path.file_name().unwrap().to_os_string().into_string().unwrap();
                            match path.extension().unwrap().to_str().unwrap() {
                                "dfa" => {self.goal = serde_json::from_str(&message.0).unwrap()},
                                "jff" => {self.goal = DFA::load_jflap_from_string(&message.0)},
                                _ => {}
                            }
                        }
                        OpenItem::SRS => {
                            self.rules = Ruleset::from_string(&message.0);
                            self.srs_text = message.0;
                        }
                        OpenItem::Result => {

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
                        self.phase_content[self.phase_idx].push(message);
                        self.phase_idx = (self.phase_idx + 1) % MinkidSolver::get_phases().len();
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
                                    self.final_dfa = Some(crate::solver::event_to_dfa(&event.0,&event.1, &self.rules));
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
            egui::Window::new("Construction Progress").show(ctx, |ui| {
                Plot::new("my_plot")
                    .set_margin_fraction(egui::Vec2 { x: 0.1, y: 0.1 })
                    .legend(Legend::default())
                    .view_aspect(2.0).show(ui, |plot_ui| 
                    {
                        
                        for (i, name) in MinkidSolver::get_phases().iter().enumerate() {
                            let mut points = vec![[0.0,0.0];self.phase_content[i].len()];
                            for j in 0..self.phase_content[i].len() {
                                points[j] = [j as f64,self.phase_content[i][j].as_secs_f64()]
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
                self.run_dfa(temp_solver, 5);
            }
            if let Some(dfa) = &self.final_dfa {
                ui.label(format!("{} States",dfa.state_transitions.len()));
                if ui.button("Save DFA").clicked() {
                    
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
fn open_file(target : OpenItem, file_s: Sender<(String,rfd::FileHandle,OpenItem)>) {
    let task = match target {
        OpenItem::SRS => rfd::AsyncFileDialog::new().pick_file(),
        _ => rfd::AsyncFileDialog::new().add_filter("Text files", &["dfa","jff"]).pick_file()
    };
    
    let async_f = async move {
        let opened_file_r = task.await;
        
        if let Some(opened_file) = opened_file_r {
            let funk = opened_file.read().await;
            let file_str = String::from_utf8_lossy(&funk[..]);
            file_s.send((file_str.to_string(),opened_file,target)).unwrap();
        }
    };
    execute(async_f);
}
#[cfg(not(target_arch = "wasm32"))]
fn execute<F: std::future::Future<Output = ()> + 'static>(f: F) {
    futures::executor::block_on(f);
}

#[cfg(target_arch = "wasm32")]
fn execute<F: std::future::Future<Output = ()> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(f);
}