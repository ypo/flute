use std::rc::Rc;

use crate::alc;
use pyo3::{exceptions::PyTypeError, prelude::*};

#[pyclass(unsendable)]
#[derive(Debug)]
pub struct FluteWriter {
    pub inner: Rc<dyn alc::objectwriter::FluteWriter>,
}

#[pymethods]
impl FluteWriter {
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        let writer = alc::objectwriter::FluteWriterFS::new(std::path::Path::new(path))
            .map_err(|e| PyTypeError::new_err(e.0.to_string()))?;
        Ok(Self {
            inner: Rc::new(writer),
        })
    }

    #[staticmethod]
    fn new_buffer() -> Self {
        let writer = alc::objectwriter::FluteWriterBuffer::new();
        Self {
            inner: Rc::new(writer),
        }
    }
}
