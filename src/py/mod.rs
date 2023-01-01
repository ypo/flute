use pyo3::prelude::*;
use pyo3::{wrap_pymodule};

mod sender;


/// A Python module implemented in Rust.
#[pymodule]
fn flute(_py: Python, m: &PyModule) -> PyResult<()> {
    pyo3_log::init();

    m.add_wrapped(wrap_pymodule!(sender::sender))?;
    Ok(())
}
