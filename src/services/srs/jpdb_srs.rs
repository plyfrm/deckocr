use std::{collections::BTreeMap, sync::Arc};

use anyhow::{anyhow, Context, Result};
use dashmap::DashMap;
use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::services::ServiceJob;
use crate::word::Word;
use crate::{config::Config, word::Definition};

use super::{CardState, SrsService};

// This file only contains the code for using jpdb as an SRS. For jpdb configuration and other
// jpdb features, see `service/dictionary/jpdb.rs`.

const API_URL_PARSE: &'static str = "https://jpdb.io/api/v1/parse";
const API_URL_LOOKUP: &'static str = "https://jpdb.io/api/v1/lookup-vocabulary";
const API_URL_ADD_TO_DECK: &'static str = "https://jpdb.io/api/v1/deck/add-vocabulary";
const API_URL_LIST_DECKS: &'static str = "https://jpdb.io/api/v1/list-user-decks";

#[derive(Default)]
pub struct JpdbSrs {
    config: JpdbSrsConfig,
    card_states_with_ids: Arc<DashMap<(u64, u64), usize>>,
    card_states_without_ids: Arc<DashMap<String, usize>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct JpdbSrsConfig {
    pub api_key: String,
    pub mining_deck_id: u64,

    pub card_states: [CardState; 7],

    #[serde(skip)]
    pub decks: BTreeMap<u64, String>,
}

impl Default for JpdbSrsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            mining_deck_id: 0,
            card_states: [
                CardState {
                    name: "unparsed".to_owned(),
                    colour: [255, 255, 255],
                    is_relevant: false,
                },
                CardState {
                    name: "not in deck".to_owned(),
                    colour: [0, 200, 255],
                    is_relevant: true,
                },
                CardState {
                    name: "new".to_owned(),
                    colour: [170, 240, 255],
                    is_relevant: true,
                },
                CardState {
                    name: "learning".to_owned(),
                    colour: [170, 240, 255],
                    is_relevant: true,
                },
                CardState {
                    name: "due".to_owned(),
                    colour: [255, 75, 60],
                    is_relevant: true,
                },
                CardState {
                    name: "known".to_owned(),
                    colour: [125, 255, 125],
                    is_relevant: false,
                },
                CardState {
                    name: "blacklisted".to_owned(),
                    colour: [192, 192, 192],
                    is_relevant: false,
                },
            ],
            decks: BTreeMap::new(),
        }
    }
}

impl Config for JpdbSrsConfig {
    fn path() -> &'static str {
        "srs_services/jpdb.json"
    }

    fn show_ui(&mut self, ui: &mut eframe::egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("API Key:");
            ui.text_edit_singleline(&mut self.api_key);
        });

        if self.decks.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Mining Deck ID:");
                ui.add(egui::DragValue::new(&mut self.mining_deck_id));
            });
        } else {
            ui.horizontal(|ui| {
                ui.label("Mining Deck:");
                egui::ComboBox::from_id_salt("jpdb_mining_deck")
                    .selected_text(
                        self.decks
                            .get(&self.mining_deck_id)
                            .cloned()
                            .unwrap_or_else(|| self.mining_deck_id.to_string()),
                    )
                    .show_ui(ui, |ui| {
                        for (id, name) in self.decks.iter().rev() {
                            ui.selectable_value(&mut self.mining_deck_id, *id, name);
                        }
                    });
            });
        }

        ui.collapsing("Card States", |ui| {
            ui.columns_const(|[col1, col2]| {
                for state in &mut self.card_states {
                    col1.horizontal(|ui| {
                        egui::color_picker::color_edit_button_srgb(ui, &mut state.colour);
                        ui.label(&state.name);
                    });
                    col2.checkbox(&mut state.is_relevant, "Is Relevant");
                }
            });
        });
    }
}

impl SrsService for JpdbSrs {
    fn init(&mut self) -> Result<()> {
        self.config =
            JpdbSrsConfig::load().context("JpdbSrs: Failed to load configuration file")?;

        let _ = (|| -> Option<()> {
            let decks: Value = attohttpc::post(API_URL_LIST_DECKS)
                .bearer_auth(&self.config.api_key)
                .json(&json!({
                    "fields": [
                        "id",
                        "name"
                    ]
                }))
                .ok()?
                .send()
                .ok()?
                .json()
                .ok()?;

            for deck in decks.get("decks")?.as_array()? {
                let id = deck.get(0)?.as_u64()?;
                let name = deck.get(1)?.as_str()?.to_owned();
                self.config.decks.insert(id, name);
            }
            Some(())
        })();

        Ok(())
    }

    fn terminate(&mut self) -> anyhow::Result<()> {
        self.config
            .save()
            .context("JpdbSrs: Failed to save configuration file")?;
        Ok(())
    }

    fn show_config_ui(&mut self, ui: &mut eframe::egui::Ui) {
        self.config.show_ui(ui);
    }

