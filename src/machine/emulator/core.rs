use anyhow::{Result, anyhow};
use bytes::Bytes;
use pyo3::{Py, PyRef, PyRefMut, Python, ffi::PyObject};

use crate::{
    buf_as_usize, function, option_to_res,
    types::{Emulator, EmulatorCore, OpHandler, ParserArgType, PopulatedArg},
};
use std::cmp::min;

impl Emulator {
    pub fn get_opcode_args(
        &self,
        state: &PyRef<'_, EmulatorCore>,
        op: usize,
        op_item: &OpHandler,
    ) -> Result<(usize, Vec<PopulatedArg>)> {
        let mut len = self.op_size;
        let mut populated_args: Vec<PopulatedArg> = vec![];

        for arg in &op_item.parser_args {
            let mut p_arg = arg.populate(&state.data, len)?;

            if p_arg.t == ParserArgType::Memory && !self.machine_config.memory.layout.is_empty() {
                for (r, v) in &self.machine_config.memory.layout {
                    if r.contains(&buf_as_usize!(p_arg.arg_val)) {
                        p_arg.memory_region = Some(v.name.clone())
                    }
                }
            }

            populated_args.push(p_arg);
            len += arg.arg_size as usize;
        }

        // args_preprocess
        if let Some(pre_process) = &op_item.args_preprocess {
            populated_args = Python::attach(|py| -> Result<Vec<PopulatedArg>> {
                Ok(pre_process
                    .call(py, (&op, populated_args), None)?
                    .extract(py)?)
            })?
        };

        Ok((len, populated_args))
    }

    pub fn execute_next_instruction(&self, py: Python<'_>) -> Result<()> {
        let state = self.state.bind(py).borrow();

        let mut op = buf_as_usize!(state.data, self.op_size);

        if !self.machine_config.little_endian {
            op = op.swap_bytes();
        }

        let op_item = option_to_res!(self.ops.get(&op), "opcode {} not found", op)?;

        let (len, args) = self.get_opcode_args(&state, op, op_item)?;

        op_item.execute_func(self.state.clone_ref(py), args)?;

        let mut pc_val = buf_as_usize!(state.pc.data);

        if !self.machine_config.little_endian {
            pc_val = pc_val.swap_bytes();
        }

        pc_val += len + self.op_size;

        if pc_val >= state.data.len() {
            return Err(anyhow!(
                "ERROR: pc register is out of bounds:  {} >= {}",
                pc_val,
                state.data.len()
            ));
        }

        let mut state = self.state.bind(py).borrow_mut();

        state.pc.data = Bytes::from(Vec::from(pc_val.swap_bytes().to_le_bytes()));

        Ok(())
    }

    pub fn run(&mut self, py: Python<'_>) -> Result<()> {
        let state = self.state.bind(py).borrow();
        loop {
            self.execute_next_instruction(py)?;

            if state.paused || state.halted {
                break;
            }
        }

        Ok(())
    }
}
