use anyhow::Result;
use eframe::egui;

pub trait DictionaryService {
    fn name(&self) -> &'static str;

    fn init(&mut self) -> Result<()>;
    fn terminate(&mut self) -> Result<()>;
    fn config_gui(&mut self, ui: &mut egui::Ui);

    fn parse_text_blocks(&mut self, text: &[&str]) -> Result<Vec<Vec<Word>>>;
    fn add_to_deck(&mut self, word: &Word) -> Result<()>;
}

pub struct Word {
    pub text: TextWithRuby,
    pub definition: Option<Definition>,
}

pub struct TextWithRuby(pub Vec<TextFragment>);

pub struct TextFragment {
    pub text: String,
    pub ruby: Option<String>,
}

pub struct Definition {
    pub spelling: TextWithRuby,
    pub reading: String,
    pub meanings: Vec<String>,
    pub frequency: u64,
}

pub struct DummyDictionaryService;

impl DictionaryService for DummyDictionaryService {
    fn name(&self) -> &'static str {
        "DummyDictionaryService"
    }

    fn init(&mut self) -> Result<()> {
        Ok(())
    }

    fn terminate(&mut self) -> Result<()> {
        Ok(())
    }

    fn config_gui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut false, "checkbox that does nothing");
    }

    fn parse_text_blocks(&mut self, _text: &[&str]) -> Result<Vec<Vec<Word>>> {
        Ok(Vec::new())
    }

    fn add_to_deck(&mut self, _word: &Word) -> Result<()> {
        Ok(())
    }
}
