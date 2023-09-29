use std::path::PathBuf;

use egui::Ui;

use crate::{util::{DFA, Ruleset}, builder::{build_1dpeg_rs, build_onlyone1}};

use super::{open_file, OpenItem, PathSender, PathReciever};

pub struct PrepPanel {
    pub srs_text : String,
    pub sig_k : usize,
    pub goal : DFA,
    pub goal_name : String,
    pub rules : Ruleset,
    pub path_r : PathReciever,
    pub path_s : PathSender
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

        if ui.text_edit_multiline(&mut self.srs_text).changed() {
            self.rules = Ruleset::from_string(&self.srs_text);
        }

        ui.label(format!("Current goal DFA: {}",self.goal_name));
        if ui.button("Open Goal DFA").clicked() {
            open_file(OpenItem::Goal, self.path_s.clone());
        }
        if ui.button("Open SRS file").clicked() {
            open_file(OpenItem::SRS, self.path_s.clone());
        }
        ui.label("Signature Set Size: ");
        let mut tmp_value = if self.sig_k == 0 {"".to_owned()} else {format!("{}", self.sig_k)};
        let res = ui.text_edit_singleline(&mut tmp_value);
        if tmp_value == "" {
            self.sig_k = 0;
        }else if let Ok(result) = tmp_value.parse() {
            self.sig_k = result;
        }
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
            goal_name: "onlyone1.dfa".to_string(), 
            rules: rules,
            path_s : channel.0,
            path_r : channel.1,

         }
    }
}