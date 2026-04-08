use pyo3::{pymethods, types::PyFunction, Py};

use crate::types::Emulator;

#[pymethods]
impl Emulator {
    pub fn set_breakpoint(&mut self, a: usize, b: Py<PyFunction>) {
        
    }
}