use anyhow::{Result, anyhow};
use bytes::Bytes;
use pyo3::{Py, PyRef, PyRefMut, Python, ffi::PyObject};

use crate::{
    buf_as_usize, function, option_to_res,
    types::{EmuWatchpoint, Emulator, EmulatorState, OpHandler, ParserArgType, PopulatedArg},
};
use std::{cmp::min, ops::Range};

impl Emulator {
    pub fn get_opcode_args(
        &self,
        op: usize,
        op_item: &OpHandler,
    ) -> Result<(usize, Vec<PopulatedArg>)> {
        let mut len = self.op_size;
        let mut populated_args: Vec<PopulatedArg> = vec![];

        for arg in &op_item.parser_args {
            let mut p_arg = arg.populate(&self.state.data, len)?;

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

    fn get_breakpoints_at_addr(&self, addr: usize) -> Vec<usize> {
        self.breakpoints
            .iter()
            .filter_map(|(k, v)| (v.address == addr).then(|| k.clone()))
            .collect()
    }

    fn get_watchpoints_at_addr(&self, addr: usize) -> Vec<usize> {
        self.breakpoints
            .iter()
            .filter_map(|(k, v)| {
                ((v.address..v.address + v.size).contains(&addr)).then(|| k.clone())
            })
            .collect()
    }

    fn handle_watchpoints_breakpoints(&mut self, addr: usize) {
        let breakpoints = self.get_breakpoints_at_addr(addr);

        for breakpoint in breakpoints {
            
        }

        let watchpoints = self.get_watchpoints_at_addr(addr);
    }

    pub fn execute_at(&mut self, pc: usize) -> Result<usize> {
        let mut op = buf_as_usize!(self.state.data[pc..], self.op_size);

        if !self.machine_config.little_endian {
            op = op.swap_bytes();
        }

        let op_item = option_to_res!(self.ops.get(&op), "opcode {} not found", op)?;

        let (len, args) = self.get_opcode_args(op, op_item)?;

        self.state = op_item.execute_func(self.state.clone(), args)?;

        Ok(len + self.op_size)
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            let mut pc_val = buf_as_usize!(self.state.pc.data);

            //self.handle_watchpoints_breakpoints(pc_val);

            let instr_len = self.execute_at(pc_val)?;

            if self.state.paused || self.state.halted {
                break;
            }

            pc_val += instr_len;
            if pc_val >= self.state.data.len() {
                eprintln!(
                    "Program reached end (pc register is out of bounds: {} >= {}), stopping automatically",
                    pc_val,
                    self.state.data.len()
                );
                self.state.halted = true;
                break;
            }
            self.state.pc.data = Bytes::from(Vec::from(pc_val.to_le_bytes()));
        }

        Ok(())
    }
}