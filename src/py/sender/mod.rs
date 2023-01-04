use pyo3::prelude::*;

mod config;
mod oti;
mod senderpy;

#[pymodule]
pub fn sender(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<config::Config>()?;
    m.add_class::<senderpy::Sender>()?;
    m.add_class::<oti::Oti>()?;
    Ok(())
}
