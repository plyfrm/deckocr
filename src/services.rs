use std::thread::JoinHandle;

use anyhow::{anyhow, Result};
use dictionary::DictionaryService;
use ocr::OcrService;
use srs::SrsService;

use crate::config::AppConfig;

pub mod dictionary;
pub mod ocr;
pub mod srs;

/// Holds instanciated services.
pub struct Services {
    pub ocr: Box<dyn OcrService>,
    pub dictionary: Box<dyn DictionaryService>,
    pub srs: Box<dyn SrsService>,
}

impl Services {
    /// Create a new `Services` from the services specified in the given `AppConfig`.
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

pub struct ServiceJob<T> {
    handle: Option<JoinHandle<T>>,
}

/// A job being performed by a service. May or may not be finished.
impl<T: Send + 'static> ServiceJob<T> {
    pub fn new<F: FnOnce() -> T + Send + 'static>(f: F) -> Self {
        std::thread::spawn(f).into()
    }
}

impl<T> ServiceJob<T> {
    /// Get the return value of this `ServiceJob` if it was finished.
    ///
    /// - Returns `Err` if the job has already finished and its return value was taken previously;
    /// - Returns `Ok(None) if the job has not finished yet;
    /// - Returns `Ok(Some(T))` if the job has finished.
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

    /// Wait for the job to finish and return its return value.
    ///
    /// - Returns `Err` if the job has already finished (eg. by calling `try_wait()`) and its return value was taken previously;
    /// - Returns `Ok(T) if the job has finished.
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
