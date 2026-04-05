use std::collections::HashMap;

use anyhow::Result;
use pyo3::types::{PyAnyMethods, PyFunction};
use pyo3::{Py, PyResult};
use pyo3::{Python, pyclass, pymethods};

use crate::types::{ArgFormatter, ArgHandler, Disassembler, OpHandler, OpMap, ParserArg};

#[pyclass(name = "define")]
pub struct PyDefine {
    pub ops: OpMap,
    pub arg_handler: Option<ArgHandler>,
    pub arg_formatter: Option<ArgFormatter>,
    pub disassembler: Option<Disassembler>,
}

#[pymethods]
impl PyDefine {
    #[staticmethod]
    pub fn init() -> Self {
        Self {
            ops: HashMap::new(),
            arg_handler: Option::None,
            arg_formatter: Option::None,
            disassembler: Option::None, //primary_args_handler: String::new(),
        }
    }

    #[pyo3(signature = (code, args=vec![], rad=None, name=None, args_preprocess=None))]
    pub fn op(
        slf: Py<Self>,
        code: usize,
        args: Vec<String>,
        rad: Option<String>,
        name: Option<String>,
        args_preprocess: Option<Py<PyFunction>>,
    ) -> OpDecorator {
        OpDecorator {
            parent: slf,
            code,
            args,
            rad,
            name,
            args_preprocess,
        }
    }

    pub fn arg_formatter(slf: Py<Self>, func: Py<PyFunction>) -> PyResult<Py<PyFunction>> {
        Python::attach(|py| -> PyResult<Py<PyFunction>> {
            let mut s = slf.borrow_mut(py);
            s.arg_formatter = Some(ArgFormatter(func.clone_ref(py)));
            Ok(func)
        })
    }

    pub fn arg_handler(slf: Py<Self>, func: Py<PyFunction>) -> PyResult<Py<PyFunction>> {
        Python::attach(|py| -> PyResult<Py<PyFunction>> {
            let mut s = slf.borrow_mut(py);
            s.arg_handler = Some(ArgHandler(func.clone_ref(py)));
            Ok(func)
        })
    }
}

#[pyclass]
pub struct OpDecorator {
    parent: Py<PyDefine>,
    args_preprocess: Option<Py<PyFunction>>,
    code: usize,
    args: Vec<String>,
    rad: Option<String>,
    name: Option<String>,
}

#[pymethods]
impl OpDecorator {
    fn __call__(&self, func: Py<PyFunction>) -> PyResult<Py<PyFunction>> {
        let parser_args: Vec<ParserArg> = self
            .args
            .iter()
            .map(|x| ParserArg::from_string(x.to_string()))
            .collect::<Result<Vec<ParserArg>>>()?;

        Python::attach(|py| {
            let mut s = self.parent.bind(py).borrow_mut();
            let op_name: String = self.name.clone().unwrap_or(
                func.bind(py)
                    .getattr("__name__")?
                    .extract::<String>()?
                    .to_uppercase(), // default uppercase cause it looks nicer imo
            );

            let args_preprocess: Option<Py<PyFunction>> =
                self.args_preprocess.as_ref().map(|x| x.clone_ref(py));

            let handler = OpHandler {
                func: func.clone_ref(py),
                op_name,
                parser_args,
                rad: self.rad.clone(),
                args_preprocess,
            };
            s.ops.insert(self.code, handler);

            Ok(func)
        })
    }
}
