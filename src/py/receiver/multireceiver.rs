use super::{config, objectwriterbuilder, udpendpoint};
use pyo3::{exceptions::PyTypeError, prelude::*};
use std::time::SystemTime;

#[pyclass(unsendable)]
#[derive(Debug)]
pub struct MultiReceiver(crate::receiver::MultiReceiver);

#[pymethods]
impl MultiReceiver {
    #[new]
    fn new(writer: &objectwriterbuilder::ObjectWriterBuilder, config: &config::Config) -> Self {
        let c = config.0.clone();
        Self {
            0: crate::receiver::MultiReceiver::new(writer.inner.clone(), Some(c), false),
        }
    }

    fn push(&mut self, endpoint: &udpendpoint::UDPEndpoint, data: &[u8]) -> PyResult<()> {
        self.0
            .push(&endpoint.inner, data, SystemTime::now())
            .map_err(|e| PyTypeError::new_err(e.0.to_string()))
    }
}
