use pyo3::prelude::*;

mod config;
mod receiverpy;

#[pymodule]
pub fn receiver(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<config::Config>()?;
    m.add_class::<receiverpy::Receiver>()?;
    Ok(())
}
