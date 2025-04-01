use anyhow::{anyhow, Context, Result};
use config::{load_config, Config};
use dictionary_service::DictionaryService;
use eframe::CreationContext;
use global_hotkey::{hotkey::HotKey, GlobalHotKeyManager};
use ocr_service::OcrService;

pub mod config;
pub mod dictionary_service;
pub mod ocr_service;

fn main() -> Result<()> {
    eframe::run_native(
        "app_name",
        Default::default(),
        Box::new(|cc| {
            EframeApp::new(cc)
                .map(|app| -> Box<dyn eframe::App> { Box::new(app) })
                .map_err(|e| todo!())
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
    hotkey: HotKey,
    ocr: Box<dyn OcrService>,
    dictionary: Box<dyn DictionaryService>,
}

impl EframeApp {
    pub fn new(cc: &CreationContext) -> Result<Self> {
        let config: Config =
            load_config("config.json").context("Could not load main configuration file")?;

        let manager =
            GlobalHotKeyManager::new().context("Failed to initialise GlobalHotKeyManager")?;
        let hotkey = HotKey::new(Some(config.hotkey_modifiers), config.hotkey_keycode);
        manager
            .register(hotkey)
            .context("Failed to register hotkey with GlobalHotKeyManager")?;

        let mut ocr: Box<dyn OcrService> = config.ocr_service.into();
        ocr.init()
            .with_context(|| format!("Failed to initialise OCR Service `{}`", ocr.name()))?;

        let mut dictionary: Box<dyn DictionaryService> = config.dictionary_service.into();
        dictionary.init().with_context(|| {
            format!(
                "Failed to initialise Dictionary Service: `{}`",
                dictionary.name()
            )
        })?;

        Ok(Self {
            config,
            hotkey,
            ocr,
            dictionary,
        })
    }
}

impl eframe::App for EframeApp {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {}
}
