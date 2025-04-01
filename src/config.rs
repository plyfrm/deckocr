use std::{fs::File, path::Path};

use anyhow::{anyhow, Context, Result};
use global_hotkey::hotkey;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{dictionary_service::DictionaryService, ocr_service::OcrService};

pub fn load_config<T: DeserializeOwned, P: AsRef<Path>>(path: P) -> Result<T> {
    let mut config_path =
        dirs::config_dir().ok_or_else(|| anyhow!("Could not find suitable config diractory"))?;
    config_path.push(env!("CARGO_PKG_NAME"));
    config_path.push(path);

    let file = File::open(&config_path).with_context(|| {
        format!(
            "Could not open configuration file: `{}`",
            config_path.display()
        )
    })?;

    let config = serde_json::from_reader(file).with_context(|| {
        format!(
            "Could not read configuration file: `{}`",
            config_path.display(),
        )
    })?;

    Ok(config)
}

pub fn save_config<T: Serialize, P: AsRef<Path>>(path: P, config: &T) -> Result<()> {
    let mut config_path =
        dirs::config_dir().ok_or_else(|| anyhow!("Could not find suitable config diractory"))?;
    config_path.push(env!("CARGO_PKG_NAME"));
    config_path.push(path);

    let mut config_dir = config_path.clone();
    config_dir.pop();
    std::fs::create_dir_all(&config_dir).with_context(|| {
        format!(
            "Could not create configuration directory: `{}`",
            config_dir.display()
        )
    })?;

    let file = File::create(&config_path).with_context(|| {
        format!(
            "Could not write to configuration file: `{}`",
            config_path.display()
        )
    })?;

    serde_json::to_writer_pretty(file, config).with_context(|| {
        format!(
            "Could not serialise configuration file: `{}`",
            config_path.display()
        )
    })?;

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    // https://w3c.github.io/uievents-key/#keys-modifier
    pub hotkey_modifiers: hotkey::Modifiers,
    // https://w3c.github.io/uievents-code/
    pub hotkey_keycode: hotkey::Code,
    pub ocr_service: OcrServiceList,
    pub dictionary_service: DictionaryServiceList,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum OcrServiceList {
    MangaOcr,
}

impl Into<Box<dyn OcrService>> for OcrServiceList {
    fn into(self) -> Box<dyn OcrService> {
        todo!()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum DictionaryServiceList {
    Jpdb,
}

impl Into<Box<dyn DictionaryService>> for DictionaryServiceList {
    fn into(self) -> Box<dyn DictionaryService> {
        todo!()
    }
}
