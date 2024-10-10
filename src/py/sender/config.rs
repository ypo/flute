use pyo3::{exceptions::PyTypeError, prelude::*};

#[pyclass]
#[derive(Debug)]
pub struct Config(pub crate::sender::Config);

#[pymethods]
impl Config {
    #[new]
    pub fn new() -> Self {
        Self {
            0: crate::sender::Config {
                ..Default::default()
            },
        }
    }

    #[getter]
    pub fn get_fdt_duration_ms(&self) -> PyResult<u64> {
        Ok(self.0.fdt_duration.as_millis() as u64)
    }

    #[setter]
    pub fn set_fdt_duration_ms(&mut self, value: u64) -> PyResult<()> {
        self.0.fdt_duration = std::time::Duration::from_millis(value);
        Ok(())
    }

    #[getter]
    pub fn get_fdt_start_id(&self) -> PyResult<u32> {
        Ok(self.0.fdt_start_id)
    }

    #[setter]
    pub fn set_fdt_start_id(&mut self, value: u32) -> PyResult<()> {
        self.0.fdt_start_id = value;
        Ok(())
    }

    #[getter]
    pub fn get_fdt_cenc(&self) -> PyResult<u8> {
        Ok(self.0.fdt_cenc as u8)
    }

    #[setter]
    pub fn set_fdt_cenc(&mut self, value: u8) -> PyResult<()> {
        let cenc = match crate::core::lct::Cenc::try_from(value) {
            Ok(res) => res,
            Err(_) => return Err(PyTypeError::new_err("Wrong CENC parameter")),
        };

        self.0.fdt_cenc = cenc;
        Ok(())
    }

    #[getter]
    pub fn get_fdt_inband_sct(&self) -> PyResult<bool> {
        Ok(self.0.fdt_inband_sct)
    }

    #[setter]
    pub fn set_fdt_inband_sct(&mut self, value: bool) -> PyResult<()> {
        self.0.fdt_inband_sct = value;
        Ok(())
    }

    #[getter]
    pub fn get_multiplex_files(&self) -> PyResult<u32> {
        Ok(self.0.priority_queues.get(&0).unwrap().multiplex_files)
    }

    #[setter]
    pub fn set_multiplex_files(&mut self, value: u32) -> PyResult<()> {
        self.0.priority_queues.get_mut(&0).unwrap().multiplex_files = value;
        Ok(())
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
