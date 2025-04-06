use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::config::Config;
use crate::service::dictionary::jpdb::JpdbConfig;
use crate::service::ServiceJob;
use crate::service::{dictionary::jpdb::Jpdb, Service};

use super::{SrsInput, SrsOutput};

// This file only contains the code for using jpdb as an SRS. For jpdb configuration and other
// jpdb features, see `service/dictionary/jpdb.rs`.

const API_URL_PARSE: &'static str = "https://jpdb.io/api/v1/parse";
const API_URL_ADD_TO_DECK: &'static str = "https://jpdb.io/api/v1/deck/add-vocabulary";

impl Service<SrsInput, SrsOutput> for Jpdb {
    fn init(&mut self) -> Result<()> {
        self.config = JpdbConfig::load()?;
        Ok(())
    }

    fn terminate(&mut self) -> anyhow::Result<()> {
        self.config.save()?;
        Ok(())
    }

    fn show_config_ui(&mut self, ui: &mut eframe::egui::Ui) {
        self.config.srs_config_ui(ui);
    }

    fn call(&mut self, word: SrsInput) -> crate::service::ServiceJob<SrsOutput> {
        let config = self.config.clone();

        let spelling = word
            .definition
            .expect("the user should not be able to add words with no definitions to a deck")
            .spelling;

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
                    "id": config.deck_id,
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
