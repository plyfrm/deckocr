use std::io::Cursor;

use anyhow::{Context, Result};
use eframe::egui;
use image::{ImageFormat, RgbaImage};
use serde::{Deserialize, Serialize};

use crate::{config::Config, services::ServiceJob};

use super::{OcrResponse, OcrService};

#[derive(Default)]
pub struct Owocr {
    config: OwocrConfig,
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

    fn show_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Make sure you start owocr separately!");
        ui.horizontal(|ui| {
            ui.label("Address:");
            ui.text_edit_singleline(&mut self.address);
        });
        ui.horizontal(|ui| {
            ui.label("Port:");
            ui.add(egui::DragValue::new(&mut self.port));
        });
    }
}

impl OcrService for Owocr {
    fn init(&mut self) -> anyhow::Result<()> {
        self.config = OwocrConfig::load().context("Owocr: Failed to load configuration file")?;
        Ok(())
    }

    fn terminate(&mut self) -> anyhow::Result<()> {
        self.config
            .save()
            .context("Owocr: Failed to save configuration file")?;
        Ok(())
    }

    fn show_config_ui(&mut self, ui: &mut egui::Ui) {
        self.config.show_ui(ui);
    }

    fn ocr(&mut self, image: RgbaImage) -> ServiceJob<Result<OcrResponse>> {
        let addr = format!("ws://{}:{}", self.config.address, self.config.port);

        ServiceJob::new(move || {
            let mut buf = Cursor::new(Vec::new());
            image.write_to(&mut buf, ImageFormat::Png).unwrap();

            let (mut socket, _) = tungstenite::connect(&addr)
                .with_context(|| format!("Owocr: Failed to connect to websocket `{addr}`"))?;

            socket
                .send(tungstenite::Message::binary(buf.into_inner()))
                .context("Owocr: Failed to send image through websocket")?;
            // NOTE: owocr sends a text message containing just "True" the socket is first connected to. we need to consume it
            socket
                .read()
                .context("Owocr: Failed to read confirmation message from websocket")?;
            let text = socket
                .read()
                .context("Owocr: Failed to read response message from websocket")?
                .into_text()
                .context(
                    "Owocr: Response message from websocket did not contain UTF-8 encoded text",
                )?;

            socket
                .close(None)
                .context("Owocr: Failed to close websocket")?;

            let text = text.split('\u{3000}').map(str::to_owned).collect();

            Ok(OcrResponse::WithoutRects(text))
        })
    }
}
