use super::{config, flutewriter};
use crate::alc;
use pyo3::{exceptions::PyTypeError, prelude::*};
use std::time::SystemTime;

#[pyclass(unsendable)]
#[derive(Debug)]
pub struct Receiver(alc::receiver::Receiver);

#[pymethods]
impl Receiver {
    #[new]
    fn new(tsi: u64, writer: &flutewriter::FluteWriter, config: &config::Config) -> Self {
        let c = config.0.clone();
        Self {
            0: alc::receiver::Receiver::new(tsi, writer.inner.clone(), Some(c)),
        }
    }

    fn push(&mut self, data: &[u8]) -> PyResult<()> {
        self.0
            .push_data(data, SystemTime::now())
            .map_err(|e| PyTypeError::new_err(e.0.to_string()))
    }
}
