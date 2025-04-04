use anyhow::Result;
use eframe::egui::{self, Rect};
use image::RgbaImage;

pub mod owocr;

pub trait OcrService {
    fn name(&self) -> &'static str;
    fn supports_text_rects(&self) -> bool;

    fn init(&mut self) -> Result<()>;
    fn terminate(&mut self) -> Result<()>;
    fn config_gui(&mut self, ui: &mut egui::Ui);

    fn ocr(&mut self, image: RgbaImage) -> Result<Vec<(Rect, String)>>;
}
