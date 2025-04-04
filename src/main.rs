use anyhow::{anyhow, Context, Result};
use config::{AppConfig, Config};
use dictionary_service::DictionaryService;
use eframe::{
    egui::{self, vec2},
    epaint::text::{FontInsert, InsertFontFamily},
    CreationContext,
};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager};
use ocr_service::OcrService;
use ocr_window::OcrWindow;

pub mod config;
pub mod dictionary_service;
pub mod ocr_service;
pub mod ocr_window;

fn main() -> Result<()> {
    pretty_env_logger::init();

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

struct EframeApp {
    config: AppConfig,
    ocr_hotkey: HotKey,
    ocr_service: Box<dyn OcrService>,
    dictionary_service: Box<dyn DictionaryService>,
    ocr_window: Option<OcrWindow>,

    errors: Errors,
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

        // cc.egui_ctx.set_theme(egui::Theme::Dark);

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

            errors: Default::default(),
        })
    }

    // TODO: show window immediately with a spinner while things are loading
    pub fn trigger_ocr(&mut self, ctx: &egui::Context) -> Result<()> {
        let monitor = xcap::Monitor::all()?
            .into_iter()
            .find(|monitor| monitor.is_primary().unwrap_or(false))
            .ok_or_else(|| anyhow!("No primary monitor found."))?;

        let image = monitor.capture_image()?;

        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_flat_samples().as_slice(),
        );

        let texture = ctx.load_texture(
            "ocr window background",
            color_image,
            egui::TextureOptions {
                magnification: egui::TextureFilter::Linear,
                minification: egui::TextureFilter::Linear,
                wrap_mode: egui::TextureWrapMode::ClampToEdge,
                mipmap_mode: None,
            },
        );

        let text_recs = self.ocr_service.ocr(image)?;
        let words = self.dictionary_service.parse_text_rects(&text_recs)?;

        self.ocr_window = Some(OcrWindow::new(self.config.clone(), texture, words)?);

        Ok(())
    }
}

impl eframe::App for EframeApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Err(e) = self.config.save() {
            log::error!("Error while saving configuration file: `{e}`");
        }

        if let Err(e) = self.ocr_service.terminate() {
            log::error!("Error while terminating OCR service: `{e}`");
        }

        if let Err(e) = self.dictionary_service.terminate() {
            log::error!("Error while terminating Dictionary service: `{e}`");
        }
    }

    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        // continuous repaint mode
        // TODO: get rid of this
        ctx.request_repaint();

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

        let mut ocr_window_close_requested = false;

        if let Some(ocr_window) = &mut self.ocr_window {
            ctx.show_viewport_immediate(
                egui::ViewportId(egui::Id::new("ocr_viewport")),
                egui::ViewportBuilder {
                    inner_size: Some(ocr_window.texture.size_vec2()),
                    fullscreen: Some(self.config.fullscreen),
                    ..Default::default()
                },
                |ctx, _| {
                    let viewport_info = ocr_window.show(
                        ctx,
                        &mut self.errors,
                        &mut self.dictionary_service,
                        self.ocr_service.supports_text_rects(),
                    );
                    ocr_window_close_requested =
                        viewport_info.close_requested() || ocr_window.should_close;
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
