use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use std::time::SystemTime;

use super::config;
use super::oti;

#[pyclass]
#[derive(Debug)]
pub struct Sender(crate::sender::Sender);

#[pymethods]
impl Sender {
    #[new]
    pub fn new(tsi: u64, oti: &oti::Oti, config: &config::Config) -> Self {
        Self {
            0: crate::sender::Sender::new(
                crate::core::UDPEndpoint::new(None, "224.0.0.1".to_owned(), 0), // FIXME
                tsi,
                &oti.0,
                &config.0,
            ),
        }
    }

    #[pyo3(signature = (content, content_type, content_location, oti=None))]
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
        let object = crate::sender::ObjectDesc::create_from_buffer(
            content.to_vec(),
            content_type,
            &content_location,
            1,
            None,
            None,
            None,
            None,
            crate::core::lct::Cenc::Null,
            true,
            oti,
            true,
        )
        .map_err(|e| PyTypeError::new_err(e.0.to_string()))?;

        self.0
            .add_object(0, object)
            .map_err(|e| PyTypeError::new_err(e.0.to_string()))
    }

    #[pyo3(signature = (filepath, cenc, content_type, content_location=None, oti=None))]
    fn add_file(
        &mut self,
        filepath: &str,
        cenc: u8,
        content_type: &str,
        content_location: Option<&str>,
        oti: Option<&oti::Oti>,
    ) -> PyResult<u128> {
        let cenc = cenc
            .try_into()
            .map_err(|_| PyTypeError::new_err("Unknown cenc"))?;
        let content_location = content_location.map(|content_location| {
            url::Url::parse(content_location).map_err(|e| PyTypeError::new_err(e.to_string()))
        });
        let content_location = match content_location {
            Some(Err(e)) => return Err(PyTypeError::new_err(e)),
            Some(Ok(url)) => Some(url),
            None => None,
        };

        let oti = oti.map(|o| o.0.clone());
        let object = crate::sender::ObjectDesc::create_from_file(
            std::path::Path::new(filepath),
            content_location.as_ref(),
            content_type,
            true,
            1,
            None,
            None,
            None,
            None,
            cenc,
            true,
            oti,
            true,
        )
        .map_err(|e| PyTypeError::new_err(e.0.to_string()))?;

        self.0
            .add_object(0, object)
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
