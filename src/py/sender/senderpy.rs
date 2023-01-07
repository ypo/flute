use crate::alc;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use std::time::SystemTime;

use super::config;
use super::oti;

#[pyclass]
#[derive(Debug)]
pub struct Sender(alc::sender::Sender);

#[pymethods]
impl Sender {
    #[new]
    pub fn new(tsi: u64, oti: &oti::Oti, config: &config::Config) -> Self {
        Self {
            0: alc::sender::Sender::new(tsi, &oti.0, &config.0),
        }
    }

    fn add_object_from_buffer(
        &mut self,
        content: &[u8],
        content_type: &str,
        content_location: &str,
        oti: Option<&oti::Oti>,
    ) -> PyResult<u128> {
        let content_location =
            url::Url::parse(content_location).map_err(|e| PyTypeError::new_err(e.to_string()))?;

        let oti = oti.map(|o| o.0.clone());
        let object = alc::objectdesc::ObjectDesc::create_from_buffer(
            content,
            content_type,
            &content_location,
            1,
            None,
            alc::lct::Cenc::Null,
            true,
            oti,
            true,
        )
        .map_err(|e| PyTypeError::new_err(e.0.to_string()))?;

        self.0
            .add_object(object)
            .map_err(|e| PyTypeError::new_err(e.0.to_string()))
    }

    fn remove_object(&mut self, toi: u128) -> bool {
        self.0.remove_object(toi)
    }

    fn nb_objects(&self) -> usize {
        self.0.nb_objects()
    }

    fn publish(&mut self) -> PyResult<()> {
        self.0
            .publish(SystemTime::now())
            .map_err(|e| PyTypeError::new_err(e.0.to_string()))
    }

    fn read(&mut self) -> PyResult<Option<Vec<u8>>> {
        Ok(self.0.read(SystemTime::now()))
    }
}
