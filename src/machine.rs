use std::{ffi::CString, fs, path::PathBuf};

use anyhow::{Result, anyhow};
use bytes::{Buf, Bytes};
use pyo3::{
    Bound, Py, PyAny, PyResult, Python,
    exceptions::PyException,
    pymethods,
    types::{PyAnyMethods, PyDict, PyDictMethods, PyFunction, PyModule},
};

use crate::{
    function, option_to_res,
    pydefine::PyDefine,
    types::{ArgFormatter, ArgHandler, Define, Machine, MachineConfig, OpHandler, PopulatedArg},
    unwrap_or, vmdef,
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
                // TODO: Add rad parameter
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
        if self.define.arg_formatter.is_none() {
            return Err(anyhow!(
                "Cannot disassemble without an arg_formatter\n\nTip: Register one with @define.arg_formatter in your .py file!"
            ));
        }
        let op = buf.get_u8();
        let op_item = option_to_res!(
            self.define.ops.get(&(op as usize)),
            "opcode {} not found",
            op
        )?;

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
            self.define
                .arg_formatter
                .as_ref()
                .unwrap()
                .execute(populated_args)?
                .join(",")
        );

        Ok((len, formatted))
    }
}

#[pymethods]
impl Machine {
    #[staticmethod]
    pub fn init(d: PathBuf, i: PathBuf) -> PyResult<Self> {
        let config: MachineConfig = serde_json::from_str(&fs::read_to_string(d)?).map_err(|x| {
            PyException::new_err(format!(
                "{} - could not read machine config: {}",
                function!(),
                x.to_string()
            ))
        })?;

        //pyo3::append_to_inittab!(vmdef);
        let code = fs::read_to_string(i)?;
        let define = Python::attach(|py| -> PyResult<Define> {
            let module = PyModule::from_code(
                py,
                CString::new(code)?.as_c_str(),
                c"machine.py",
                c"machine",
            )?;
            let load = module.getattr("load")?;

            let extracted: Bound<'_, PyDefine> = load.call0()?.extract()?;
            let mut borrowed = extracted.borrow_mut();

            Ok(Define {
                ops: std::mem::take(&mut borrowed.ops), // clone is impossible as all fields contain pointers to Py<PyFunction>
                arg_handler: std::mem::take(&mut borrowed.arg_handler),
                arg_formatter: std::mem::take(&mut borrowed.arg_formatter),
            })
        })?;
        
        dbg!(&config, &define);

        Ok(Machine { config, define })
    }
}
