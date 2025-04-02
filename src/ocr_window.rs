use eframe::egui::{self, Color32, Pos2, Rect, TextureHandle, TextureOptions, ViewportInfo};
use image::{EncodableLayout, RgbaImage};

pub struct OcrWindow {
    texture: TextureHandle,
}

impl OcrWindow {
    pub fn new(ctx: &egui::Context, image: RgbaImage) -> Self {
        // https://docs.rs/egui/0.31.1/egui/struct.ColorImage.html#method.from_rgba_unmultiplied
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_flat_samples().as_slice(),
        );

        let texture = ctx.load_texture(
            "ocr window background",
            image,
            TextureOptions {
                magnification: egui::TextureFilter::Linear,
                minification: egui::TextureFilter::Linear,
                wrap_mode: egui::TextureWrapMode::ClampToEdge,
                mipmap_mode: None,
            },
        );

        Self { texture }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> ViewportInfo {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.painter().image(
                self.texture.id(),
                Rect::from_min_size(Pos2::ZERO, ui.available_size()),
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            )
        });

        ctx.input(|input| input.viewport().clone())
    }
}
