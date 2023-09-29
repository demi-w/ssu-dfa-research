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
use srs_to_dfa::wbf_fix;
use srs_to_dfa::ui::*;
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



struct MyApp {
    dfa_constructor : DFAConstructor,
    prep_panel : PrepPanel,
    c_visualizer : CVisualizer,
    blob_link : String
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            c_visualizer : CVisualizer::default(),
            blob_link : "".to_owned(),
            dfa_constructor : DFAConstructor::default(),
            prep_panel : PrepPanel::default()
        }
    }
}


impl eframe::App for MyApp {

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        let symset_err = self.prep_panel.goal.symbol_set != self.prep_panel.rules.symbol_set;

        egui::CentralPanel::default().show(ctx, |ui| {

            if self.prep_panel.update(ui) {
                self.dfa_constructor.run_dfa(MinkidSolver::new(self.prep_panel.rules.clone(),self.prep_panel.goal.clone()),self.prep_panel.sig_k);
                Plot::new("my_plot").reset();
            }
            self.dfa_constructor.update(ui);
        });
        self.c_visualizer.update(ctx, &self.dfa_constructor);




    }
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