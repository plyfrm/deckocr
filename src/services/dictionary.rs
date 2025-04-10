use anyhow::Result;
use eframe::egui;

use crate::word::Word;

use super::ServiceJob;

pub mod jpdb_dictionary;

pub type DictionaryServiceJob = ServiceJob<Result<Vec<Vec<Word>>>>;

/// A dictionary service.
pub trait DictionaryService {
    /// Initialise the service (ie. load its configuration file, etc).
    fn init(&mut self) -> Result<()>;
    /// Terminate the service (ie. save its configuration file, etc).
    fn terminate(&mut self) -> Result<()>;

    /// Show the config UI for the service's configuration.
    fn show_config_ui(&mut self, ui: &mut egui::Ui);

    /// Parse a list of paragraphs into a list of list of words with definitions.
    fn parse(&mut self, paragraphs: Vec<String>) -> DictionaryServiceJob;
}
