use eframe::egui::{self, vec2};

/// A stack of popups which should be shown to the user (eg. for error messages).
#[derive(Debug, Default)]
pub struct Popups(Vec<Popup>);

#[derive(Debug)]
struct Popup {
    message: String,
    first_frame: bool,
}

impl Popups {
    /// Show a new error message to the user.
    pub fn error(&mut self, e: anyhow::Error) {
        let mut s = format!("Error: {e}\n");

        for (idx, error) in e.chain().enumerate().skip(1) {
            s.push_str(&format!("\t{}. {}\n", idx, error));
        }

        self.0.push(Popup {
            message: s,
            first_frame: true,
        });
    }

    /// Show all currently held popups.
    pub fn show(&mut self, ctx: &egui::Context) {
        let mut close_popup = None;

        for (idx, popup) in self.0.iter_mut().enumerate() {
            ctx.show_viewport_immediate(
                egui::ViewportId(egui::Id::new(&popup.message)),
                egui::ViewportBuilder {
                    inner_size: Some(vec2(640.0, 480.0)),
                    ..Default::default()
                },
                |ctx, _| {
                    if popup.first_frame {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        popup.first_frame = false
                    }

                    egui::CentralPanel::default().show(ctx, |ui| {
                        egui_extras::StripBuilder::new(ui)
                            .size(egui_extras::Size::remainder())
                            .size(egui_extras::Size::exact(22.0))
                            .vertical(|mut strip| {
                                strip.cell(|ui| {
                                    egui::ScrollArea::vertical().auto_shrink(false).show(
                                        ui,
                                        |ui| {
                                            ui.label(&popup.message);
                                        },
                                    );
                                });

                                strip.cell(|ui| {
                                    ui.centered_and_justified(|ui| {
                                        if ui.button("Close").clicked() {
                                            close_popup = Some(idx);
                                        }
                                    });
                                });
                            });
                    });

                    if ctx.input(|input| input.viewport().close_requested()) {}
                    close_popup = Some(idx);
                },
            );
        }

        if let Some(idx) = close_popup {
            self.0.remove(idx);
        }
    }
}
