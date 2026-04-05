use std::{cmp::min, collections::HashMap, ffi::CString};

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
}

macro_rules! combine_error_len {
    ($e:expr, $l:expr) => {{ $e.or_else(|e| Err((e, $l))) }};
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
        /*
         * The flow is roughly:
         *     Get Operator and determine args
         *     Populate args
         *     Preprocess their value if there's a preprocess function
         *     Run the RAD statement if there is one
         *     Format it nicely into 1 instruction
         */
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

        let op_item = combine_error_len!(
            option_to_res!(self.define.ops.get(&op), "opcode {} not found", op),
            op_size
        )?;

        let mut len = op_size;
        let mut populated_args: Vec<PopulatedArg> = vec![];

        for arg in &op_item.parser_args {
            populated_args.push(combine_error_len!(arg.populate(&buf, len), len)?);
            len += arg.arg_size as usize;
        }

        // args_preprocess
        if let Some(pre_process) = &op_item.args_preprocess {
            populated_args = combine_error_len!(
                Python::attach(|py| -> Result<Vec<PopulatedArg>> {
                    Ok(pre_process
                        .call(py, (&op, populated_args), None)?
                        .extract(py)?)
                }),
                len
            )?;
        }

        // if there's a RAD we need to execute it now
        let rad_args: HashMap<usize, Vec<u8>>;

        if let Some(rad) = &op_item.rad {
            rad_args = {
                let mut guard = self.rad_state.lock().unwrap();
                combine_error_len!(guard.process(rad.clone(), populated_args.clone()), len)?
            };
        } else {
            rad_args = {
                let guard = self.rad_state.lock().unwrap();
                guard.get_state()
            };
        }

        let formatted = format!(
            "{} {}",
            op_item.op_name,
            combine_error_len!(
                self.define
                    .arg_formatter
                    .as_ref()
                    .unwrap()
                    .execute(populated_args, rad_args),
                len
            )?
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
            let last_remaining = b.remaining();
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
            if last_remaining == b.remaining() {
                return Err(anyhow!("ERROR - Possible infinite loop detected, exiting"));
            }
        }

        Ok(disassembly)
    }
}

/// When processing rad statement RadState takes care of dereferencing registers to values and then re-referencing them
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

    pub fn process(
        &mut self,
        rad_statement: String,
        args: Vec<PopulatedArg>,
    ) -> Result<HashMap<usize, Vec<u8>>> {
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
            if arg.direction == ParserArgDirection::Source {
                continue;
            }
            self.set(arg, res.clone()).or_else(|e| {
                Err(anyhow!(
                    "{}\nStatement at fault: {}",
                    e.to_string(),
                    rad_statement
                ))
            })?;
        }

        Ok(self.0.clone())
    }

    pub fn get_state(&self) -> HashMap<usize, Vec<u8>> {
        return self.0.clone();
    }
}
