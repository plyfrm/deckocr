use anyhow::Result;
use eframe::egui;

use crate::word::Word;

use super::ServiceJob;

pub mod jpdb_dictionary;

pub type DictionaryServiceJob = ServiceJob<Result<Vec<Vec<Word>>>>;

pub trait DictionaryService {
    fn init(&mut self) -> Result<()>;
    fn terminate(&mut self) -> Result<()>;

    fn show_config_ui(&mut self, ui: &mut egui::Ui);

    fn parse(&mut self, paragraphs: Vec<String>) -> DictionaryServiceJob;
}
