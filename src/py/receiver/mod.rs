use pyo3::prelude::*;

mod config;
mod objectwriterbuilder;
mod multireceiver;
mod receiverpy;
mod udpendpoint;
mod lct;

#[pymodule]
pub fn receiver(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<config::Config>()?;
    m.add_class::<objectwriterbuilder::ObjectWriterBuilder>()?;
    m.add_class::<receiverpy::Receiver>()?;
    m.add_class::<multireceiver::MultiReceiver>()?;
    m.add_class::<udpendpoint::UDPEndpoint>()?;
    m.add_class::<lct::LCTHeader>()?;
    Ok(())
}
