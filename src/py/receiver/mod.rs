use pyo3::prelude::*;

mod config;
mod flutewriter;
mod multireceiver;
mod receiverpy;

#[pymodule]
pub fn receiver(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<config::Config>()?;
    m.add_class::<flutewriter::FluteWriter>()?;
    m.add_class::<receiverpy::Receiver>()?;
    m.add_class::<multireceiver::MultiReceiver>()?;
    Ok(())
}
