use super::{config, flutewriter};
use crate::alc;
use pyo3::{exceptions::PyTypeError, prelude::*};
use std::time::SystemTime;

#[pyclass(unsendable)]
#[derive(Debug)]
pub struct MultiReceiver(alc::multireceiver::MultiReceiver);

#[pymethods]
impl MultiReceiver {
    #[new]
    fn new(
        tsi: Option<Vec<u64>>,
        writer: &flutewriter::FluteWriter,
        config: &config::Config,
    ) -> Self {
        let c = config.0.clone();
        Self {
            0: alc::multireceiver::MultiReceiver::new(
                tsi.as_ref().map(|v| v.as_ref()),
                writer.inner.clone(),
                Some(c),
            ),
        }
    }

    fn push(&mut self, data: &[u8]) -> PyResult<()> {
        self.0
            .push(data, SystemTime::now())
            .map_err(|e| PyTypeError::new_err(e.0.to_string()))
    }
}
