use anyhow::Result;

use super::{dictionary::Word, Service, ServiceJob};

pub mod jpdb_srs;

pub type SrsInput = Word;
pub type SrsOutput = Result<()>;

pub trait SrsService: Service<SrsInput, SrsOutput> {
    fn add_to_deck(&mut self, word: SrsInput) -> ServiceJob<SrsOutput> {
        self.call(word)
    }
}

impl<T> SrsService for T where T: Service<SrsInput, SrsOutput> {}
