use std::{
    any::Any,
    marker::PhantomData,
    sync::{mpsc, Arc, Mutex},
    thread::JoinHandle,
};

use anyhow::{anyhow, Context, Result};
use config::{AppConfig, Config};
use dictionary_service::DictionaryService;
use eframe::{
    egui::{self, vec2, Color32, TextureHandle},
    epaint::text::{FontInsert, InsertFontFamily},
    CreationContext,
};
use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager};
use ocr_service::OcrService;
use ocr_window::{fill_texture, OcrWindow};

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
    services: ServiceManager,

    ocr_window_loading: Option<(TextureHandle, ServiceJob<Result<OcrWindow>>)>,
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

        let services = ServiceManager::new(&config).context("Failed to initialise services")?;

        Ok(Self {
            config,
            ocr_hotkey,
            services,

            ocr_window_loading: None,
            ocr_window: None,

            errors: Default::default(),
        })
    }

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

        let job = {
            let config = self.config.clone();
            let texture = texture.clone();
            self.services.exec(move |services| -> Result<OcrWindow> {
                let text_recs = services.ocr.ocr(image)?;
                let words = services.dictionary.parse_text_rects(&text_recs)?;
                let window = OcrWindow::new(config, texture, words)?;
                Ok(window)
            })
        };

        self.ocr_window_loading = Some((texture, job));

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

        if self.ocr_window_loading.is_some() || self.ocr_window.is_some() {
            let mut has_finished_loading = true;

            let mut size = vec2(0.0, 0.0);

            if let Some((texture, job)) = &mut self.ocr_window_loading {
                match job.try_finish().transpose() {
                    None => has_finished_loading = false,
                    Some(Err(e)) => self.errors.push(e),
                    Some(Ok(result)) => match *result {
                        Ok(ocr_window) => self.ocr_window = Some(ocr_window),
                        Err(e) => self.errors.push(e),
                    },
                }

                size = texture.size_vec2();
            }

            if has_finished_loading {
                self.ocr_window_loading = None;
            }

            if let Some(ocr_window) = &self.ocr_window {
                size = ocr_window.texture.size_vec2();
            }

            ctx.show_viewport_immediate(
                egui::ViewportId(egui::Id::new("ocr_viewport")),
                egui::ViewportBuilder {
                    inner_size: Some(size),
                    fullscreen: Some(self.config.fullscreen),
                    ..Default::default()
                },
                |ctx, _| {
                    if let Some((texture, _)) = &self.ocr_window_loading {
                        egui::CentralPanel::default().show(ctx, |ui| {
                            fill_texture(ctx, ui, texture);
                            ui.centered_and_justified(|ui| {
                                // FIXME: doesn't animate for some reason
                                ui.add(
                                    egui::Spinner::new()
                                        .color(Color32::from_white_alpha(96))
                                        .size(48.0),
                                );
                            });
                        });
                    }

                    if let Some(ocr_window) = &mut self.ocr_window {
                        ocr_window.show(ctx, &mut self.errors, &mut self.services);

                        if ocr_window.should_close
                            || ctx.input(|input| input.viewport().close_requested())
                        {
                            ocr_window_close_requested = true;
                        }
                    }
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

                            let ocr_name = self.services.ocr(|ocr| ocr.name());
                            let dictionary_name =
                                self.services.dictionary(|dictionary| dictionary.name());

                            egui::CollapsingHeader::new(
                                egui::RichText::new(format!("OCR: {ocr_name}")).size(header_size),
                            )
                            .default_open(true)
                            .show_unindented(ui, |ui| {
                                self.services.ocr(|ocr| ocr.config_gui(ui));
                            });

                            ui.separator();

                            egui::CollapsingHeader::new(
                                egui::RichText::new(format!("Dictionary: {dictionary_name}",))
                                    .size(header_size),
                            )
                            .default_open(true)
                            .show_unindented(ui, |ui| {
                                self.services
                                    .dictionary(|dictionary| dictionary.config_gui(ui));
                            });
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

pub struct ServiceManager {
    _thread: JoinHandle<()>,
    services: Arc<Mutex<Services>>,
    tx: mpsc::Sender<Box<dyn FnOnce(&mut Services) + Send>>,
}

pub struct Services {
    pub ocr: Box<dyn OcrService + Send>,
    pub dictionary: Box<dyn DictionaryService + Send>,
}

impl ServiceManager {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let mut services = Services {
            ocr: config.ocr_service.into(),
            dictionary: config.dictionary_service.into(),
        };

        services.ocr.init()?;
        services.dictionary.init()?;

        let services = Arc::new(Mutex::new(services));

        let (tx, rx) = mpsc::channel();

        let thread = {
            let services = Arc::clone(&services);
            std::thread::spawn(move || loop {
                let Ok(f): std::result::Result<
                    Box<dyn FnOnce(&mut Services) + Send>,
                    mpsc::RecvError,
                > = rx.recv() else {
                    drop(services);
                    break;
                };

                let mut lock = services.lock().unwrap();
                f(&mut lock);
                drop(lock);
            })
        };

        Ok(ServiceManager {
            _thread: thread,
            services,
            tx,
        })
    }

    pub fn exec<R: Send + 'static>(
        &mut self,
        f: impl FnOnce(&mut Services) -> R + Send + 'static,
    ) -> ServiceJob<R> {
        let (tx, rx) = mpsc::channel();

        let wrapped = move |inner: &mut Services| {
            let r = f(inner);
            let boxed: Box<dyn Any + Send> = Box::new(r);
            tx.send(boxed).unwrap();
        };

        self.tx.send(Box::new(wrapped)).unwrap();

        ServiceJob {
            _t: PhantomData,
            rx,
        }
    }

    pub fn ocr<R>(&mut self, f: impl FnOnce(&mut Box<dyn OcrService + Send>) -> R) -> R {
        let mut lock = self.services.lock().unwrap();
        let r = f(&mut lock.ocr);
        drop(lock);
        r
    }

    pub fn dictionary<R>(
        &mut self,
        f: impl FnOnce(&mut Box<dyn DictionaryService + Send>) -> R,
    ) -> R {
        let mut lock = self.services.lock().unwrap();
        let r = f(&mut lock.dictionary);
        drop(lock);
        r
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        if let Err(e) = self.ocr.terminate() {
            log::error!(
                "Error while terminating OCR service {}: `{e}`",
                self.ocr.name()
            );
        }
        if let Err(e) = self.dictionary.terminate() {
            log::error!(
                "Error while terminating Dictionary service {}: `{e}`",
                self.dictionary.name()
            );
        }
    }
}

pub struct ServiceJob<T> {
    _t: PhantomData<T>,
    rx: mpsc::Receiver<Box<dyn Any + Send>>,
}

impl<T: 'static> ServiceJob<T> {
    pub fn try_finish(&mut self) -> Result<Option<Box<T>>> {
        match self.rx.try_recv() {
            Ok(t) => Ok(Some(t.downcast().unwrap())),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                Err(anyhow!("ServiceJob channel was disconnected"))
            }
        }
    }

    pub fn finish(&mut self) -> Result<Box<T>> {
        self.rx
            .recv()
            .map(|t| t.downcast().unwrap())
            .map_err(|_| anyhow!("ServiceJob channel was disconnected"))
    }
}
