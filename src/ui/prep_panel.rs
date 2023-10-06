use std::path::PathBuf;

use egui::Ui;

use crate::{util::{DFA, Ruleset}, builder::{build_1dpeg_rs, build_onlyone1, build_all0, build_onlyone2, build_1dpeg_result, build_flip_rs, build_flipx3_rs, build_threerule1dpeg_rs, build_defaultsolver_rs, build_threerulesolver_rs, build_2xnswap_rs}};

use super::{open_file, OpenItem, PathSender, PathReciever};

pub struct PrepPanel {
    pub srs_text : String,
    pub sig_k : usize,
    pub goal : DFA,
    path_r : PathReciever,
    path_s : PathSender,
    ruleset_pick : ExampleRulesets,
    goal_pick : ExampleGoals,
    pub verify_run : bool

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
    Custom(String)
}
#[derive(PartialEq,Clone)]
enum ExampleGoals {
    All0,
    OnlyOne1,
    OnlyOne2,
    OneDPegResult,
    Custom(String)
}

impl ExampleGoals {
    fn to_dfa(&self) -> DFA {
        match self {
            Self::All0 => build_all0(),
            Self::OnlyOne1 => build_onlyone1(),
            Self::OnlyOne2 => build_onlyone2(),
            Self::OneDPegResult => build_1dpeg_result(),
            Self::Custom(_) => build_onlyone1()
        }
    }
}

impl std::fmt::Display for ExampleGoals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = match self {
            Self::All0 => "All0.dfa",
            Self::OnlyOne1 => "OnlyOne1.dfa",
            Self::OnlyOne2 => "OnlyOne2.dfa",
            Self::OneDPegResult => "1dPegResult.dfa",
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
            Self::TwoxNSwap => build_2xnswap_rs(),
            Self::Custom(_) => build_1dpeg_rs()
        }
    }
}

impl PrepPanel {
    pub fn update(&mut self, ui : &mut Ui) -> bool {
        loop {
            match self.path_r.try_recv() {
                Ok((contents,fh, item_t)) => {
                    match item_t {
                        OpenItem::Goal => {
                            let path = PathBuf::from(fh.file_name());
                            println!("responding to new goal");
                            self.goal_pick = ExampleGoals::Custom(path.file_name().unwrap().to_os_string().into_string().unwrap());
                            match path.extension().unwrap().to_str().unwrap() {
                                "dfa" => {self.goal = serde_json::from_str(&contents).unwrap()},
                                "jff" => {self.goal = DFA::load_jflap_from_string(&contents)},
                                _ => {}
                            }
                        }
                        OpenItem::SRS => {
                            let path = PathBuf::from(fh.file_name());
                            self.ruleset_pick = ExampleRulesets::Custom(path.file_name().unwrap().to_os_string().into_string().unwrap());
                            println!("responded to new srs");
                            self.srs_text = contents;
                        }
                    }
                }
                Err(_) => {break}
            }
        }

        ui.horizontal(|ui| {
            egui::ScrollArea::vertical().max_height(800.0).show(ui, |ui| {
                let srs_editor = egui::TextEdit::multiline(&mut self.srs_text).code_editor();
                ui.add(srs_editor);
            });
            ui.group(|ui| {
            ui.vertical_centered(|ui|{
            ui.horizontal(|ui|{
                ui.label("Pick Goal:");
                ui.separator();                    
                ui.label("Pick SRS:");
            });
            //ui.separator();
            ui.horizontal(|ui|{
                let old_pick = self.goal_pick.clone();
                egui::ComboBox::from_id_source("Pick DFA")
                .selected_text(format!("{}", self.goal_pick))
                .show_ui(ui, |ui| {
                    for i in vec![ExampleGoals::All0, ExampleGoals::OnlyOne1, ExampleGoals::OnlyOne2] {
                        ui.selectable_value(&mut self.goal_pick, i.clone(), i.to_string());
                    }
                }
                );
                if old_pick != self.goal_pick {
                    self.goal = self.goal_pick.to_dfa();
                }
                ui.separator();
                let old_pick = self.ruleset_pick.clone();
                egui::ComboBox::from_id_source("Pick Ruleset")
                .selected_text(format!("{}", self.ruleset_pick))
                .show_ui(ui, |ui| {
                    for i in vec![ExampleRulesets::OneDPeg,
                                        ExampleRulesets::ThreeRuleOneDPeg,
                                        ExampleRulesets::DefaultSolver,
                                        ExampleRulesets::ThreeRuleSolver,
                                        ExampleRulesets::Flip,
                                        ExampleRulesets::ThreexNFlip,
                                        ExampleRulesets::TwoxNSwap] {
                        ui.selectable_value(&mut self.ruleset_pick, i.clone(), i.to_string());
                    }
                    if let ExampleRulesets::Custom(custom_str) = &self.ruleset_pick {
                        let mut _dummy_val =ExampleRulesets::DefaultSolver;
                        ui.selectable_value(&mut _dummy_val, ExampleRulesets::Custom(custom_str.to_owned()), custom_str);
                    }
                }
                );
                if old_pick != self.ruleset_pick {
                    self.srs_text = self.ruleset_pick.to_srs().to_string();
                }
            });
            //ui.separator();
            ui.horizontal(|ui|{
                if ui.button("Open Goal DFA").clicked() {
                    open_file(OpenItem::Goal, self.path_s.clone());
                }
                ui.separator();
                if ui.button("Open SRS file").clicked() {
                    open_file(OpenItem::SRS, self.path_s.clone());
                }
            });
            });
            });
            
        });
        ui.label("Signature Set Size: ");
        let mut tmp_value = if self.sig_k == 0 {"".to_owned()} else {format!("{}", self.sig_k)};
        ui.text_edit_singleline(&mut tmp_value);
        if tmp_value == "" {
            self.sig_k = 0;
        }else if let Ok(result) = tmp_value.parse() {
            self.sig_k = result;
        }
        ui.checkbox(&mut self.verify_run, "Verify Results of Generation?");
        ui.add(egui::widgets::Button::new("Solve")).clicked()
    }
}

impl Default for PrepPanel {
    fn default() -> Self {
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
            verify_run : true
         }
    }
}