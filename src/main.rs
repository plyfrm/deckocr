use std::{sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use config::{AppConfig, Config};
use eframe::{
    egui::{self, vec2},
    epaint::text::{FontInsert, InsertFontFamily},
    CreationContext,
};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager};
use gui::{config_window::show_config_window, ocr_window::OcrWindow, popups::Popups};
use services::Services;

pub mod config;
pub mod gui;
pub mod services;
pub mod word;

const WINDOW_TITLE: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));
const WINDOW_W: f32 = 400.0;
const WINDOW_H: f32 = 600.0;
const WINDOW_H_MIN: f32 = 300.0;
const WINDOW_H_MAX: f32 = 720.0;

fn main() -> Result<()> {
    pretty_env_logger::init();

    // TODO: nicely show any errors returned from main to the user somehow
    eframe::run_native(
        "app_name",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder {
                title: Some(WINDOW_TITLE.to_owned()),
                icon: Some({
                    let logo =
                        image::load_from_memory(include_bytes!("../assets/logo.png")).unwrap();
                    Arc::new(egui::IconData {
                        width: logo.width(),
                        height: logo.height(),
                        rgba: logo.into_rgba8().into_vec(),
                    })
                }),
                // TODO: update window size when UI scaling is changed
                inner_size: Some(vec2(WINDOW_W, WINDOW_H)),
                min_inner_size: Some(vec2(WINDOW_W, WINDOW_H_MIN)),
                max_inner_size: Some(vec2(WINDOW_W, WINDOW_H_MAX)),
                ..Default::default()
            },
            ..Default::default()
        },
        Box::new(|cc| {
            EframeApp::new(cc)
                .map(|app| -> Box<dyn eframe::App> { Box::new(app) })
                .map_err(|e| panic!("{e:?}"))
        }),
    )
    .map_err(|e| anyhow!("{e}"))
}

pub struct EframeApp {
    config: AppConfig,
    ocr_hotkey: HotKey,
    services: Services,

    ocr_window: Option<OcrWindow>,

    popups: Popups,
}

impl EframeApp {
    pub fn new(cc: &CreationContext) -> Result<Self> {
        egui_extras::install_image_loaders(&cc.egui_ctx);

        // FIXME: some characters aren't being rendered properly with this font
        cc.egui_ctx.add_font(FontInsert::new(
            "M+",
            egui::FontData::from_static(include_bytes!("../assets/fonts/MPLUS1-Regular.ttf")),
            vec![InsertFontFamily {
                family: egui::FontFamily::Proportional,
                priority: egui::epaint::text::FontPriority::Highest,
            }],
        ));

        let config = AppConfig::load().context("Could not load main configuration file")?;

        // NOTE: this isn't documented, but GlobalHotKeyManager needs to stay alive for the entire duration of the program.
        let hotkey_manager = Box::leak(Box::new(
            GlobalHotKeyManager::new().context("Failed to initialise GlobalHotKeyManager")?,
        ));
        let ocr_hotkey = HotKey::new(Some(config.hotkey_modifiers), config.hotkey_keycode);
        hotkey_manager
            .register(ocr_hotkey)
            .context("Failed to register hotkey with GlobalHotKeyManager")?;

        let services = Services::new(&config).context("Failed to initialise services")?;

        Ok(Self {
            config,
            ocr_hotkey,
            services,

            ocr_window: None,

            popups: Default::default(),
        })
    }

    pub fn trigger_ocr(&mut self, ctx: &egui::Context) -> Result<()> {
        let currently_loading = self
            .ocr_window
            .as_ref()
            .map(|window| window.is_loading())
            .unwrap_or(false);

        // only trigger ocr if we are not currently loading an ocr window (eliminates some jankiness with steam input)
        if currently_loading {
            return Ok(());
        }

        let monitor = xcap::Monitor::all()?
            .into_iter()
            .find(|monitor| monitor.is_primary().unwrap_or(false))
            .ok_or_else(|| anyhow!("No primary monitor found."))?;

        let image = monitor
            .capture_image()
            .context("Failed to capture primary monitor")?;

        self.ocr_window = Some(OcrWindow::new(
            ctx,
            self.config.clone(),
            image,
            &mut self.services,
        ));

        Ok(())
    }
}

impl eframe::App for EframeApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Err(e) = self.config.save() {
            log::error!("Error while saving configuration file: `{e}`");
        }
    }

    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(250));

        ctx.set_zoom_factor(self.config.zoom_factor);

        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.ocr_hotkey.id && event.state == global_hotkey::HotKeyState::Pressed
            {
                if let Err(e) = self.trigger_ocr(ctx) {
                    self.popups.error(e);
                }
            }
        }

        if let Some(ocr_window) = &mut self.ocr_window {
            ocr_window.show(ctx, &self.config, &mut self.popups, &mut self.services);

            if ocr_window.close_requested {
                self.ocr_window = None;
            }
        }

        show_config_window(self, ctx);

        self.popups.show(ctx);
    }
}
