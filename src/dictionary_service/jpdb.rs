use anyhow::anyhow;
use eframe::egui::{self, ahash::HashMap};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    config::Config,
    dictionary_service::{Definition, TextFragment, TextWithRuby, Word},
};

use super::DictionaryService;

const API_URL_PARSE: &'static str = "https://jpdb.io/api/v1/parse";
const API_URL_ADD_TO_DECK: &'static str = "https://jpdb.io/api/v1/deck/add-vocabulary";

#[derive(Default)]
pub struct Jpdb {
    config: JpdbConfig,
    word_ids: HashMap<String, (u64, u64)>,
}

impl DictionaryService for Jpdb {
    fn name(&self) -> &'static str {
        "jpdb"
    }

    fn init(&mut self) -> anyhow::Result<()> {
        self.config = JpdbConfig::load()?;

        Ok(())
    }

    fn terminate(&mut self) -> anyhow::Result<()> {
        self.config.save()?;

        Ok(())
    }

    fn config_gui(&mut self, ui: &mut egui::Ui) {
        self.config.gui(ui);
    }

    fn parse_text_rects(
        &mut self,
        text: &[(egui::Rect, String)],
    ) -> anyhow::Result<Vec<(egui::Rect, Vec<super::Word>)>> {
        let rects: Vec<_> = text.iter().map(|(rect, _)| *rect).collect();
        let text: Vec<_> = text.iter().map(|(_, s)| s).collect();

        let json: Value = attohttpc::post(API_URL_PARSE)
            .bearer_auth(&self.config.api_key)
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

        let tokens_json = json.get("tokens").map(Value::as_array).flatten().ok_or_else(|| {
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
        .ok_or_else(|| anyhow!("Malformed item in token list returned from `{API_URL_PARSE}`"))?;

        struct Vocabulary {
            vid: u64,
            sid: u64,
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
                    vid: word.get(0)?.as_u64()?,
                    sid: word.get(1)?.as_u64()?,
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
                self.word_ids.insert(
                    vocab_data.spelling.clone(),
                    (vocab_data.vid, vocab_data.sid),
                );
                vocab.push(vocab_data);
            }
            Some(())
        })()
        .ok_or_else(|| anyhow!("Malformed item in token list returned from `{API_URL_PARSE}`"))?;

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

        let words = rects
            .into_iter()
            .zip(words)
            .filter(|(_, words)| {
                if self.config.filter_paragraphs_with_no_definitions {
                    !words.iter().all(|word| word.definition.is_none())
                } else {
                    true
                }
            })
            .collect();

        Ok(words)
    }

    fn add_to_deck(&mut self, word: &Word) -> anyhow::Result<()> {
        let (vid, sid) = self
            .word_ids
            .get(
                &word
                    .definition
                    .as_ref()
                    .ok_or_else(|| anyhow!("Word has no definition"))?
                    .spelling,
            )
            .copied()
            .ok_or_else(|| anyhow!("Unknown word"))?;

        attohttpc::post(API_URL_ADD_TO_DECK)
            .bearer_auth(&self.config.api_key)
            .json(&json!({
                "id": self.config.deck_id,
                "vocabulary": [[vid, sid]],
                "occurences": [1],
                "replace_existing_occurences": true
            }))?
            .send()?
            .error_for_status()?;

        Ok(())
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct JpdbConfig {
    pub api_key: String,
    pub deck_id: u64,
    pub filter_paragraphs_with_no_definitions: bool,
}

impl Config for JpdbConfig {
    fn path() -> &'static str {
        "dictionary_services/jpdb.json"
    }

    fn gui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("API Key: ");
            ui.text_edit_singleline(&mut self.api_key);
        });
        ui.add(
            egui::DragValue::new(&mut self.deck_id)
                .speed(1)
                .prefix("Mining Deck ID: "),
        );
        ui.checkbox(
            &mut self.filter_paragraphs_with_no_definitions,
            "Filter out paragraphs with no definitions",
        );
    }
}
