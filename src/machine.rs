use std::{fs, path::PathBuf};

use anyhow::{Result, anyhow};
use bytes::{Buf, Bytes};
use pyo3::{
    Py, PyAny, PyResult, Python,
    exceptions::PyException,
    pymethods,
    types::{PyDict, PyDictMethods},
};

use crate::{
    function, option_to_res,
    types::{ArgFormatter, ArgHandler, Machine, MachineConfig, OpHandler, PopulatedArg},
    unwrap_or,
};

impl OpHandler {
    pub fn execute_func(&self, args: Py<PyAny>) -> Result<()> {
        unwrap_or!(
            Python::try_attach(|py| -> Result<()> {
                let kwargs = PyDict::new(py);

                self.func.call(py, (), Some(&kwargs))?;
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
                let kwargs = PyDict::new(py);
                kwargs.set_item("args", inp.clone())?;
                Ok(self.0.call(py, (inp,), Some(&kwargs))?)
            }),
            "could not attach to python interpreter"
        )
    }
}

impl ArgFormatter {
    pub fn execute(&self, inp: Vec<PopulatedArg>) -> Result<Vec<String>> {
        unwrap_or!(
            Python::try_attach(|py| -> Result<Vec<String>> {
                let kwargs = PyDict::new(py);
                kwargs.set_item("args", inp.clone())?;
                let res: Vec<String> = self.0.call(py, (inp,), Some(&kwargs))?.extract(py)?;
                Ok(res)
            }),
            "could not attach to python interpreter"
        )
    }
}

impl Machine {
    /// Disassemble next instruction
    ///
    /// Return value is (len, formatted_instruction)
    ///
    /// **This function will consume the buffer**
    pub fn next_disassemble(&self, mut buf: Bytes) -> Result<(usize, String)> {
        let op = buf.get_u8();
        let op_item = option_to_res!(self.ops.get(&(op as usize)), "opcode {} not found", op)?;

        let mut len = 1;
        let mut populated_args: Vec<PopulatedArg> = vec![];
        for arg in &op_item.parser_args {
            populated_args.push(arg.populate(&buf));
            buf.advance(arg.arg_size as usize);
            len += arg.arg_size as usize;
        }
        let formatted = format!(
            "{} {}",
            op_item.op_name,
            self.arg_formatter.execute(populated_args)?.join(",")
        );

        Ok((len, formatted))
    }
}

#[pymethods]
impl Machine {
    #[staticmethod]
    pub fn init(d: PathBuf, i: PathBuf) -> PyResult<Self> {
        let config: MachineConfig = serde_json::from_str(&fs::read_to_string(d)?).map_err(|e| {
            PyException::new_err(format!(
                "{} - Failed to parse JSON config file: {}",
                function!(),
                e
            ))
        })?;

        Machine {
            config,
            ops: (),
            arg_handlers: (),
            arg_formatter: (),
            primary_args_handler: (),
        }
    }
}
