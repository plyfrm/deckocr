use anyhow::{anyhow, Result};
use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    config::Config,
    service::{
        dictionary::{Definition, TextFragment, TextWithRuby, Word},
        ServiceJob,
    },
};

use super::DictionaryService;

const API_URL_PARSE: &'static str = "https://jpdb.io/api/v1/parse";

#[derive(Default)]
pub struct JpdbDictionary {
    pub config: JpdbDictionaryConfig,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct JpdbDictionaryConfig {
    pub api_key: String,
    pub filter_paragraphs_with_no_definitions: bool,
}

impl Default for JpdbDictionaryConfig {
    fn default() -> Self {
        Self {
            api_key: "".to_owned(),
            filter_paragraphs_with_no_definitions: true,
        }
    }
}

impl Config for JpdbDictionaryConfig {
    fn path() -> &'static str {
        "dictionary_services/jpdb.json"
    }

    fn show_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("API Key:");
            ui.text_edit_singleline(&mut self.api_key);
        });
        ui.checkbox(
            &mut self.filter_paragraphs_with_no_definitions,
            "Filter out paragraphs with no definitions",
        );
    }
}

impl DictionaryService for JpdbDictionary {
    fn init(&mut self) -> anyhow::Result<()> {
        self.config = JpdbDictionaryConfig::load()?;
        Ok(())
    }

    fn terminate(&mut self) -> anyhow::Result<()> {
        self.config.save()?;
        Ok(())
    }

    fn show_config_ui(&mut self, ui: &mut egui::Ui) {
        self.config.show_ui(ui);
    }

    fn parse(&mut self, text: Vec<String>) -> ServiceJob<Result<Vec<Vec<Word>>>> {
        let config = self.config.clone();

        ServiceJob::new(move || {
            let json: Value = attohttpc::post(API_URL_PARSE)
                .bearer_auth(&config.api_key)
                .json(&json!({
                    "text": text,
                    "token_fields": [
                        "vocabulary_index",
                        "position",
                        "length",
                        "furigana"
                    ],
                    "vocabulary_fields": [
                        "vid",
                        "sid",
                        "spelling",
                        "reading",
                        "frequency_rank",
                        "meanings",
                        "card_state"
                    ]
                }))?
                .send()?
                .error_for_status()?
                .json()?;

            let tokens_json = json.get("tokens").map(Value::as_array).flatten().ok_or({
            anyhow!("Response from `{API_URL_PARSE}` did not contain a `tokens` field, or it was not an array")
        })?;

            let vocab_json = json.get("vocabulary").map(Value::as_array).flatten().ok_or_else(|| {
            anyhow!("Response from `{API_URL_PARSE}` did not contain a `vocabulary` field, or it was not an array")
        })?;

            struct Token {
                vocab_index: usize,
                position: usize,
                length: usize,
                furigana: Option<Vec<TextFragment>>,
            }

            let mut tokens = Vec::new();

            (|| {
                for line in tokens_json {
                    let mut v = Vec::new();

                    for token in line.as_array()? {
                        let furigana = if let Some(array) = token.get(3)?.as_array() {
                            let mut furigana = Vec::new();
                            for val in array {
                                if let Some(furi) = val.as_array() {
                                    furigana.push(TextFragment {
                                        text: furi.get(0)?.as_str()?.to_owned(),
                                        ruby: Some(furi.get(1)?.as_str()?.to_owned()),
                                    });
                                } else {
                                    furigana.push(TextFragment {
                                        text: val.as_str()?.to_owned(),
                                        ruby: None,
                                    });
                                }
                            }
                            Some(furigana)
                        } else {
                            None
                        };

                        v.push(Token {
                            vocab_index: token.get(0)?.as_u64()? as usize,
                            position: token.get(1)?.as_u64()? as usize,
                            length: token.get(2)?.as_u64()? as usize,
                            furigana,
                        });
                    }
                    tokens.push(v);
                }
                Some(())
            })()
            .ok_or_else(|| {
                anyhow!("Malformed item in token list returned from `{API_URL_PARSE}`")
            })?;

            struct Vocabulary {
                _vid: u64,
                _sid: u64,
                spelling: String,
                reading: String,
                frequency: Option<u64>,
                meanings: Vec<String>,
                card_state: Option<String>,
            }

            let mut vocab = Vec::new();

            (|| {
                for word in vocab_json {
                    let vocab_data = Vocabulary {
                        _vid: word.get(0)?.as_u64()?,
                        _sid: word.get(1)?.as_u64()?,
                        spelling: word.get(2)?.as_str()?.to_owned(),
                        reading: word.get(3)?.as_str()?.to_owned(),
                        frequency: word.get(4)?.as_u64(),
                        meanings: word
                            .get(5)?
                            .as_array()?
                            .iter()
                            .map(|v| v.as_str())
                            .flatten()
                            .map(str::to_owned)
                            .collect(),
                        card_state: word
                            .get(6)?
                            .get(0)
                            .map(|v| v.as_str().map(str::to_owned))
                            .flatten(),
                    };

                    vocab.push(vocab_data);
                }
                Some(())
            })()
            .ok_or_else(|| {
                anyhow!("Malformed item in token list returned from `{API_URL_PARSE}`")
            })?;

            let mut words = Vec::new();

            for (text, tokens) in text.iter().zip(tokens) {
                let mut cursor = 0;
                let mut vec = Vec::new();

                if tokens.is_empty() {
                    vec.push(Word {
                        text: TextWithRuby(vec![TextFragment {
                            text: (*text).clone(),
                            ruby: None,
                        }]),
                        definition: None,
                    });
                }

                for token in tokens {
                    if token.position > cursor {
                        // next token is ahead of the cursor, unparsed text ahead
                        vec.push(Word {
                            text: TextWithRuby(vec![TextFragment {
                                text: text[cursor..token.position].to_owned(),
                                ruby: None,
                            }]),
                            definition: None,
                        });
                    }
                    // we are now sure to be at the next token
                    let text = TextWithRuby(token.furigana.unwrap_or_else(|| {
                        vec![TextFragment {
                            text: text[token.position..token.position + token.length].to_owned(),
                            ruby: None,
                        }]
                    }));
                    let definition = Some(Definition {
                        spelling: vocab[token.vocab_index].spelling.clone(),
                        reading: vocab[token.vocab_index].reading.clone(),
                        frequency: vocab[token.vocab_index].frequency,
                        meanings: vocab[token.vocab_index].meanings.clone(),
                        card_state: vocab[token.vocab_index]
                            .card_state
                            .clone()
                            .unwrap_or_else(|| "not in deck".to_owned()),
                    });
                    vec.push(Word { text, definition });

                    cursor = token.position + token.length;
                }

                words.push(vec);
            }

            if config.filter_paragraphs_with_no_definitions {
                words.retain(|paragraph| paragraph.iter().any(|word| word.definition.is_some()));
            }

            Ok(words)
        })
    }
}
