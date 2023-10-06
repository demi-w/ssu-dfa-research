use egui::{Ui, plot::{Legend, Plot, Line}, Vec2};

use crate::solver::{MinkidSolver, Solver};

use super::DFAConstructor;
use crate::ui::Instant;

#[derive(Default)]
pub struct CVisualizer {}

impl CVisualizer {
    pub fn update<S>(&mut self, ctx : &egui::Context, constructor : &DFAConstructor<S>) where S : Solver {
        if constructor.has_started {
            let mut title = "Construction Progress - ".to_owned();
            title.push_str(&MinkidSolver::get_phases()[constructor.phase_idx]);
            egui::Window::new(title).id(egui::Id::new("CVisualizer")).show(ctx, |ui| {
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
                    .include_x(constructor.phase_content[0].len() as f32)
                    .include_y(constructor.max_duration)
                    .show(ui, |plot_ui| 
                    {
                        
                        for (i, name) in MinkidSolver::get_phases().iter().enumerate() {
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
                
                );
            });
        }
    }
}