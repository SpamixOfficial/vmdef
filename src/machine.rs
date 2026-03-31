use std::collections::HashMap;

use anyhow::{Result, anyhow};
use pyo3::{Python, types::PyDict};

use crate::types::{Machine, OpHandler};

impl OpHandler {
    pub fn execute_func(&self, args: Vec<Vec<u8>>) -> Result<()> {
        Python::try_attach(|py| -> Result<()> {
            let kwargs = PyDict::new(py);

            self.func.call(py, (), Some(&kwargs))?;
            Ok(())
        })
        .unwrap_or(Err(anyhow!(
            "OpHandler::execute_func could not attach to python interpreter"
        )))?;

        Ok(())
    }
}

impl Machine {
    pub fn execute_opcode(&self, op: usize) -> Result<()> {

    }
}