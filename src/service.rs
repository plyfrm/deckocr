use std::thread::JoinHandle;

use anyhow::{anyhow, Result};
use dictionary::DictionaryService;
use eframe::egui;
use ocr::OcrService;
use srs::SrsService;

use crate::config::AppConfig;

pub mod dictionary;
pub mod ocr;
pub mod srs;

pub struct Services {
    pub ocr: Box<dyn OcrService>,
    pub dictionary: Box<dyn DictionaryService>,
    pub srs: Box<dyn SrsService>,
}

impl Services {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let mut services = Self {
            ocr: config.ocr_service.create_service(),
            dictionary: config.dictionary_service.create_service(),
            srs: config.srs_service.create_service(),
        };

        services.ocr.init()?;
        services.dictionary.init()?;
        services.srs.init()?;

        Ok(services)
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        self.ocr
            .terminate()
            .expect("Failed to terminate OCR Service");
        self.dictionary
            .terminate()
            .expect("Failed to terminate dictionary Service");
        self.srs
            .terminate()
            .expect("Failed to terminate SRS Service");
    }
}

pub trait Service<I, O> {
    fn init(&mut self) -> Result<()>;
    fn terminate(&mut self) -> Result<()>;

    fn show_config_ui(&mut self, ui: &mut egui::Ui);

    fn call(&mut self, input: I) -> ServiceJob<O>;
}

pub struct ServiceJob<T> {
    handle: Option<JoinHandle<T>>,
}

impl<T: Send + 'static> ServiceJob<T> {
    pub fn new<F: FnOnce() -> T + Send + 'static>(f: F) -> Self {
        std::thread::spawn(f).into()
    }
}

impl<T> ServiceJob<T> {
    pub fn try_wait(&mut self) -> Result<Option<T>> {
        match &self.handle {
            None => Err(anyhow!("job already finished")),
            Some(handle) if handle.is_finished() => {
                Ok(Some(self.handle.take().unwrap().join().unwrap()))
            }
            Some(handle) if !handle.is_finished() => Ok(None),
            _ => unreachable!(),
        }
    }

    pub fn wait(self) -> Result<T> {
        match self.handle {
            None => Err(anyhow!("job already finished")),
            Some(handle) => Ok(handle.join().unwrap()),
        }
    }
}

impl<T> Into<ServiceJob<T>> for JoinHandle<T> {
    fn into(self) -> ServiceJob<T> {
        ServiceJob { handle: Some(self) }
    }
}
