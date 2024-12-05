use pyo3::prelude::*;

#[pyclass(unsendable)]
#[derive(Debug)]
pub struct UDPEndpoint {
    pub inner: crate::core::UDPEndpoint,
}

#[pymethods]
impl UDPEndpoint {
    #[new]
    #[pyo3(signature = (destination_group_address, port, source_address=None))]
    fn new(
        destination_group_address: &str,
        port: u16,
        source_address: Option<&str>,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: crate::core::UDPEndpoint {
                source_address: source_address.map(|f| f.to_string()),
                destination_group_address: destination_group_address.to_string(),
                port,
            },
        })
    }
}
