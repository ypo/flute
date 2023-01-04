use crate::alc;
use pyo3::prelude::*;

#[pyclass(unsendable)]
#[derive(Debug)]
pub struct Receiver(alc::receiver::Receiver);

#[pymethods]
impl Receiver {
    /* 
    #[new]
    pub fn new(tsi: u64, config: &config::Config) -> Self {
        let c = config.0.clone();
        Self {
            0: alc::receiver::Receiver::new(tsi, writer, Some(c)),
        }
    }
    */
}
