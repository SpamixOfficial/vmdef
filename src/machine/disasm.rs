use std::{cmp::max, ffi::CString};

use anyhow::{Result, anyhow};
use bytes::{Buf, Bytes};
use pyo3::{
    Python,
    types::{PyAnyMethods, PyDict},
};

use crate::{
    buf_as_usize, function, option_to_res,
    types::{Machine, ParserArgDirection, ParserArgType, PopulatedArg, RadState},
};

macro_rules! rad_error {
    ($e:expr) => {{
        anyhow!(format!(
            "WARNING - RAD statement not executed because {}. Either apply a pre-process or discard your RAD.",
            $e,
        ))
    }};
    ($e:expr, $s:expr) => {{
        anyhow!(format!(
            "WARNING - RAD statement not executed because {}. Either apply a pre-process or discard your RAD.\nStatement at fault: {}",
            $e,
            $s
        ))
    }};
}

impl Machine {
    /// Disassemble next instruction
    ///
    /// Return value is (len, formatted_instruction)
    ///
    /// **This function will not consume the buffer**
    pub fn next_disassemble(
        &self,
        buf: &Bytes,
        op_size: usize,
    ) -> Result<(usize, String), (anyhow::Error, usize)> {
        assert!(op_size <= size_of::<usize>());

        if self.define.arg_formatter.is_none() {
            return Err((
                anyhow!(
                    "Cannot disassemble without an arg_formatter\n\nTip: Register one with @define.arg_formatter in your .py file!"
                ),
                0,
            ));
        }

        let mut op = buf_as_usize!(buf, op_size);

        /*let mut op_bytes = [0u8; size_of::<usize>()];
        op_bytes[..op_size].copy_from_slice(&buf[..op_size]);

        let mut op = usize::from_le_bytes(op_bytes);*/

        if !self.config.little_endian {
            op = op.swap_bytes();
        }

        let op_item = option_to_res!(self.define.ops.get(&op), "opcode {} not found", op)
            .or_else(|e| Err((e, op_size)))?;

        let mut len = op_size;
        let mut populated_args: Vec<PopulatedArg> = vec![];

        for arg in &op_item.parser_args {
            populated_args.push(arg.populate(&buf, len).or_else(|e| Err((e, len)))?);
            len += arg.arg_size as usize;
        }

        let formatted = format!(
            "{} {}",
            op_item.op_name,
            self.define
                .arg_formatter
                .as_ref()
                .unwrap()
                .execute(populated_args, self.rad_state.clone())
                .or_else(|e| Err((e, len)))?
                .join(",")
        );

        Ok((len, formatted))
    }

    pub fn _disassemble(&self, data: Vec<u8>) -> Result<String> {
        let op_size = self.config.instruction.op_size.unwrap();

        if op_size > size_of::<usize>() {
            return Err(anyhow!(
                "{} - op_size ({} bytes) was bigger than max size of {} bytes",
                function!(),
                op_size,
                size_of::<usize>()
            ));
        }

        let mut b = Bytes::from(data);

        let mut disassembly: String = String::new();

        while b.remaining() > 0 {
            if op_size > b.remaining() {
                disassembly += &hex::encode(b.to_vec());
                break;
            }
            let len;
            let tmp_dis: String;

            match self.next_disassemble(&b, op_size) {
                Ok((l, d)) => {
                    tmp_dis = d;
                    len = l;
                }
                Err((e, l)) => {
                    eprintln!("WARNING - {}", e.to_string());
                    tmp_dis = String::from("INVALID");
                    len = l;
                }
            }

            disassembly += &format!("{}\t{}\n", hex::encode(&b[..len]), tmp_dis);

            b.advance(len);
        }

        Ok(disassembly)
    }
}

/// When processing evals RadState takes care of dereferencing registers to values and then re-referencing them
///
/// NOTE: A RAD statement will not run if a source-register is missing a value
impl RadState {
    pub fn get(&self, arg: PopulatedArg) -> Result<Vec<u8>> {
        if arg.t == ParserArgType::None || arg.t == ParserArgType::Memory {
            return Err(rad_error!("RAD cannot process Memory-type arguments"));
        }

        if arg.direction == ParserArgDirection::Destination && arg.t != ParserArgType::Register {
            return Err(rad_error!("only registers can be destinations"));
        }

        if arg.t == ParserArgType::Immediate {
            Ok(arg.arg_val)
        } else {
            let reg_key = buf_as_usize!(arg.arg_val);
            if let Some(x) = self.0.get(&reg_key) {
                Ok(x.clone())
            } else if arg.direction == ParserArgDirection::Source {
                Err(rad_error!("source register cannot be empty"))
            } else {
                Ok(vec![0]) // If the destination is non-existant we just return a filler value
            }
        }
    }

    pub fn set(&mut self, arg: PopulatedArg, value: Vec<u8>) -> Result<()> {
        if arg.direction == ParserArgDirection::Source {
            // None type is fine, its "bidirectional"
            return Err(rad_error!(
                "cannot set source (if the value is bidirectional remove your direction flag)"
            ));
        }

        if arg.t != ParserArgType::Register {
            return Err(rad_error!("can only set registers"));
        }
        let reg_key = buf_as_usize!(arg.arg_val);
        self.0.insert(reg_key, value);

        Ok(())
    }

    pub fn process_eval(
        &mut self,
        rad_statement: String,
        args: Vec<PopulatedArg>,
    ) -> Result<Vec<Vec<u8>>> {
        let mut py_args: Vec<Vec<u8>> = vec![];
        for arg in args.clone() {
            py_args.push(self.get(arg).or_else(|e| {
                Err(anyhow!(
                    "{}\nStatement at fault: {}",
                    e.to_string(),
                    rad_statement
                ))
            })?);
        }

        let mut res_args: Vec<Vec<u8>> = vec![];
        Python::attach(|py| {
            let locals = PyDict::new(py);
            locals.set_item("args", &py_args).unwrap();
            py.run(
                CString::new(rad_statement.clone()).unwrap().as_c_str(),
                None,
                Some(&locals),
            )
            .unwrap();
            res_args = locals.get_item("args").unwrap().extract().unwrap();
        });
        assert_eq!(res_args.len(), py_args.len());

        for (res, arg) in res_args.iter().zip(args) {
            self.set(arg, res.clone()).or_else(|e| {
                Err(anyhow!(
                    "{}\nStatement at fault: {}",
                    e.to_string(),
                    rad_statement
                ))
            })?;
        }

        Ok(res_args)
    }
}
