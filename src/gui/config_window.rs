use eframe::egui;

use crate::{config::Config, services::Services, EframeApp};

pub fn show_config_window(app: &mut EframeApp, ctx: &egui::Context) {
    egui::CentralPanel::default().show(ctx, |ui| {
        egui_extras::StripBuilder::new(ui)
            .size(egui_extras::Size::remainder())
            .size(egui_extras::Size::exact(0.0))
            .size(egui_extras::Size::exact(22.0))
            .vertical(|mut strip| {
                strip.cell(|ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        let header_size = 24.0;

                        ui.label(
                            egui::RichText::new(concat!(env!("CARGO_PKG_NAME"), " Configuration"))
                                .size(header_size)
                                .strong(),
                        );

                        app.config.show_ui(ui);

                        ui.separator();

                        egui::CollapsingHeader::new(
                            egui::RichText::new(format!("OCR: {}", app.config.ocr_service.name()))
                                .size(header_size),
                        )
                        .default_open(true)
                        .show_unindented(ui, |ui| {
                            app.services.ocr.show_config_ui(ui);
                        });

                        ui.separator();

                        egui::CollapsingHeader::new(
                            egui::RichText::new(format!(
                                "Dictionary: {}",
                                app.config.dictionary_service.name()
                            ))
                            .size(header_size),
                        )
                        .default_open(true)
                        .show_unindented(ui, |ui| {
                            app.services.dictionary.show_config_ui(ui);
                        });

                        ui.separator();

                        egui::CollapsingHeader::new(
                            egui::RichText::new(format!("SRS: {}", app.config.srs_service.name()))
                                .size(header_size),
                        )
                        .default_open(true)
                        .show_unindented(ui, |ui| {
                            app.services.srs.show_config_ui(ui);
                        });
                    });
                });

                strip.empty();

                strip.cell(|ui| {
                    ui.centered_and_justified(|ui| {
                        if ui.button("Reload Services").clicked() {
                            match Services::new(&app.config) {
                                Ok(services) => app.services = services,
                                Err(e) => app.popups.error(e),
                            }
                        }
                    });
                });
            });
    });
}
