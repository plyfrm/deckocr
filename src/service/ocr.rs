use anyhow::Result;
use eframe::egui::Rect;
use image::RgbaImage;

use super::{Service, ServiceJob};

pub mod owocr;

pub type OcrInput = RgbaImage;
pub type OcrOutput = Result<OcrResponse>;

pub enum OcrResponse {
    WithRects(Vec<(Rect, String)>),
    WithoutRects(Vec<String>),
}

pub trait OcrService: Service<OcrInput, OcrOutput> {
    fn ocr(&mut self, input: OcrInput) -> ServiceJob<OcrOutput> {
        self.call(input)
    }
}

impl<T> OcrService for T where T: Service<OcrInput, OcrOutput> {}
