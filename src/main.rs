use std::default;

use anyhow::{anyhow, Context, Result};
use config::{AppConfig, Config};
use dictionary_service::DictionaryService;
use eframe::{egui, CreationContext};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager};
use ocr_service::OcrService;
use ocr_window::OcrWindow;

pub mod config;
pub mod dictionary_service;
pub mod ocr_service;
pub mod ocr_window;

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
    config: AppConfig,
    ocr_hotkey: HotKey,
    ocr_service: Box<dyn OcrService>,
    dictionary_service: Box<dyn DictionaryService>,
    ocr_window: Option<OcrWindow>,
}

impl EframeApp {
    pub fn new(cc: &CreationContext) -> Result<Self> {
        let config = AppConfig::load().context("Could not load main configuration file")?;

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
            ocr_window: None,
        })
    }

    pub fn show_error(&mut self, error: anyhow::Error) {
        panic!("{error:?}"); // TODO: show error to the user properly
    }

    pub fn trigger_ocr(&mut self, ctx: &egui::Context) -> Result<()> {
        let monitor = xcap::Monitor::all()?
            .into_iter()
            .find(|monitor| monitor.is_primary().unwrap_or(false))
            .ok_or_else(|| anyhow!("No primary monitor found."))?;

        let image = monitor.capture_image()?;

        self.ocr_window = Some(OcrWindow::new(ctx, image));

        Ok(())
    }
}

impl eframe::App for EframeApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.ocr_hotkey.id && event.state == global_hotkey::HotKeyState::Pressed
            {
                if let Err(e) = self.trigger_ocr(ctx) {
                    self.show_error(e);
                }
            }
        }

        let mut ocr_window_close_requested = false;

        if let Some(ocr_window) = &mut self.ocr_window {
            ctx.show_viewport_immediate(
                egui::ViewportId(egui::Id::new("ocr_viewport")),
                egui::ViewportBuilder {
                    ..Default::default()
                },
                |ctx, _| {
                    let viewport_info = ocr_window.show(ctx);
                    ocr_window_close_requested = viewport_info.close_requested();
                },
            );
        }

        if ocr_window_close_requested {
            self.ocr_window = None;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui_extras::StripBuilder::new(ui)
                .size(egui_extras::Size::remainder())
                .size(egui_extras::Size::exact(32.0))
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            let header_size = 24.0;

                            ui.label(
                                egui::RichText::new(concat!(
                                    env!("CARGO_PKG_NAME"),
                                    " Configuration"
                                ))
                                .size(header_size)
                                .strong(),
                            );

                            self.config.gui(ui);

                            ui.separator();

                            egui::CollapsingHeader::new(
                                egui::RichText::new(format!("OCR: {}", self.ocr_service.name()))
                                    .size(header_size),
                            )
                            .default_open(true)
                            .show_unindented(ui, |ui| self.ocr_service.config_gui(ui));

                            ui.separator();

                            egui::CollapsingHeader::new(
                                egui::RichText::new(format!(
                                    "Dictionary: {}",
                                    self.dictionary_service.name()
                                ))
                                .size(header_size),
                            )
                            .default_open(true)
                            .show_unindented(ui, |ui| self.dictionary_service.config_gui(ui));
                        });
                    });

                    strip.cell(|ui| {
                        ui.centered_and_justified(|ui| {
                            ui.add(
                                egui::Button::new("Apply Changes").min_size(ui.available_size()),
                            );
                        });
                        // TODO: restart the app when clicked
                    });
                });
        });
    }
}
