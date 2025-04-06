use anyhow::{anyhow, Result};
use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::Config;
use crate::service::dictionary::Word;
use crate::service::ServiceJob;

use super::SrsService;

// This file only contains the code for using jpdb as an SRS. For jpdb configuration and other
// jpdb features, see `service/dictionary/jpdb.rs`.

const API_URL_PARSE: &'static str = "https://jpdb.io/api/v1/parse";
const API_URL_ADD_TO_DECK: &'static str = "https://jpdb.io/api/v1/deck/add-vocabulary";

#[derive(Default)]
pub struct JpdbSrs {
    config: JpdbSrsConfig,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct JpdbSrsConfig {
    pub api_key: String,
    pub mining_deck_id: u64,
}

impl Config for JpdbSrsConfig {
    fn path() -> &'static str {
        "srs/jpdb.json"
    }

    fn show_ui(&mut self, ui: &mut eframe::egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("API Key:");
            ui.text_edit_singleline(&mut self.api_key);
        });
        ui.horizontal(|ui| {
            ui.label("Mining Deck ID:");
            ui.add(egui::DragValue::new(&mut self.mining_deck_id));
        });
    }
}

impl SrsService for JpdbSrs {
    fn init(&mut self) -> Result<()> {
        self.config = JpdbSrsConfig::load()?;
        Ok(())
    }

    fn terminate(&mut self) -> anyhow::Result<()> {
        self.config.save()?;
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
                }))?
                .send()?
                .error_for_status()?
                .json()?;

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
                }))?
                .send()?
                .error_for_status()?;

            Ok(())
        })
    }
}
