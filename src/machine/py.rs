use std::{collections::HashMap, ffi::CString, fs, path::PathBuf, sync::Mutex};

use pyo3::{
    Bound, PyResult, Python,
    exceptions::PyException,
    pymethods,
    types::{PyAnyMethods, PyModule},
};

use crate::{
    function,
    pydefine::PyDefine,
    types::{Define, DisFormatter, DisFormatterLine, Machine, MachineConfig, RadState},
};

/// python exposed API
#[pymethods]
impl Machine {
    #[staticmethod]
    #[pyo3(signature = (d, i=None, verbose=false))]
    pub fn init(d: PathBuf, i: Option<PathBuf>, verbose: bool) -> PyResult<Self> {
        let mut config: MachineConfig =
            serde_json::from_str(&fs::read_to_string(d)?).map_err(|x| {
                PyException::new_err(format!(
                    "{} - could not read machine config: {}",
                    function!(),
                    x.to_string()
                ))
            })?;

        // overrides
        config.verbose = config.verbose || verbose;

        let impl_path = config.implementation.clone()
            .or(i)
            .ok_or_else(|| PyException::new_err(format!(
                "{} - At least one of config.implementation and parameter \"i\" should contain a value",
                function!()
            )))?;

        let code = fs::read_to_string(impl_path)?;
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
                //arg_handler: std::mem::take(&mut borrowed.arg_handler),
                arg_formatter: std::mem::take(&mut borrowed.arg_formatter),
                disassembler: std::mem::take(&mut borrowed.disassembler),
            })
        })?;

        dbg!(&config, &define);

        Ok(Machine {
            config,
            define,
            rad_state: Mutex::new(RadState(HashMap::new())),
        })
    }

    #[pyo3(signature = (data))]
    pub fn disassemble(&self, data: Vec<u8>) -> PyResult<String> {
        let disassembly_lines: Vec<DisFormatterLine>;

        if self.config.instruction.op_size.is_none() {
            unimplemented!() // TODO: Implement custom disassembler execution
        } else {
            disassembly_lines = self._disassemble(data)?;
        };

        let disassembly_formatter = DisFormatter {
            lines: disassembly_lines,
            config: self.config.format.clone(),
        };

        Ok(disassembly_formatter.format()?)
    }
}
