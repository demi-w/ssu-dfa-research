#![warn(unused_crate_dependencies)]
#![warn(unused_import_braces)]
#![warn(unused_qualifications)]
#![windows_subsystem = "windows"]
use eframe::egui;
use egui::plot::Plot;

mod ui;
use ui::*;

use srs_to_dfa::util::Ruleset;

#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;


#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;


#[cfg(target_arch = "wasm32")]
use web_sys;


#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "SRS To DFA",
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
    e_reporter : ErrorReporter,
    open_generator_window : bool,
}

impl Default for MyApp {
    fn default() -> Self {
        let error_channel = std::sync::mpsc::channel();
        Self {
            c_visualizer : CVisualizer::default(),
            dfa_constructor : DFAConstructor::new(error_channel.0.clone()),
            prep_panel : PrepPanel::new(error_channel.0.clone()),
            e_reporter : ErrorReporter::new(error_channel.1),
            open_generator_window : false
        }
    }
}


impl eframe::App for MyApp {

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        self.e_reporter.update(ctx);

        egui::TopBottomPanel::top("my_panel").show(ctx, |ui| {
            ui.add_enabled_ui(!self.e_reporter.error_onscreen, |ui|{
            ui.horizontal(|ui|{
                self.prep_panel.topbar_update(ui);
                if ui.button("Generate DFA").clicked() {
                    self.open_generator_window = !self.open_generator_window;
                }
                
            });});
         });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_enabled_ui(!self.e_reporter.error_onscreen, |ui|{
            self.prep_panel.update(ui);
            self.dfa_constructor.update(&mut self.prep_panel);
            });
        });
        egui::Window::new("DFA Generator").collapsible(false).open(&mut self.open_generator_window).show(ctx, |ui| {
            ui.add_enabled_ui(!self.e_reporter.error_onscreen, |ui|{
            self.c_visualizer.update(ui, &self.dfa_constructor, &self.prep_panel);
            ui.add_enabled_ui((!self.dfa_constructor.has_started && !self.dfa_constructor.has_finished) || self.dfa_constructor.has_finished, |ui|{
            ui.horizontal_wrapped(|ui|{
            if self.prep_panel.solve_window_update(ui) { 
                self.dfa_constructor.run_dfa(self.prep_panel.solver_type,Ruleset::from_string(&self.prep_panel.srs_text),self.prep_panel.goal.clone(),self.prep_panel.sig_k,self.prep_panel.verify_run);
                Plot::new("my_plot").reset();
            }
            });
            self.dfa_constructor.update_solve_window(ui);
            });});
        });
        




    }
}

/* 
#[cfg(not(target_arch = "wasm32"))]
fn generate_obj_link(dfa : &DFA) -> String {
    "".to_owned()
}*/


/*#[cfg(not(target_arch = "wasm32"))]

pub fn wbf_fix<S : 'static, F: std::future::Future<Output = S> + 'static>(f: F) -> S {
    futures::executor::block_on(f)
}*/