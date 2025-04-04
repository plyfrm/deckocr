use std::io::Cursor;

use crate::config::Config;

use super::OcrService;

use anyhow::Result;
use eframe::egui::{self, Rect};
use image::ImageFormat;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Owocr {
    config: OwocrConfig,
}

impl OcrService for Owocr {
    fn name(&self) -> &'static str {
        "owocr"
    }

    fn supports_text_rects(&self) -> bool {
        false
    }

    fn init(&mut self) -> Result<()> {
        self.config = OwocrConfig::load()?;
        Ok(())
    }

    fn terminate(&mut self) -> Result<()> {
        self.config.save()?;
        Ok(())
    }

    fn config_gui(&mut self, ui: &mut egui::Ui) {
        self.config.gui(ui);
    }

    fn ocr(&mut self, image: image::RgbaImage) -> Result<Vec<(Rect, String)>> {
        let mut buf = Cursor::new(Vec::new());
        image.write_to(&mut buf, ImageFormat::Png)?;

        let addr = format!("ws://{}:{}", self.config.address, self.config.port);

        let (mut socket, _) = tungstenite::connect(addr)?;

        socket.send(tungstenite::Message::binary(buf.into_inner()))?;
        // NOTE: owocr seems to send a utf8 message just containing "True" when the socket first gets established
        socket.read()?;
        let text = socket.read()?.into_text()?;

        socket.close(None)?;

        // TODO: filter out non japanese text, merge rects which contain too few characters
        let text = text
            .split('\u{3000}')
            .map(|s| (Rect::ZERO, s.to_owned()))
            .collect();

        Ok(text)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OwocrConfig {
    address: String,
    port: u16,
}

impl Default for OwocrConfig {
    fn default() -> Self {
        Self {
            address: "127.0.0.1".to_owned(),
            port: 7331,
        }
    }
}

impl Config for OwocrConfig {
    fn path() -> &'static str {
        "ocr_services/owocr.json"
    }

    fn gui(&mut self, ui: &mut egui::Ui) {
        ui.label("Make sure you start owocr separately!");
        ui.horizontal(|ui| {
            ui.label("Address: ");
            ui.text_edit_singleline(&mut self.address);
        });
        ui.add(egui::DragValue::new(&mut self.port).prefix("Port: "));
    }
}
