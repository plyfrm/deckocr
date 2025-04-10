use anyhow::Result;
use eframe::egui::{self, Rect};
use image::RgbaImage;

use super::ServiceJob;

pub mod owocr;

pub type OcrServiceJob = ServiceJob<Result<OcrResponse>>;

pub trait OcrService {
    fn init(&mut self) -> Result<()>;
    fn terminate(&mut self) -> Result<()>;

    fn show_config_ui(&mut self, ui: &mut egui::Ui);

    fn ocr(&mut self, image: RgbaImage) -> OcrServiceJob;
}

pub enum OcrResponse {
    WithRects(Vec<(Rect, String)>),
    WithoutRects(Vec<String>),
}
