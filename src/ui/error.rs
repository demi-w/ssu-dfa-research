use std::{sync::mpsc::Receiver, collections::VecDeque};

use egui::Context;
use egui::RichText;

pub struct ErrorReporter {
    error_reciever : Receiver<Error>,
    error_queue : VecDeque<Error>,
    pub error_onscreen : bool
}

pub struct Error {
    pub title : String,
    pub body : RichText
}

impl ErrorReporter {

    pub fn new(reciever : Receiver<Error>) -> ErrorReporter {
        ErrorReporter { error_reciever: reciever, error_queue: VecDeque::new(), error_onscreen: false }
    }

    pub fn update(&mut self, ctx : &Context) {
        match self.error_reciever.try_recv() {
            Ok(message) => {
                self.error_queue.push_back(message);
            }
            Err(_) => {}
        }
        if !self.error_queue.is_empty() {
            let mut window_open = true;
            let mut button_clicked = false;
            egui::Window::new(format!("Error - {}",self.error_queue[0].title)).open(&mut window_open).movable(false).collapsible(false).resizable(false).show(ctx, |ui| {
                ui.vertical_centered(|ui|{
                    ui.label(self.error_queue[0].body.clone());
                    button_clicked = ui.button("Okay").clicked();
                });
            });
            if button_clicked || !window_open {
                self.error_queue.pop_front();
            }
        }
        self.error_onscreen = !self.error_queue.is_empty();
    }
}