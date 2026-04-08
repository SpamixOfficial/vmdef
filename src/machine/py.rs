use std::{
    collections::HashMap,
    ffi::CString,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use bytes::Bytes;
use pyo3::{
    Bound, PyResult, Python,
    exceptions::PyException,
    pymethods,
    types::{PyAnyMethods, PyModule},
};

use crate::{
    function, option_to_res,
    pydefine::PyDefine,
    types::{
        ArgFormatter, Define, DisFormatter, DisFormatterLine, Disassembler, EmuRegister, Emulator,
        Machine, MachineConfig, OpMap, RadState,
    },
};

use anyhow::anyhow;

/// python exposed API
#[pymethods]
impl Machine {
    #[staticmethod]
    #[pyo3(signature = (d, i=None, verbose=false))]
    /// Create a new `machine` object
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

            // clone is impossible as all fields contain pointers to Py<PyFunction>
            let ops: Arc<OpMap> = Arc::new(std::mem::take(&mut borrowed.ops));
            let arg_formatter: Option<ArgFormatter> = std::mem::take(&mut borrowed.arg_formatter);
            let disassembler: Option<Disassembler> = std::mem::take(&mut borrowed.disassembler);

            Ok(Define {
                ops,
                //arg_handler: std::mem::take(&mut borrowed.arg_handler),
                arg_formatter,
                disassembler,
            })
        })?;

        dbg!(&config, &define);

        Ok(Machine {
            config,
            define,
            rad_state: Mutex::new(RadState(HashMap::new())),
        })
    }

    /// Disassemble provided data
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

    #[pyo3(signature = (data, registers=None, memory=None))]
    pub fn create_emulation(
        &self,
        data: Vec<u8>,
        registers: Option<HashMap<String, Vec<u8>>>,
        memory: Option<Vec<u8>>,
    ) -> PyResult<Emulator> {
        let m_size = self.config.memory.initial_size.unwrap_or(4096);
        let memory: Vec<u8> = memory.unwrap_or(vec![0; m_size]);

        let mut emu_registers: HashMap<usize, EmuRegister> = HashMap::new();
        let pc: Option<EmuRegister> = None;
        let sp: Option<EmuRegister> = None;
        let flags: Option<EmuRegister> = None;
        for reg in &self.config.registers {
            emu_registers.insert(
                reg.1.code,
                EmuRegister {
                    max_size: reg.1.size as usize,
                    write: reg.1.write,
                    ..Default::default()
                },
            );
        }

        if let Some(regs) = registers {
            for reg in regs {
                let reg_info = option_to_res!(
                    self.config.registers.get(&reg.0),
                    "Could not find register {}",
                    &reg.0
                )?;

                if reg.1.len() > reg_info.size as usize {
                    Err(anyhow!(
                        "{} - Initial data for register {} is bigger than allowed size {}",
                        function!(),
                        reg.0,
                        reg_info.size
                    ))?;
                }

                emu_registers.insert(
                    reg_info.code,
                    EmuRegister {
                        data: Bytes::from(reg.1),
                        max_size: reg_info.size as usize,
                        write: reg_info.write,
                    },
                );
            }
        }

        Ok(Emulator {
            data: Bytes::from(data),
            memory_config: self.config.memory.clone(),
            memory,
            registers: emu_registers,
            pc: pc.unwrap(),
            sp,
            flags,
            ops: self.define.ops.clone(),
        })
    }
}
