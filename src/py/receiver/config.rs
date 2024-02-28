use pyo3::prelude::*;

#[pyclass]
#[derive(Debug)]
pub struct Config(pub crate::receiver::Config);

#[pymethods]
impl Config {
    #[new]
    fn new() -> Self {
        Self {
            0: crate::receiver::Config {
                ..Default::default()
            },
        }
    }

    #[getter]
    fn get_max_objects_error(&self) -> PyResult<usize> {
        Ok(self.0.max_objects_error)
    }

    #[setter]
    fn set_max_objects_error(&mut self, value: usize) -> PyResult<()> {
        self.0.max_objects_error = value;
        Ok(())
    }

    #[getter]
    fn get_session_timeout_ms(&self) -> PyResult<Option<u64>> {
        Ok(self
            .0
            .session_timeout
            .map(|timeout| timeout.as_millis() as u64))
    }

    #[setter]
    fn set_session_timeout_ms(&mut self, value: Option<u64>) -> PyResult<()> {
        self.0.session_timeout = value.map(|timeout| std::time::Duration::from_millis(timeout));
        Ok(())
    }

    #[getter]
    fn get_object_timeout_ms(&self) -> PyResult<Option<u64>> {
        Ok(self
            .0
            .object_timeout
            .map(|timeout| timeout.as_millis() as u64))
    }

    #[setter]
    fn set_object_timeout_ms(&mut self, value: Option<u64>) -> PyResult<()> {
        self.0.object_timeout = value.map(|timeout| std::time::Duration::from_millis(timeout));
        Ok(())
    }
}
