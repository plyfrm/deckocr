use anyhow::Result;
use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::word::Word;

use super::ServiceJob;

pub mod jpdb_srs;

pub trait SrsService {
    /// Initialise the service.
    fn init(&mut self) -> Result<()>;
    /// Terminate the service.
    fn terminate(&mut self) -> Result<()>;

    /// Show the service's configuration UI.
    fn show_config_ui(&mut self, ui: &mut egui::Ui);

    /// Query the card states for the given words and stores them inside the `SrsService` for later retrieval.
    fn load_card_states(&mut self, words: Vec<Word>) -> ServiceJob<Result<()>>;
    /// Add the given word to the user's mining deck and update its internal card state.
    fn add_to_deck(&mut self, word: &Word) -> ServiceJob<Result<()>>;

    /// Retrieve the card state for a given word.
    fn card_state(&self, word: &Word) -> &CardState;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardState {
    /// Name of the card state.
    pub name: String,
    /// Colour associated with the card state.
    pub colour: [u8; 3],
    /// If this is `false`, words this card state is associated with will be skipped when the user moves their selection while holding R2.
    pub is_relevant: bool,
}