    fn add_to_deck(&mut self, word: &Word) -> ServiceJob<Result<()>> {
        let config = self.config.clone();

        let spelling = word
            .definition
            .as_ref()
            .expect("the user should not be able to add words with no definitions to a deck")
            .spelling
            .clone();

        ServiceJob::new(move || {
            let json: Value = attohttpc::post(API_URL_PARSE)
                .bearer_auth(&config.api_key)
                .json(&json!({
                    "text": [spelling],
                    "token_fields": [
                    ],
                    "vocabulary_fields": [
                        "vid",
                        "sid"
                    ]
                }))
                .unwrap()
                .send()
                .context("JpdbSrs: Failed to send http request")?
                .error_for_status()
                .context("JpdbSrs: Response status code is not a success code")?
                .json()
                .context("JpdbSrs: Response from server is not valid json")?;

            let ids = json
                .get("vocabulary")
                .map(|v| v.get(0))
                .flatten()
                .ok_or_else(|| anyhow!("Response from `{API_URL_PARSE}` did not contain a `vocabulary` field, or it was not an array containing at least one element"))?;

            let vid = ids
                .get(0)
                .map(|v| v.as_u64())
                .flatten()
                .ok_or_else(|| anyhow!("Data returned from `{API_URL_PARSE}` is incorrect."))?;

            let sid = ids
                .get(1)
                .map(|v| v.as_u64())
                .flatten()
                .ok_or_else(|| anyhow!("Data returned from `{API_URL_PARSE}` is incorrect."))?;

            attohttpc::post(API_URL_ADD_TO_DECK)
                .bearer_auth(&config.api_key)
                .json(&json!({
                    "id": config.mining_deck_id,
                    "vocabulary": [[vid, sid]],
                    "occurences": [1],
                    "replace_existing_occurences": true
                }))
                .unwrap()
                .send()
                .context("JpdbSrs: Failed to send http request")?
                .error_for_status()
                .context("JpdbSrs: Response status code is not a success code")?;

            Ok(())
        })
    }

    fn load_card_states(&mut self, words: Vec<Word>) -> ServiceJob<Result<()>> {
        let config = self.config.clone();

        let map_with_ids = Arc::clone(&self.card_states_with_ids);
        let map_without_ids = Arc::clone(&self.card_states_without_ids);

        // we do this in two steps here to ensure we get the right word if ids are set, since jpdb
        // can have different entries with the same spelling.

        let words_with_ids: Vec<_> = words
            .iter()
            .filter_map(|word| word.definition.as_ref())
            .filter_map(|definition| definition.jpdb_vid_sid)
            .collect();

        let words_without_ids: Vec<_> = words
            .iter()
            .filter_map(|word| word.definition.as_ref())
            .filter(|definition| definition.jpdb_vid_sid.is_none())
            .map(|definition| definition.spelling.clone())
            .collect();

        ServiceJob::new(move || -> Result<()> {
            if !words_without_ids.is_empty() {
                let json: Value = attohttpc::post(API_URL_PARSE)
                    .bearer_auth(&config.api_key)
                    .json(&json!({
                        "text": words_without_ids,
                        "token_fields": [],
                        "vocabulary_fields": [
                            "card_state"
                        ]
                    }))
                    .unwrap()
                    .send()
                    .context("JpdbSrs: Failed to send http request")?
                    .error_for_status()
                    .context("JpdbSrs: Response status code is not a success code")?
                    .json()
                    .context("JpdbSrs: Response from server is not valid json")?;

                let ids_and_states = json
                .get("vocabulary")
                .map(Value::as_array)
                .flatten()
                .ok_or_else(|| anyhow!("Response from `{API_URL_PARSE}` did not contain a `vocabulary` field, or it was not an array containing at least one element"))?;

                for (value, spelling) in ids_and_states.iter().zip(words_without_ids) {
                    (|| -> Option<()> {
                        if let Some(state_name) = value.get(0)?.as_str() {
                            if let Some((idx, _)) = config
                                .card_states
                                .iter()
                                .enumerate()
                                .find(|(_, state)| state.name == state_name)
                            {
                                map_without_ids.insert(spelling, idx);
                            }
                        } else {
                            map_without_ids.insert(spelling, 1);
                        }

                        Some(())
                    })()
                    .ok_or_else(|| anyhow!("Data returned from `{API_URL_PARSE}` is incorrect."))?;
                }
            }

            if !words_with_ids.is_empty() {
                let json: Value = attohttpc::post(API_URL_LOOKUP)
                    .bearer_auth(&config.api_key)
                    .json(&json!({
                        "list": words_with_ids,
                        "fields": ["card_state"]
                    }))
                    .unwrap()
                    .send()
                    .context("JpdbSrs: Failed to send http request")?
                    .error_for_status()
                    .context("JpdbSrs: Response status code is not a success code")?
                    .json()
                    .context("JpdbSrs: Response from server is not valid json")?;

                let states = json
                .get("vocabulary_info")
                .map(Value::as_array)
                .flatten()
                .ok_or_else(|| anyhow!("Response from `{API_URL_LOOKUP}` did not contain a `vocabulary_info` field, or it was not an array containing at least one element"))?;

                for (value, ids) in states.iter().zip(words_with_ids) {
                    (|| -> Option<()> {
                        if let Some(state_name) = value.get(0)?.get(0).map(Value::as_str).flatten()
                        {
                            if let Some((idx, _)) = config
                                .card_states
                                .iter()
                                .enumerate()
                                .find(|(_, state)| state.name == state_name)
                            {
                                map_with_ids.insert(ids, idx);
                            }
                        } else {
                            map_with_ids.insert(ids, 1);
                        }

                        Some(())
                    })()
                    .ok_or_else(|| {
                        anyhow!("Data returned from `{API_URL_LOOKUP}` is incorrect.")
                    })?;
                }
            }

            Ok(())
        })
    }

    fn card_state(&self, word: &Word) -> &CardState {
        match &word.definition {
            None => &self.config.card_states[0],
            Some(Definition {
                jpdb_vid_sid: Some(ids),
                ..
            }) => self
                .card_states_with_ids
                .get(ids)
                .map(|idx| &self.config.card_states[*idx.value()])
                .unwrap_or(&self.config.card_states[0]),
            Some(Definition {
                reading,
                jpdb_vid_sid: None,
                ..
            }) => self
                .card_states_without_ids
                .get(reading)
                .map(|idx| &self.config.card_states[*idx.value()])
                .unwrap_or(&self.config.card_states[0]),
        }
    }
}
