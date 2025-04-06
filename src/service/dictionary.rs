use anyhow::Result;

use super::{Service, ServiceJob};

pub mod jpdb;

pub type DictionaryInput = Vec<String>;
pub type DictionaryOutput = Result<Vec<Vec<Word>>>;

pub trait DictionaryService: Service<DictionaryInput, DictionaryOutput> {
    fn parse(&mut self, text: DictionaryInput) -> ServiceJob<DictionaryOutput> {
        self.call(text)
    }
}

impl<T> DictionaryService for T where T: Service<DictionaryInput, DictionaryOutput> {}

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
