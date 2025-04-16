use pyo3::{exceptions::PyTypeError, prelude::*};
use std::rc::Rc;

#[pyclass(unsendable)]
#[derive(Debug)]
pub struct ObjectWriterBuilder {
    pub inner: Rc<dyn crate::receiver::writer::ObjectWriterBuilder>,
}

#[pymethods]
impl ObjectWriterBuilder {
    #[new]
    fn new(path: &str) -> PyResult<Self> {
        let writer =
            crate::receiver::writer::ObjectWriterFSBuilder::new(std::path::Path::new(path), true)
                .map_err(|e| PyTypeError::new_err(e.0.to_string()))?;
        Ok(Self {
            inner: Rc::new(writer),
        })
    }

    #[staticmethod]
    fn new_buffer() -> Self {
        let writer = crate::receiver::writer::ObjectWriterBufferBuilder::new(true);
        Self {
            inner: Rc::new(writer),
        }
    }
}
