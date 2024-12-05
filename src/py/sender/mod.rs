use pyo3::prelude::*;

mod config;
mod oti;
mod senderpy;

#[pymodule]
pub fn sender(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<config::Config>()?;
    m.add_class::<senderpy::Sender>()?;
    m.add_class::<oti::Oti>()?;
    Ok(())
}
