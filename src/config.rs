use std::fs::File;

use anyhow::{anyhow, Context, Result};
use eframe::egui::{self};
use global_hotkey::hotkey;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::services::{
    dictionary::{jpdb_dictionary::JpdbDictionary, DictionaryService},
    ocr::{owocr::Owocr, OcrService},
    srs::{jpdb_srs::JpdbSrs, SrsService},
};

/// Represents a configuration file.
pub trait Config: Serialize + DeserializeOwned + Default {
    /// Relative path to the configuration file, assuming `./` is the deckocr configuration directory.
    fn path() -> &'static str;

    /// Show the UI for editing this config.
    fn show_ui(&mut self, ui: &mut egui::Ui);

    /// Load a configuration file, or create a default configuration struct if the file does not exist.
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

            // TODO: contruct a value manually from serde_json::Value so that we can easily migrate from old versions
            let config = serde_json::from_reader(file).with_context(|| {
                format!(
                    "Could not read configuration file: `{}`",
                    config_path.display(),
                )
            })?;

            Ok(config)
        }
    }

    /// Save a configuration file.
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

/// `deckocr`'s main configuration file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Modifiers for the OCR hotkey. Details: https://w3c.github.io/uievents-key/#keys-modifier
    pub hotkey_modifiers: hotkey::Modifiers,
    /// Keycode for the OCR hotkey. Details: https://w3c.github.io/uievents-code/
    pub hotkey_keycode: hotkey::Code,

    /// The OCR service selected by the user.
    pub ocr_service: OcrServiceList,
    /// The dictionary service selected by the user.
    pub dictionary_service: DictionaryServiceList,
    /// The SRS service selected by the user.
    pub srs_service: SrsServiceList,

    /// The UI scaling for the whole app. Passed to `egui::Context::set_zoom_factor`.
    pub zoom_factor: f32,
    /// Whether the OCR window should be shown in fullscreen.
    pub fullscreen: bool,
    /// Width of the OCR window.
    pub window_width: u32,
    /// Height of the OCR window.
    pub window_height: u32,
    /// How dim should the screenshot shown in the background of the OCR window be.
    pub background_dimming: u8,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey_modifiers: hotkey::Modifiers::ALT,
            hotkey_keycode: hotkey::Code::F12,

            ocr_service: OcrServiceList::Owocr,
            dictionary_service: DictionaryServiceList::Jpdb,
            srs_service: SrsServiceList::Jpdb,

            zoom_factor: 1.0,
            fullscreen: true,
            window_width: 1280,
            window_height: 720,
            background_dimming: 204,
        }
    }
}

impl Config for AppConfig {
    fn path() -> &'static str {
        "config.json"
    }

    fn show_ui(&mut self, ui: &mut egui::Ui) {
        let spacing = 5.0;

        // TODO: let the user set the hotkey from the config panel directly
        ui.add_enabled_ui(false, |ui| {
            let mut hotkey = global_hotkey::hotkey::HotKey::new(
                Some(self.hotkey_modifiers),
                self.hotkey_keycode,
            )
            .to_string().to_uppercase();

            ui.horizontal(|ui| {
                ui.label("OCR Hotkey: ");
                ui.text_edit_singleline(&mut hotkey);
            });
        }).response.on_disabled_hover_text(format!("Listening for a new hotkey is not currently supported. You can set your hotkey manually by editing {}", match std::env::consts::OS {
            "linux" => "`~/.config/deckocr/config.json`.",
            "windows" => "`%APPDATA%/deckocr/config.json`.",
            _ => "`deckocr/config.json` in your config directory."
        }));

        ui.add_space(spacing);

        egui::ComboBox::from_label("OCR Service")
            .selected_text(self.ocr_service.name())
            .show_ui(ui, |ui| {
                for service in OcrServiceList::ALL {
                    ui.selectable_value(&mut self.ocr_service, *service, service.name());
                }
            });

        egui::ComboBox::from_label("Dictionary Service")
            .selected_text(self.dictionary_service.name())
            .show_ui(ui, |ui| {
                for service in DictionaryServiceList::ALL {
                    ui.selectable_value(&mut self.dictionary_service, *service, service.name());
                }
            });

        egui::ComboBox::from_label("SRS Service")
            .selected_text(self.srs_service.name())
            .show_ui(ui, |ui| {
                for service in SrsServiceList::ALL {
                    ui.selectable_value(&mut self.srs_service, *service, service.name());
                }
            });

        ui.add_space(spacing);

        ui.horizontal(|ui| {
            ui.label("UI Scale:");
            egui::ComboBox::from_id_salt("UI Scale ComboBox")
                .selected_text(format!("{}%", (self.zoom_factor * 100.0) as i32))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.zoom_factor, 0.5, "50%");
                    ui.selectable_value(&mut self.zoom_factor, 0.75, "75%");
                    ui.selectable_value(&mut self.zoom_factor, 1.0, "100%");
                    ui.selectable_value(&mut self.zoom_factor, 1.5, "150%");
                    ui.selectable_value(&mut self.zoom_factor, 2.0, "200%");
                });
        });

        ui.horizontal(|ui| {
            ui.label("Fullscreen:");
            ui.add(egui::Checkbox::without_text(&mut self.fullscreen));
        });

        ui.horizontal(|ui| {
            ui.label("Window Size:");
            ui.add(
                egui::DragValue::new(&mut self.window_width)
                    .range(640..=3840)
                    .speed(1),
            );
            ui.label("×");
            ui.add(
                egui::DragValue::new(&mut self.window_height)
                    .range(480..=2160)
                    .speed(1),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Background Dimming:");
            ui.add(
                egui::DragValue::new(&mut self.background_dimming)
                    .custom_formatter(|n, _| format!("{}%", (n / 255.0 * 100.0) as i32))
                    .custom_parser(|s| {
                        s.trim_end_matches('%')
                            .parse()
                            .ok()
                            .map(|n: f64| n * 255.0 / 100.0)
                    }),
            );
        });
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum OcrServiceList {
    Owocr,
}

impl OcrServiceList {
    pub const ALL: &'static [Self] = &[Self::Owocr];

    pub fn name(&self) -> &str {
        match self {
            Self::Owocr => "owocr",
        }
    }

    pub fn create_service(&self) -> Box<dyn OcrService> {
        match self {
            Self::Owocr => Box::new(Owocr::default()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum DictionaryServiceList {
    Jpdb,
}

impl DictionaryServiceList {
    pub const ALL: &'static [Self] = &[Self::Jpdb];

    pub fn name(&self) -> &str {
        match self {
            Self::Jpdb => "jpdb",
        }
    }

    pub fn create_service(&self) -> Box<dyn DictionaryService> {
        match self {
            Self::Jpdb => Box::new(JpdbDictionary::default()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum SrsServiceList {
    Jpdb,
}

impl SrsServiceList {
    pub const ALL: &'static [Self] = &[Self::Jpdb];

    pub fn name(&self) -> &str {
        match self {
            Self::Jpdb => "jpdb",
        }
    }

    pub fn create_service(&self) -> Box<dyn SrsService> {
        match self {
            Self::Jpdb => Box::new(JpdbSrs::default()),
        }
    }
}
