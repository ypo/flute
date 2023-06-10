use pyo3::prelude::*;

#[pyclass(unsendable)]
#[derive(Debug)]
pub struct UDPEndpoint {
    pub inner: crate::receiver::UDPEndpoint,
}

#[pymethods]
impl UDPEndpoint {
    #[new]
    fn new(
        source_address: Option<&str>,
        destination_group_address: &str,
        port: u16,
    ) -> PyResult<Self> {
        Ok(Self {
            inner: crate::receiver::UDPEndpoint {
                source_address: source_address.map(|f| f.to_string()),
                destination_group_address: destination_group_address.to_string(),
                port,
            },
        })
    }
}
