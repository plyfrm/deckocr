use anyhow::Result;
use eframe::egui;

use super::{dictionary::Word, ServiceJob};

pub mod jpdb_srs;

pub trait SrsService {
    fn init(&mut self) -> Result<()>;
    fn terminate(&mut self) -> Result<()>;

    fn show_config_ui(&mut self, ui: &mut egui::Ui);

    fn add_to_deck(&mut self, word: &Word) -> ServiceJob<Result<()>>;
}
