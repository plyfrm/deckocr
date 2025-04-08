use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use config::{AppConfig, Config};
use eframe::{
    egui::{self, vec2},
    epaint::text::{FontInsert, InsertFontFamily},
    CreationContext,
};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager};
use ocr_window::OcrWindow;
use service::Services;

pub mod config;
pub mod ocr_window;
pub mod service;

const WINDOW_TITLE: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"));

fn main() -> Result<()> {
    pretty_env_logger::init();

    // TODO: graceful error handling in main
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
                inner_size: Some(vec2(400.0, 600.0)),
                min_inner_size: Some(vec2(400.0, 200.0)),
                max_inner_size: Some(vec2(400.0, 800.0)),
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

struct EframeApp {
    config: AppConfig,
    ocr_hotkey: HotKey,
    services: Services,

    ocr_window: Option<OcrWindow>,

    errors: Errors,
    last_repaint: std::time::Instant,
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

            errors: Default::default(),
            last_repaint: Instant::now(),
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

        let image = monitor.capture_image()?;

        self.ocr_window = Some(OcrWindow::new(
            ctx,
            self.config.clone(),
            image,
            &mut self.services,
        )?);

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
        // ideally we should only repaint when needed instead of limiting the framerate, but the hotkey can
        // only be checked for on the same thread the event loop is running on so that limits our options.
        std::thread::sleep(Duration::from_millis(16).saturating_sub(self.last_repaint.elapsed()));
        ctx.request_repaint();
        self.last_repaint = Instant::now();

        ctx.set_zoom_factor(self.config.zoom_factor);

        self.errors.show(ctx);

        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == self.ocr_hotkey.id && event.state == global_hotkey::HotKeyState::Pressed
            {
                if let Err(e) = self.trigger_ocr(ctx) {
                    self.errors.push(e);
                }
            }
        }

        if let Some(ocr_window) = &mut self.ocr_window {
            ocr_window.show(ctx, &self.config, &mut self.errors, &mut self.services);

            if ocr_window.close_requested {
                self.ocr_window = None;
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            egui_extras::StripBuilder::new(ui)
                .size(egui_extras::Size::remainder())
                .size(egui_extras::Size::exact(0.0))
                .size(egui_extras::Size::exact(22.0))
                .vertical(|mut strip| {
                    strip.cell(|ui| {
                        egui::ScrollArea::vertical()
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                            )
                            .show(ui, |ui| {
                                let header_size = 24.0;

                                ui.label(
                                    egui::RichText::new(concat!(
                                        env!("CARGO_PKG_NAME"),
                                        " Configuration"
                                    ))
                                    .size(header_size)
                                    .strong(),
                                );

                                self.config.show_ui(ui);

                                ui.separator();

                                egui::CollapsingHeader::new(
                                    egui::RichText::new(format!(
                                        "OCR: {}",
                                        self.config.ocr_service.name()
                                    ))
                                    .size(header_size),
                                )
                                .default_open(true)
                                .show_unindented(ui, |ui| {
                                    self.services.ocr.show_config_ui(ui);
                                });

                                ui.separator();

                                egui::CollapsingHeader::new(
                                    egui::RichText::new(format!(
                                        "Dictionary: {}",
                                        self.config.dictionary_service.name()
                                    ))
                                    .size(header_size),
                                )
                                .default_open(true)
                                .show_unindented(ui, |ui| {
                                    self.services.dictionary.show_config_ui(ui);
                                });

                                ui.separator();

                                egui::CollapsingHeader::new(
                                    egui::RichText::new(format!(
                                        "SRS: {}",
                                        self.config.srs_service.name()
                                    ))
                                    .size(header_size),
                                )
                                .default_open(true)
                                .show_unindented(ui, |ui| {
                                    self.services.srs.show_config_ui(ui);
                                });
                            });
                    });

                    strip.empty();

                    strip.cell(|ui| {
                        ui.centered_and_justified(|ui| {
                            if ui.button("Reload Services").clicked() {
                                match Services::new(&self.config) {
                                    Ok(services) => self.services = services,
                                    Err(e) => self.errors.push(e),
                                }
                            }
                        });
                    });
                });
        });
    }
}

#[derive(Debug, Default)]
pub struct Errors(Vec<anyhow::Error>);

impl Errors {
    pub fn push(&mut self, e: anyhow::Error) {
        self.0.push(e);
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let mut remove = None;
        for (idx, error) in self.0.iter().enumerate() {
            let message = format!("{:?}", error);

            ctx.show_viewport_immediate(
                egui::ViewportId(egui::Id::new(&message)),
                egui::ViewportBuilder {
                    title: Some("An error has occured!".to_owned()),
                    inner_size: Some(vec2(300.0, 100.0)),
                    ..Default::default()
                },
                |ctx, _| {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

                    egui::CentralPanel::default().show(ctx, |ui| {
                        egui_extras::StripBuilder::new(ui)
                            .size(egui_extras::Size::remainder())
                            .size(egui_extras::Size::exact(20.0))
                            .vertical(|mut strip| {
                                strip.cell(|ui| {
                                    egui::ScrollArea::vertical().show(ui, |ui| {
                                        ui.vertical_centered_justified(|ui| {
                                            ui.label(&message);
                                        });
                                    });
                                });

                                strip.cell(|ui| {
                                    ui.vertical_centered(|ui| {
                                        if ui.button("Close").clicked()
                                            || ctx.input(|input| input.viewport().close_requested())
                                        {
                                            remove = Some(idx)
                                        }
                                    });
                                });
                            });
                    });
                },
            );
        }

        remove.map(|idx| self.0.swap_remove(idx));
    }
}
