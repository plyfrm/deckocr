use anyhow::Result;
use eframe::egui::{self, Rect};
use image::RgbaImage;

use super::ServiceJob;

pub mod owocr;

pub type OcrServiceJob = ServiceJob<Result<OcrResponse>>;

pub trait OcrService {
    /// Initialise the service (ie. load its configuration file, etc).
    fn init(&mut self) -> Result<()>;
    /// Terminate the service (ie. save its configuration file, etc).
    fn terminate(&mut self) -> Result<()>;

    /// Show the config UI for the service's configuration.
    fn show_config_ui(&mut self, ui: &mut egui::Ui);

    /// Extract text from an image, returning a list of paragraphs.
    fn ocr(&mut self, image: RgbaImage) -> OcrServiceJob;
}

/// The data returned by an OCR service.
pub enum OcrResponse {
    /// A list of paragraphs with their associated text bounds.
    ///
    /// NOTE: This is not currently supported.
    WithRects(Vec<(Rect, String)>),
    /// A simple list of paragraphs.
    WithoutRects(Vec<String>),
}
