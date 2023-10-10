use std::{time::Duration};

use egui::{Ui, plot::{Legend, Plot, Line}, Vec2};


use super::{DFAConstructor, PrepPanel};
use crate::ui::Instant;

#[derive(Default)]
pub struct CVisualizer {}

impl CVisualizer {
    pub fn update(&mut self, ui : &mut Ui, constructor : &DFAConstructor, prep_panel : &PrepPanel) {
        Plot::new("my_plot")
            .legend(Legend::default())
            .view_aspect(2.0)
            .auto_bounds_x()
            .auto_bounds_y()
            .include_x(0.0)
            .include_y(0.0)
            .allow_boxed_zoom(false)
            .allow_double_click_reset(false)
            .allow_drag(false)
            .allow_scroll(false)
            .allow_zoom(false)
            .set_margin_fraction(Vec2 {x : 0.3, y:0.1})
            .include_x(match constructor.phase_content.get(0) {Some(x) => x.len(), None =>{0}} as f32)
            .include_y(constructor.max_duration)
            .show(ui, |plot_ui| 
            {
                if constructor.has_started && constructor.phase_content[0].len() > 0 {
                    for (i, name) in constructor.last_solver.as_ref().unwrap().solve_type.get_phases().iter().enumerate() {
                        let mut points = vec![[0.0,0.0];constructor.phase_content[i].len()];
                        for j in 0..constructor.phase_content[i].len() {
                            points[j] = [j as f64,constructor.phase_content[i][j].as_secs_f64()]
                        }
                        if !constructor.has_finished && constructor.phase_idx == i {
                            points.push([points.len() as f64,(Instant::now() - constructor.last_phase_msg).as_secs_f64()]);
                        }
                        plot_ui.line(Line::new(points).name(name));
                    }
                }
            }
        
        );
        egui::Grid::new("gen_summary").striped(true).show(ui, |ui| {
        if let Some(dfa) = &constructor.dfa_content {
            
            let ss_element_len = dfa.1.element_len();
            if let Some(init_time) = constructor.initialization_dur {
                let total_time = init_time + constructor.phase_content.iter().map(|x|x.iter().sum::<Duration>()).sum();
                ui.label(format!("Total time: {}",total_time.as_secs_f64()));
                if let Some(state_len) = constructor.iteration_state_lens.last() {
                    let total_boards = ss_element_len + (state_len - 1) *constructor.last_solver.as_ref().unwrap().goal.symbol_set.length.pow(prep_panel.sig_k as u32 + 1);
                    ui.label(format!("Total # of strings processed: {}",total_boards));
                    
                    ui.label(format!("Strings processed per second: {:.2}", total_boards as f64 / total_time.as_secs_f64()));
                }
                ui.end_row();
                ui.label(format!("Initialization time: {}",constructor.initialization_dur.unwrap().as_secs_f64()));
            }
            if let Some(state_len) = constructor.iteration_state_lens.last() {
                ui.label(format!("Total states discovered: {}",state_len));
            } else {
                ui.label(format!("Total states discovered: N/A"));
            }
            if constructor.iteration_state_lens.len() >= 2 {
                let temp_len = constructor.iteration_state_lens.len();
                ui.label(format!("States created last iteration: {}",constructor.iteration_state_lens[temp_len-1]-constructor.iteration_state_lens[temp_len-2]));
            } else {
                ui.label("States created last iteration: 1");
            }
            ui.end_row();
        } else {
            ui.label("Total time: 0.0");
            ui.label("Total # of strings processed: 0");
            ui.label("Strings processed per second: N/A");
            ui.end_row();
            ui.label("Initialization time: 0.0");
            ui.label("Total states discovered: 0");
            ui.label("States created last iteration: N/A");
            ui.end_row();
        }
        }); 
    }
}