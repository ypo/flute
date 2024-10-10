use pyo3::{exceptions::PyTypeError, prelude::*};

#[pyclass]
#[derive(Debug)]
pub struct Oti(pub crate::core::Oti);

#[pymethods]
impl Oti {
    #[new]
    pub fn new() -> Self {
        Self {
            0: Default::default(),
        }
    }

    #[staticmethod]
    fn new_no_code(
        encoding_symbol_length: u16,
        maximum_source_block_length: u16,
    ) -> PyResult<Self> {
        Ok(Self {
            0: crate::core::Oti::new_no_code(encoding_symbol_length, maximum_source_block_length),
        })
    }

    #[staticmethod]
    fn new_reed_solomon_rs28(
        encoding_symbol_length: u16,
        maximum_source_block_length: u8,
        max_number_of_parity_symbols: u8,
    ) -> PyResult<Self> {
        let oti = crate::core::Oti::new_reed_solomon_rs28(
            encoding_symbol_length,
            maximum_source_block_length,
            max_number_of_parity_symbols,
        )
        .map_err(|e| PyTypeError::new_err(e.0.to_string()))?;
        Ok(Self { 0: oti })
    }

    #[staticmethod]
    fn new_reed_solomon_rs28_under_specified(
        encoding_symbol_length: u16,
        maximum_source_block_length: u16,
        max_number_of_parity_symbols: u16,
    ) -> PyResult<Self> {
        let oti = crate::core::Oti::new_reed_solomon_rs28_under_specified(
            encoding_symbol_length,
            maximum_source_block_length,
            max_number_of_parity_symbols,
        )
        .map_err(|e| PyTypeError::new_err(e.0.to_string()))?;
        Ok(Self { 0: oti })
    }

    #[getter]
    fn get_max_transfer_length(&self) -> PyResult<usize> {
        Ok(self.0.max_transfer_length())
    }

    #[getter]
    fn get_fec_encoding_id(&self) -> PyResult<u8> {
        Ok(self.0.fec_encoding_id as u8)
    }

    #[setter]
    fn set_fec_encoding_id(&mut self, value: u8) -> PyResult<()> {
        let encoding_id: crate::core::FECEncodingID = value
            .try_into()
            .map_err(|_| PyTypeError::new_err("Invalid FEC Encoding ID"))?;
        self.0.fec_encoding_id = encoding_id;
        Ok(())
    }

    #[getter]
    fn get_fec_instance_id(&self) -> PyResult<u16> {
        Ok(self.0.fec_instance_id)
    }

    #[setter]
    fn set_fec_instance_id(&mut self, value: u16) -> PyResult<()> {
        self.0.fec_instance_id = value;
        Ok(())
    }

    #[getter]
    fn get_maximum_source_block_length(&self) -> PyResult<u32> {
        Ok(self.0.maximum_source_block_length)
    }

    #[setter]
    fn set_maximum_source_block_length(&mut self, value: u32) -> PyResult<()> {
        self.0.maximum_source_block_length = value;
        Ok(())
    }

    #[getter]
    fn get_encoding_symbol_length(&self) -> PyResult<u16> {
        Ok(self.0.encoding_symbol_length)
    }

    #[setter]
    fn set_encoding_symbol_length(&mut self, value: u16) -> PyResult<()> {
        self.0.encoding_symbol_length = value;
        Ok(())
    }

    #[getter]
    fn get_max_number_of_parity_symbols(&self) -> PyResult<u16> {
        Ok(self.0.encoding_symbol_length)
    }

    #[setter]
    fn set_max_number_of_parity_symbols(&mut self, value: u32) -> PyResult<()> {
        self.0.max_number_of_parity_symbols = value;
        Ok(())
    }

    #[getter]
    fn get_inband_fti(&self) -> PyResult<bool> {
        Ok(self.0.inband_fti)
    }

    #[setter]
    fn set_inband_fti(&mut self, value: bool) -> PyResult<()> {
        self.0.inband_fti = value;
        Ok(())
    }

    // TODO
    // reed_solomon_scheme_specific
    // raptorq_scheme_specific
}
