use anyhow::anyhow;
use bytes::Bytes;
use pyo3::{Py, PyRefMut, PyResult, Python, pymethods, types::PyFunction};

use crate::{
    function,
    types::{EmuWatchpoint, Emulator, EmulatorState},
};

#[pymethods]
impl Emulator {
    /// Create a new breakpoint, return value is specific ID/n for that breakpoint
    #[pyo3(signature = (address, callback=None))]
    pub fn set_breakpoint(&mut self, address: usize, callback: Option<Py<PyFunction>>) -> usize {
        self.breakpoints_id += 1;
        self.breakpoints.insert(
            self.breakpoints_id,
            EmuWatchpoint {
                address,
                size: 1,
                callback,
                hit: 0,
                disabled: false,
            },
        );
        return self.breakpoints_id;
    }

    /// Remove all breakpoints at an address
    #[pyo3(signature = (address))]
    pub fn remove_all_breakpoints_at(&mut self, address: usize) {
        self.breakpoints.retain(|_, x| x.address != address);
    }

    /// Remove the breakpoint with id `id`
    #[pyo3(signature = (id))]
    pub fn remove_breakpoint(&mut self, id: usize) {
        self.breakpoints.remove(&id);
    }

    /// Create a new watchpoint at `address` with size `size`, return value is specific ID/n for that breakpoint
    #[pyo3(signature = (address, size=1, callback=None))]
    pub fn set_watchpoint(
        &mut self,
        address: usize,
        size: usize,
        callback: Option<Py<PyFunction>>,
    ) -> usize {
        self.watchpoints_id += 1;
        self.watchpoints.insert(
            self.watchpoints_id,
            EmuWatchpoint {
                address,
                size,
                callback,
                hit: 0,
                disabled: false,
            },
        );
        return self.watchpoints_id;
    }

    /// Remove all watchpoints at an address
    #[pyo3(signature = (address))]
    pub fn remove_all_watchpoints_at(&mut self, address: usize) {
        self.watchpoints.retain(|_, x| x.address != address);
    }

    /// Remove the watchpoint with id `id`
    #[pyo3(signature = (id))]
    pub fn remove_watchpoint(&mut self, id: usize) {
        self.watchpoints.remove(&id);
    }

    pub fn start(&mut self) -> PyResult<()> {
        self.run()?;
        Ok(())
    }

    pub fn kill(&mut self) -> PyResult<()> {
        Ok(())
    }

    pub fn restart(&mut self) -> PyResult<()> {
        Ok(())
    }

    pub fn unpause(&mut self) -> PyResult<()> {
        if !self.state.paused {
            Err(anyhow!(
                "{} - Cannot unpause the emulator if it isn't paused",
                function!()
            ))?;
        } else if self.state.halted {
            Err(anyhow!(
                "{} - Cannot unpause the emulator if it's halted (there's nothing to run!)",
                function!()
            ))?;
        }
        Ok(())
    }
}

#[pymethods]
impl EmulatorState {
    #[pyo3(signature = (reg, val))]
    pub fn set_register(&mut self, reg: usize, val: Vec<u8>) -> PyResult<()> {
        let ereg = self
            .registers
            .get_mut(&reg)
            .ok_or(anyhow!("No such register: {}", reg))?;
        ereg.data = Bytes::from(val);

        Ok(())
    }

    #[pyo3(signature = (reg))]
    pub fn get_register(&self, reg: usize) -> PyResult<Vec<u8>> {
        let ereg = self
            .registers
            .get(&reg)
            .ok_or(anyhow!("No such register: {}", reg))?;
        Ok(ereg.data.to_vec())
    }
}
