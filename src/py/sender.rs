use crate::alc;
use pyo3::prelude::*;

#[pyclass]
#[derive(Debug)]
struct Config(alc::sender::Config);

#[pymethods]
impl Config {
    #[new]
    pub fn new() -> Self {
        Self {
            0: alc::sender::Config {
                ..Default::default()
            },
        }
    }
    #[getter]
    pub fn get_interleave_blocks(&self) -> PyResult<u8> {
        Ok(self.0.interleave_blocks)
    }

    #[setter]
    pub fn set_interleave_blocks(&mut self, value: u8) -> PyResult<()> {
        self.0.interleave_blocks = value;
        Ok(())
    }
}

#[pymodule]
pub fn sender(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Config>()?;
    Ok(())
}
