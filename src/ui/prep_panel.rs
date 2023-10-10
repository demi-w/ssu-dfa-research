use std::{path::PathBuf, sync::mpsc::Sender};

use egui::{Ui, RichText};

use crate::{util::{DFA, Ruleset}, builder::*};

use super::{open_file, OpenItem, PathSender, PathReciever, AvailableSolver, Error};

pub struct PrepPanel {
    pub srs_text : String,
    pub sig_k : usize,
    pub goal : DFA,
    path_r : PathReciever,
    path_s : PathSender,
    ruleset_pick : ExampleRulesets,
    goal_pick : ExampleGoals,
    pub verify_run : bool,
    pub solver_type : AvailableSolver,
    e_reporter : Sender<Error>
}


#[derive(PartialEq,Clone)]
enum ExampleRulesets {
    ThreeRuleSolver,
    DefaultSolver,
    OneDPeg,
    ThreeRuleOneDPeg,
    TwoxNSwap,
    Flip,
    ThreexNFlip,
    ThreexNPeg,
    Custom(String)
}
#[derive(PartialEq,Clone)]
enum ExampleGoals {
    All0,
    OnlyOne1,
    OnlyOne2,
    OneDPegResult,
    All000,
    OneDPegResultxThree,
    Custom(String)
}



impl ExampleGoals {
    fn to_dfa(&self) -> DFA {
        match self {
            Self::All0 => build_all0(),
            Self::OnlyOne1 => build_onlyone1(),
            Self::OnlyOne2 => build_onlyone2(),
            Self::OneDPegResult => build_1dpeg_result(),
            Self::OneDPegResultxThree => build_2dpeg_goal(),
            Self::All000 => build_all000(),
            Self::Custom(_) => build_onlyone1()
        }
    }
}

impl std::fmt::Display for ExampleGoals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = match self {
            Self::All0 => "All0.dfa",
            Self::All000 => "All000.dfa",
            Self::OnlyOne1 => "OnlyOne1.dfa",
            Self::OnlyOne2 => "OnlyOne2.dfa",
            Self::OneDPegResult => "1dPegResult.dfa",
            Self::OneDPegResultxThree => "1dPegResultx3.dfa",
            Self::Custom(str) => str
        };
        write!(f,"{}",val)
    }
}

impl std::fmt::Display for ExampleRulesets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = match self {
            Self::Flip => "1D Flip",
            Self::DefaultSolver => "1D Peg 'Solver'",
            Self::OneDPeg => "1D Peg Solitaire",
            Self::ThreeRuleOneDPeg => "1D Peg Three Rule Variant",
            Self::ThreeRuleSolver => "1D Peg Three Rule 'Solver'",
            Self::ThreexNFlip => "3xN Flip",
            Self::ThreexNPeg => "3xN Peg Solitaire",
            Self::TwoxNSwap => "2xN 'Swap' Peg Solitaire",
            Self::Custom(str) => str
        };
        write!(f,"{}",val)
    }
}

impl ExampleRulesets {

    fn to_srs(&self) -> Ruleset {
        match self {
            Self::Flip => build_flip_rs(),
            Self::ThreexNFlip => build_flipx3_rs(),
            Self::OneDPeg => build_1dpeg_rs(),
            Self::ThreeRuleOneDPeg => build_threerule1dpeg_rs(),
            Self::DefaultSolver => build_defaultsolver_rs(),
            Self::ThreeRuleSolver => build_threerulesolver_rs(),
            Self::ThreexNPeg => build_default2dpegx3_rs(),
            Self::TwoxNSwap => build_2xnswap_rs(),
            Self::Custom(_) => build_1dpeg_rs()
        }
    }
}

impl PrepPanel {


    pub fn new(e_reporter : Sender<Error>) -> Self {
        let rules = build_1dpeg_rs();
        let goal = build_onlyone1();
        let channel = std::sync::mpsc::channel();
        Self { 
            srs_text: rules.to_string(), 
            sig_k: 5, 
            goal: goal, 
            path_s : channel.0,
            path_r : channel.1,
            ruleset_pick : ExampleRulesets::OneDPeg,
            goal_pick : ExampleGoals::OnlyOne1,
            verify_run : true,
            solver_type : AvailableSolver::Minkid,
            e_reporter : e_reporter
            }
    }

