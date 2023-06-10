use pyo3::prelude::*;

mod config;
mod objectwriterbuilder;
mod multireceiver;
mod receiverpy;
mod udpendpoint;

#[pymodule]
pub fn receiver(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<config::Config>()?;
    m.add_class::<objectwriterbuilder::ObjectWriterBuilder>()?;
    m.add_class::<receiverpy::Receiver>()?;
    m.add_class::<multireceiver::MultiReceiver>()?;
    Ok(())
}
