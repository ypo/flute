use pyo3::{exceptions::PyTypeError, prelude::*};

use crate::common::alc;

#[pyclass(unsendable)]
#[derive(Debug)]
pub struct LCTHeader {
    pub inner: crate::core::lct::LCTHeader,
    pub payload_id: Option<crate::core::alc::PayloadID>,
}

#[pymethods]
impl LCTHeader {
    #[new]
    fn new(data: &[u8]) -> PyResult<Self> {
        let alc = crate::core::alc::parse_alc_pkt(data)
            .map_err(|e| PyTypeError::new_err(e.0.to_string()))?;

        let payload_id = alc::get_fec_inline_payload_id(&alc).ok();

        Ok(LCTHeader {
            inner: alc.lct,
            payload_id,
        })
    }

    #[getter]
    fn cci(&self) -> PyResult<u128> {
        Ok(self.inner.cci)
    }

    #[getter]
    fn toi(&self) -> PyResult<u128> {
        Ok(self.inner.toi)
    }

    #[getter]
    fn tsi(&self) -> PyResult<u64> {
        Ok(self.inner.tsi)
    }

    #[getter]
    fn sbn(&self) -> PyResult<Option<u32>> {
        Ok(self.payload_id.as_ref().map(|p| p.sbn))
    }

    #[getter]
    fn esi(&self) -> PyResult<Option<u32>> {
        Ok(self.payload_id.as_ref().map(|p| p.esi))
    }
}
