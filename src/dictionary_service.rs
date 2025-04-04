use anyhow::Result;
use eframe::egui::{self, Rect};

pub mod jpdb;

pub trait DictionaryService {
    fn name(&self) -> &'static str;

    fn init(&mut self) -> Result<()>;
    fn terminate(&mut self) -> Result<()>;
    fn config_gui(&mut self, ui: &mut egui::Ui);

    fn parse_text_rects(&mut self, text: &[(Rect, String)]) -> Result<Vec<(Rect, Vec<Word>)>>;
    fn add_to_deck(&mut self, word: &Word) -> Result<()>;
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

    fn parse_text_rects(&mut self, text: &[(Rect, String)]) -> Result<Vec<(Rect, Vec<Word>)>> {
        Ok(text
            .iter()
            .map(|(rect, string)| {
                (
                    *rect,
                    vec![Word {
                        text: TextWithRuby(vec![TextFragment {
                            text: string.clone(),
                            ruby: None,
                        }]),
                        definition: None,
                    }],
                )
            })
            .collect())
    }

    fn add_to_deck(&mut self, _word: &Word) -> Result<()> {
        Ok(())
    }
}
