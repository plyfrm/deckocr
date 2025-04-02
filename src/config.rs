use std::{fs::File, path::Path};

use anyhow::{anyhow, Context, Result};
use eframe::egui;
use global_hotkey::hotkey;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    dictionary_service::{DictionaryService, DummyDictionaryService},
    ocr_service::{DummyOcrService, OcrService},
};

// TODO: fix config issues
// with each service's config being part of its trait, we currently cannot change the config gui
// when the user changes which services they use

pub trait Config: Serialize + DeserializeOwned + Default {
    fn path() -> &'static str;
    fn gui(&mut self, ui: &mut egui::Ui);

    /// Loads a configuration file, or creates a default configuration struct if the file does not exist.
    fn load() -> Result<Self> {
        let mut config_path = dirs::config_dir()
            .ok_or_else(|| anyhow!("Could not find suitable config diractory"))?;
        config_path.push(env!("CARGO_PKG_NAME"));
        config_path.push(Self::path());

        if !config_path.exists() {
            Ok(Self::default())
        } else {
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
    }

    fn save(&self) -> Result<()> {
        let mut config_path = dirs::config_dir()
            .ok_or_else(|| anyhow!("Could not find suitable config diractory"))?;
        config_path.push(env!("CARGO_PKG_NAME"));
        config_path.push(Self::path());

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

        serde_json::to_writer_pretty(file, self).with_context(|| {
            format!(
                "Could not serialise configuration file: `{}`",
                config_path.display()
            )
        })?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    // https://w3c.github.io/uievents-key/#keys-modifier
    pub hotkey_modifiers: hotkey::Modifiers,
    // https://w3c.github.io/uievents-code/
    pub hotkey_keycode: hotkey::Code,
    pub ocr_service: OcrServiceList,
    pub dictionary_service: DictionaryServiceList,
}

impl Config for AppConfig {
    fn path() -> &'static str {
        "config.json"
    }

    fn gui(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_label("OCR Service")
            .selected_text(format!("{:?}", self.ocr_service))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.ocr_service, OcrServiceList::Dummy, "Dummy");
            });
        egui::ComboBox::from_label("Dictionary Service")
            .selected_text(format!("{:?}", self.dictionary_service))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.dictionary_service,
                    DictionaryServiceList::Dummy,
                    "Dummy",
                );
            });
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey_modifiers: hotkey::Modifiers::ALT,
            hotkey_keycode: hotkey::Code::KeyS,
            ocr_service: OcrServiceList::Dummy,
            dictionary_service: DictionaryServiceList::Dummy,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum OcrServiceList {
    Dummy,
}

impl Into<Box<dyn OcrService>> for OcrServiceList {
    fn into(self) -> Box<dyn OcrService> {
        Box::new(match self {
            Self::Dummy => DummyOcrService,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum DictionaryServiceList {
    Dummy,
}

impl Into<Box<dyn DictionaryService>> for DictionaryServiceList {
    fn into(self) -> Box<dyn DictionaryService> {
        Box::new(match self {
            Self::Dummy => DummyDictionaryService,
        })
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DummyConfig(bool);

impl Config for DummyConfig {
    fn path() -> &'static str {
        "dummy_config.json"
    }

    fn gui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.0, "checkbox that does nothing");
    }

    fn load() -> Result<Self> {
        Ok(Self(false))
    }

    fn save(&self) -> Result<()> {
        Ok(())
    }
}
