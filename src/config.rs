use std::{collections::HashMap, fs::File};

use anyhow::{anyhow, Context, Result};
use eframe::egui;
use global_hotkey::hotkey;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
    dictionary_service::{jpdb::Jpdb, DictionaryService, DummyDictionaryService},
    ocr_service::{owocr::Owocr, OcrService},
};

// TODO: fix config issues
// with each service's config being part of its trait, we currently cannot change the config gui
// when the user changes which services they use

const CARD_STATE_DEFAULTS: &[(&str, [u8; 3])] = &[
    ("not in deck", [90, 220, 255]),
    ("new", [170, 240, 255]),
    ("due", [255, 100, 90]),
    ("known", [170, 255, 170]),
    ("blacklisted", [192, 192, 192]),
];

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    // https://w3c.github.io/uievents-key/#keys-modifier
    pub hotkey_modifiers: hotkey::Modifiers,
    // https://w3c.github.io/uievents-code/
    pub hotkey_keycode: hotkey::Code,
    pub ocr_service: OcrServiceList,
    pub dictionary_service: DictionaryServiceList,

    pub zoom_factor: f32,
    pub fullscreen: bool,

    pub card_colours: HashMap<String, [u8; 3]>,
}

impl Config for AppConfig {
    fn path() -> &'static str {
        "config.json"
    }

    fn gui(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_label("OCR Service")
            .selected_text(format!("{:?}", self.ocr_service))
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.ocr_service, OcrServiceList::Owocr, "owocr");
            });

        egui::ComboBox::from_label("Dictionary Service")
            .selected_text(format!("{:?}", self.dictionary_service))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.dictionary_service,
                    DictionaryServiceList::Jpdb,
                    "jpdb",
                );
                ui.selectable_value(
                    &mut self.dictionary_service,
                    DictionaryServiceList::Dummy,
                    "Dummy",
                );
            });

        ui.add(
            egui::DragValue::new(&mut self.zoom_factor)
                .prefix("UI Scaling: ")
                .range(0.5..=2.0)
                .speed(0.01)
                .custom_formatter(|n, _| format!("{}%", (n * 100.0) as i32))
                .custom_parser(|s| {
                    s.trim_end_matches('%')
                        .parse::<f64>()
                        .ok()
                        .map(|n| n / 100.0)
                }),
        );

        ui.checkbox(&mut self.fullscreen, "Fullscreen");

        for (card_state, _) in CARD_STATE_DEFAULTS {
            if let Some(srgb) = self.card_colours.get_mut(*card_state) {
                ui.horizontal(|ui| {
                    egui::color_picker::color_edit_button_srgb(ui, srgb);
                    ui.label(format!("'{card_state}' colour"));
                });
            }
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut card_colours = HashMap::new();
        for (card_state, srgb) in CARD_STATE_DEFAULTS {
            card_colours.insert((*card_state).to_owned(), *srgb);
        }

        Self {
            hotkey_modifiers: hotkey::Modifiers::ALT,
            hotkey_keycode: hotkey::Code::F12,
            ocr_service: OcrServiceList::Owocr,
            dictionary_service: DictionaryServiceList::Jpdb,

            zoom_factor: 1.0,
            fullscreen: true,

            card_colours,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum OcrServiceList {
    Owocr,
}

impl Into<Box<dyn OcrService + Send>> for OcrServiceList {
    fn into(self) -> Box<dyn OcrService + Send> {
        match self {
            Self::Owocr => Box::new(Owocr::default()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum DictionaryServiceList {
    Dummy,
    Jpdb,
}

impl Into<Box<dyn DictionaryService + Send>> for DictionaryServiceList {
    fn into(self) -> Box<dyn DictionaryService + Send> {
        match self {
            Self::Dummy => Box::new(DummyDictionaryService),
            Self::Jpdb => Box::new(Jpdb::default()),
        }
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
