use std::collections::HashMap;

use anyhow::{Result, anyhow};
use pyo3::{Py, PyRefMut, Python};

use crate::{
    function,
    types::{ArgFormatter, EmulatorCore, OpHandler, PopulatedArg},
    unwrap_or,
};

impl OpHandler {
    pub fn execute_func(&self, emu_state: Py<EmulatorCore>, args: Vec<PopulatedArg>) -> Result<()> {
        unwrap_or!(
            Python::try_attach(|py| -> Result<()> {
                self.func.call(py, (emu_state, args), None)?;
                Ok(())
            }),
            "could not attach to python interpreter"
        )?;

        Ok(())
    }
}

/*impl ArgHandler {
    pub fn execute(&self, inp: Vec<PopulatedArg>) -> Result<Py<PyAny>> {
        unwrap_or!(
            Python::try_attach(|py| -> Result<Py<PyAny>> {
                Ok(self.0.call(py, (inp,), None)?)
            }),
            "could not attach to python interpreter"
        )
    }
}*/

impl ArgFormatter {
    pub fn execute(&self, inp: Vec<PopulatedArg>, rad_state: HashMap<usize, Vec<u8>>) -> Result<Vec<String>> {
        unwrap_or!(
            Python::try_attach(|py| -> Result<Vec<String>> {
                let res: Vec<String> = self.0.call(py, (inp,rad_state), None)?.extract(py)?;
                Ok(res)
            }),
            "could not attach to python interpreter"
        )
    }
}
