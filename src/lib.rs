mod types;
mod machine;

use pyo3::prelude::*;

/// VM-Definition framework for CTF python-scripting written in Rust
#[pymodule]
mod vmdef {
    use pyo3::prelude::*;

    /// Formats the sum of two numbers as string.
    #[pyfunction]
    fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
        Ok((a + b).to_string())
    }
}
