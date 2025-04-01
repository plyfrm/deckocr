use anyhow::Result;

pub trait DictionaryService {
    fn name(&self) -> &'static str;

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
