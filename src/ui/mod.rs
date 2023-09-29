use std::sync::mpsc::{Receiver, Sender};
use rfd::FileHandle;

mod prep_panel;
pub use prep_panel::PrepPanel;

mod constructor;
pub use constructor::DFAConstructor;

mod visualizer;
pub use visualizer::CVisualizer;

pub type PathSender = Sender<(String,FileHandle,OpenItem)>;

pub type PathReciever = Receiver<(String,FileHandle,OpenItem)>;


use egui::Ui;
#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;


#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;

pub enum OpenItem {
    Goal,
    SRS
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
pub fn execute<F: std::future::Future<Output = ()> + 'static>(f: F) {
    futures::executor::block_on(f);
}

#[cfg(target_arch = "wasm32")]
pub fn execute<F: std::future::Future<Output = ()> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(f);
}