    pub fn topbar_update(&mut self, ui : &mut Ui) {
        ui.menu_button("File", |ui| {

            if ui.button("Save SRS").clicked() {
                save_srs(self.srs_text.clone());
                ui.close_menu();
            }

            if ui.button("Open SRS from file").clicked() {
                open_file(OpenItem::SRS, self.path_s.clone());
                ui.close_menu();
            }

            ui.menu_button("Load example SRS", |ui|{
            for i in vec![ExampleRulesets::OneDPeg,
                                ExampleRulesets::ThreeRuleOneDPeg,
                                ExampleRulesets::DefaultSolver,
                                ExampleRulesets::ThreeRuleSolver,
                                ExampleRulesets::Flip,
                                ExampleRulesets::ThreexNFlip,
                                ExampleRulesets::TwoxNSwap,
                                ExampleRulesets::ThreexNPeg] {
                if ui.button(i.to_string()).clicked() {
                    self.ruleset_pick = i;
                    self.srs_text = self.ruleset_pick.to_srs().to_string();
                    ui.close_menu();
                }
            }
            });
        });
        
    }

    pub fn solve_window_update(&mut self, ui : &mut Ui) -> bool{
        let mut was_solve_asked = false;
        
        ui.vertical(|ui|{
            ui.separator();
        ui.horizontal(|ui|{
            ui.label("Estimated k-distinguishability: ");
            let mut tmp_value = if self.sig_k == 0 {"".to_owned()} else {format!("{}", self.sig_k)};
            ui.text_edit_singleline(&mut tmp_value).enabled();
            if tmp_value == "" {
                self.sig_k = 0;
            }else if let Ok(result) = tmp_value.parse() {
                self.sig_k = result;
            }
        });
        ui.horizontal(|ui|{
        ui.label("Select solver");
        egui::ComboBox::from_id_source("Select solver")
            .selected_text(format!("{}", self.solver_type))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.solver_type, AvailableSolver::Minkid, AvailableSolver::Minkid.to_string());
                ui.selectable_value(&mut self.solver_type, AvailableSolver::Subset, AvailableSolver::Subset.to_string());
            }
        );
        }); 
        ui.horizontal(|ui|{
        ui.label(format!("Current goal: {}",self.goal_pick));
        ui.menu_button("Select Goal", |ui| {
            if ui.button("Open Goal DFA").clicked() {
                ui.close_menu();
                open_file(OpenItem::Goal, self.path_s.clone());
            }
            ui.menu_button("Load example goal", |ui| {
                for i in vec![ExampleGoals::All0, ExampleGoals::OnlyOne1, ExampleGoals::OnlyOne2, ExampleGoals::All000,ExampleGoals::OneDPegResultxThree] {
                    if ui.button(i.to_string()).clicked() {
                        self.goal_pick = i;
                        self.goal = self.goal_pick.to_dfa();
                        ui.close_menu();
                    }
                }
            });
        }); });
        ui.horizontal(|ui|{
        ui.checkbox(&mut self.verify_run, "Verify Results of Generation?");
        was_solve_asked = ui.add(egui::widgets::Button::new("Generate")).clicked();
        });
        
        });
        was_solve_asked
    }

    pub fn update(&mut self, ui : &mut Ui) {
        loop {
            match self.path_r.try_recv() {
                Ok((contents,fh, item_t)) => {
                    match item_t {
                        OpenItem::Goal => {
                            let path = PathBuf::from(fh.file_name());
                            
                            match path.extension().unwrap().to_str().unwrap() {
                                "dfa" => {self.goal = serde_json::from_str(&contents).unwrap();self.goal_pick = ExampleGoals::Custom(path.file_name().unwrap().to_os_string().into_string().unwrap());},
                                "jff" => {self.goal = DFA::load_jflap_from_string(&contents);self.goal_pick = ExampleGoals::Custom(path.file_name().unwrap().to_os_string().into_string().unwrap());},
                                _ => {let _ = self.e_reporter.send(Error {title : "Unrecognized file type".to_owned(),body : RichText::new("Only .jff and .dfa files can be parsed")});}
                            }
                        }
                        OpenItem::SRS => {
                            let path = PathBuf::from(fh.file_name());
                            self.ruleset_pick = ExampleRulesets::Custom(path.file_name().unwrap().to_os_string().into_string().unwrap());
                            self.srs_text = contents;
                        }
                    }
                }
                Err(_) => {break}
            }
        }

        egui::ScrollArea::vertical().auto_shrink([false,false]).max_height(f32::INFINITY).show(ui, |ui| {
            let srs_editor = egui::TextEdit::multiline(&mut self.srs_text).frame(false).desired_width(f32::INFINITY).code_editor();
            ui.add_sized(ui.available_size(),srs_editor);
            });
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn save_srs(input : String) {
    use super::execute;


    let task = rfd::AsyncFileDialog::new().set_file_name("srs").save_file();
    let async_f = async move {
        let opened_file_r = task.await;
        
        if let Some(opened_file) = opened_file_r {
            opened_file.write(input.as_bytes()).await.unwrap();
        }
    };
    execute(async_f);
}