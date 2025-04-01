use std::sync::mpsc;

use anyhow::{anyhow, Context, Result};
use config::{load_config, Config};
use dictionary_service::DictionaryService;
use eframe::CreationContext;
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager};
use ocr_service::OcrService;

pub mod config;
pub mod dictionary_service;
pub mod ocr_service;

fn main() -> Result<()> {
    // TODO: graceful error handling in main
    eframe::run_native(
        "app_name",
        Default::default(),
        Box::new(|cc| {
            EframeApp::new(cc)
                .map(|app| -> Box<dyn eframe::App> { Box::new(app) })
                .map_err(|e| panic!("{e:?}"))
        }),
    )
    .map_err(|e| anyhow!("{e}"))
}

// - start eframe app
// - read config (services + hotkey)
// - register global hotkey
// - initialise both ocr and dictionary services
// - should be good?

struct EframeApp {
    config: Config,
    ocr_hotkey: HotKey,
    ocr_service: Box<dyn OcrService>,
    dictionary_service: Box<dyn DictionaryService>,
}

impl EframeApp {
    pub fn new(cc: &CreationContext) -> Result<Self> {
        let config: Config =
            load_config("config.json").context("Could not load main configuration file")?;

        // NOTE: this isn't documented, but GlobalHotKeyManager needs to stay alive for the entire duration of the program.
        let hotkey_manager = Box::leak(Box::new(
            GlobalHotKeyManager::new().context("Failed to initialise GlobalHotKeyManager")?,
        ));
        let ocr_hotkey = HotKey::new(Some(config.hotkey_modifiers), config.hotkey_keycode);
        hotkey_manager
            .register(ocr_hotkey)
            .context("Failed to register hotkey with GlobalHotKeyManager")?;

        let mut ocr_service: Box<dyn OcrService> = config.ocr_service.into();
        ocr_service.init().with_context(|| {
            format!("Failed to initialise OCR Service `{}`", ocr_service.name())
        })?;

        let mut dictionary_service: Box<dyn DictionaryService> = config.dictionary_service.into();
        dictionary_service.init().with_context(|| {
            format!(
                "Failed to initialise Dictionary Service: `{}`",
                dictionary_service.name()
            )
        })?;

        Ok(Self {
            config,
            ocr_hotkey,
            ocr_service,
            dictionary_service,
        })
    }
}

impl eframe::App for EframeApp {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.ocr_hotkey.id && event.state == global_hotkey::HotKeyState::Pressed
            {
                println!("OCR hotkey was pressed!");
            }
        }
    }
}
