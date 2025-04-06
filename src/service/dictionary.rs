use anyhow::Result;
use eframe::egui;

use super::ServiceJob;

pub mod jpdb_dictionary;

pub type DictionaryServiceJob = ServiceJob<Result<Vec<Vec<Word>>>>;

pub trait DictionaryService {
    fn init(&mut self) -> Result<()>;
    fn terminate(&mut self) -> Result<()>;

    fn show_config_ui(&mut self, ui: &mut egui::Ui);

    fn parse(&mut self, paragraphs: Vec<String>) -> DictionaryServiceJob;
}

#[derive(Debug, Clone)]
pub struct Word {
    pub text: TextWithRuby,
    pub definition: Option<Definition>,
}

#[derive(Debug, Hash, Clone)]
pub struct TextWithRuby(pub Vec<TextFragment>);

#[derive(Debug, Hash, Clone)]
pub struct TextFragment {
    pub text: String,
    pub ruby: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub spelling: String,
    pub reading: String,
    pub frequency: Option<u64>,
    pub meanings: Vec<String>,
    pub card_state: String,
}
