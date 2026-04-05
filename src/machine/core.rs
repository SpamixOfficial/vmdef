use std::collections::HashMap;

use anyhow::{Result, anyhow};
use pyo3::{Py, PyAny, Python};

use crate::{
    function,
    types::{ArgFormatter, ArgHandler, OpHandler, PopulatedArg, RadState},
    unwrap_or,
};

impl OpHandler {
    pub fn execute_func(&self, args: Py<PyAny>) -> Result<()> {
        unwrap_or!(
            Python::try_attach(|py| -> Result<()> {
                //let kwargs = PyDict::new(py);
                //kwargs.set_item("args", args.clone_ref(py))?;

                self.func.call(py, (args,), None)?;
                Ok(())
            }),
            "could not attach to python interpreter"
        )?;

        Ok(())
    }
}

impl ArgHandler {
    pub fn execute(&self, inp: Vec<PopulatedArg>) -> Result<Py<PyAny>> {
        unwrap_or!(
            Python::try_attach(|py| -> Result<Py<PyAny>> {
                //let kwargs = PyDict::new(py);
                //kwargs.set_item("args", inp.clone())?;
                Ok(self.0.call(py, (inp,), None)?)
            }),
            "could not attach to python interpreter"
        )
    }
}

impl ArgFormatter {
    pub fn execute(&self, inp: Vec<PopulatedArg>, rad_state: HashMap<usize, Vec<u8>>) -> Result<Vec<String>> {
       //unwrap_or!(
            Python::attach(|py| -> Result<Vec<String>> {
                //let kwargs = PyDict::new(py);
                //kwargs.set_item("args", inp.clone())?;
                //kwargs.set_item("rad_state", value)
                let res: Vec<String> = self.0.call(py, (inp,rad_state), None)?.extract(py)?;
                Ok(res)
            })
            //"could not attach to python interpreter"
        //)
    }
}
