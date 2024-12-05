use pyo3::prelude::*;
use pyo3::wrap_pymodule;

mod receiver;
mod sender;

/// A Python module implemented in Rust.
#[pymodule]
fn flute(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();
    m.add_wrapped(wrap_pymodule!(sender::sender))?;
    m.add_wrapped(wrap_pymodule!(receiver::receiver))?;
    Ok(())
}
