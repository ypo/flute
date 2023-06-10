use super::{config, objectwriterbuilder, udpendpoint};
use pyo3::{exceptions::PyTypeError, prelude::*};
use std::time::SystemTime;

#[pyclass(unsendable)]
#[derive(Debug)]
pub struct Receiver(crate::receiver::Receiver);

#[pymethods]
impl Receiver {
    #[new]
    fn new(
        endpoint: &udpendpoint::UDPEndpoint,
        tsi: u64,
        writer: &objectwriterbuilder::ObjectWriterBuilder,
        config: &config::Config,
    ) -> Self {
        let c = config.0.clone();
        Self {
            0: crate::receiver::Receiver::new(&endpoint.inner, tsi, writer.inner.clone(), Some(c)),
        }
    }

    fn push(&mut self, data: &[u8]) -> PyResult<()> {
        self.0
            .push_data(data, SystemTime::now())
            .map_err(|e| PyTypeError::new_err(e.0.to_string()))
    }
}
