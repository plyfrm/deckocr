use global_hotkey::hotkey;
use serde::{Deserialize, Serialize};

use crate::{dictionary_service::DictionaryService, ocr_service::OcrService};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    // https://w3c.github.io/uievents-key/#keys-modifier
    pub hotkey_modifiers: hotkey::Modifiers,
    // https://w3c.github.io/uievents-code/
    pub hotkey_keycode: hotkey::Code,
    pub ocr_service: OcrServiceList,
    pub dictionary_service: DictionaryServiceList,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum OcrServiceList {
    MangaOcr,
}

impl Into<Box<dyn OcrService>> for OcrServiceList {
    fn into(self) -> Box<dyn OcrService> {
        todo!()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DictionaryServiceList {
    Jpdb,
}

impl Into<Box<dyn DictionaryService>> for DictionaryServiceList {
    fn into(self) -> Box<dyn DictionaryService> {
        todo!()
    }
}
