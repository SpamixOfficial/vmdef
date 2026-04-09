use anyhow::{Result, anyhow};
use bytes::{Buf, Bytes};
use pyo3::{
    Bound, FromPyObject, IntoPyObject, Py, PyAny, PyErr, PyResult, Python, pyclass,
    types::{PyAnyMethods, PyBytes, PyDict, PyFunction},
};
use serde::{Deserialize, Deserializer};
use serde_with::{DefaultOnNull, serde_as};
use std::{
    borrow::Cow,
    collections::HashMap,
    ops::Range,
    path::PathBuf,
    sync::{Arc, Mutex},
};

pub type OpMap = HashMap<usize, OpHandler>;

#[pyclass(name = "machine")]
pub struct Machine {
    pub config: MachineConfig,
    pub define: Define,
    pub rad_state: Mutex<RadState>,
}

#[derive(Clone)]
pub struct RadState(pub HashMap<usize, Vec<u8>>);

// This is the owned final type of the Define Python API
#[derive(Debug)]
pub struct Define {
    pub ops: Arc<OpMap>,
    //pub arg_handler: Option<ArgHandler>,
    pub arg_formatter: Option<ArgFormatter>,
    pub disassembler: Option<Disassembler>,
    //pub primary_args_handler: String,
}

/*#[derive(Debug)]
pub struct ArgHandler(pub Py<PyFunction>);*/

#[derive(Debug)]
pub struct ArgFormatter(pub Py<PyFunction>);

#[derive(Debug)]
pub struct Disassembler(pub Py<PyFunction>);

#[derive(Debug)]
pub struct OpHandler {
    pub func: Py<PyFunction>,
    pub parser_args: Vec<ParserArg>,
    pub args_preprocess: Option<Py<PyFunction>>,
    pub op_name: String,
    pub rad: Option<String>,
}

#[serde_as]
#[derive(Deserialize, Debug, Clone)]
pub struct MachineConfig {
    pub registers: HashMap<String, Register>,

    #[serde_as(as = "DefaultOnNull")]
    #[serde(default)]
    pub memory: MemoryConfig,

    pub instruction: InstructionConfig,

    #[serde_as(as = "DefaultOnNull")]
    #[serde(default = "default_true")]
    pub little_endian: bool,

    pub implementation: Option<PathBuf>,

    #[serde_as(as = "DefaultOnNull")]
    pub verbose: bool,

    #[serde_as(as = "DefaultOnNull")]
    #[serde(default)]
    pub format: FormattingConfig,
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct MemoryConfig {
    pub size: Option<usize>, // upper size limit for memory vector, None just lets it grow continuously
    pub initial_size: Option<usize>,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_range_hashmap")]
    pub layout: HashMap<Range<usize>, MemoryLayout>,
}

#[serde_as]
#[derive(Deserialize, Debug, Clone)]
pub struct MemoryLayout {
    pub name: String,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_memory_permissions")]
    pub permissions: MemoryLayoutPermissions,
}

#[derive(Deserialize, Debug, Clone)]
pub struct MemoryLayoutPermissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl Default for MemoryLayoutPermissions {
    fn default() -> Self {
        Self {
            read: true,
            write: true,
            execute: true,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct InstructionConfig {
    pub op_size: Option<usize>, // if None custom decoder will be enforced
}

#[derive(Debug, Clone, Default)]
pub struct Register {
    pub code: usize,
    pub attribute: RegisterAttribute,
    pub size: u8, // size in bytes, max is sizeof(usize)
    pub write: bool,
}

impl<'de> Deserialize<'de> for Register {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        #[serde_as]
        enum Raw {
            Simple(usize),
            Complex {
                code: Option<usize>,
                #[serde(default)]
                #[serde_as(as = "DefaultOnNull")]
                attribute: RegisterAttribute,
                #[serde(default = "default_1")]
                #[serde_as(as = "DefaultOnNull")]
                size: u8,
                #[serde(default = "default_true")]
                write: bool,
            },
        }

        let res = Raw::deserialize(deserializer)?;

        match res {
            Raw::Complex {
                attribute,
                size,
                write,
                ..
            } if attribute != RegisterAttribute::None => Ok(Register {
                code: 0,
                attribute,
                size,
                write,
            }),
            Raw::Complex {
                code,
                attribute,
                size,
                write,
            } => {
                if code.is_none() {
                    panic!(
                        "register.code must be present if register doesn't have a special attribute"
                    )
                }
                Ok(Register {
                    code: code.unwrap(),
                    attribute,
                    size,
                    write,
                })
            }
            Raw::Simple(code) => Ok(Register {
                code,
                ..Default::default()
            }),
        }
    }
}

#[derive(Deserialize, Debug, Default, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RegisterAttribute {
    #[default]
    None,
    Sp, // StackPointer
    Pc, // ProgramCounter
    Flags,
}

/// A ParserArg populated with a value, to be passed onto arg handlers and prettifier
#[derive(IntoPyObject, FromPyObject, Clone, Debug)]
pub struct PopulatedArg {
    #[pyo3(item)]
    pub t: ParserArgType,
    #[pyo3(item)]
    pub direction: ParserArgDirection,
    #[pyo3(item)]
    pub arg_val: Vec<u8>,
    #[pyo3(item)]
    pub memory_region: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ParserArg {
    pub t: ParserArgType,
    pub direction: ParserArgDirection,
    pub arg_size: u16,
}

impl ParserArg {
    pub fn from_string(s: String) -> Result<Self> {
        let mut t: ParserArgType = ParserArgType::None;
        let mut d: ParserArgDirection = ParserArgDirection::None;
        let mut num_start = 0;
        for (i, c) in s.chars().enumerate() {
            if c.is_numeric() {
                num_start = i;
                break;
            }
            (t, d) = match c {
                'r' => (ParserArgType::Register, d),
                'm' => (ParserArgType::Memory, d),
                'i' => (ParserArgType::Immediate, d),
                's' => (t, ParserArgDirection::Source),
                'd' => (t, ParserArgDirection::Destination),
                _ => {
                    return Err(anyhow!(
                        "Unrecognized char ({c}) when deserializing argstring ({s})"
                    ));
                }
            };
        }

        if num_start == 0 {
            return Err(anyhow!("Address size cannot be at position 0: {s}"));
        };

        if t == ParserArgType::None || d == ParserArgDirection::None {
            return Err(anyhow!("Argtype and direction must be specified: {s}"));
        }

        return Ok(Self {
            t,
            direction: d,
            arg_size: s[num_start..].parse::<u16>()?,
        });
    }

    pub fn as_string(&self) -> String {
        let mut res: String = String::new();
        res.push(match self.t {
            ParserArgType::Register => 'r',
            ParserArgType::Immediate => 'i',
            _ => 'm',
        });
        res.push(match self.direction {
            ParserArgDirection::Destination => 'd',
            _ => 's',
        });
        res + &self.arg_size.to_string()
    }

    /// Populate the argument
    pub fn populate(&self, buf: &Bytes, offset: usize) -> Result<PopulatedArg> {
        if offset + self.arg_size as usize > buf.remaining() {
            return Err(anyhow!("Out of bounds population"));
        }

        let mut b = Vec::with_capacity(self.arg_size as usize);
        b.extend_from_slice(&buf[offset..(offset + self.arg_size as usize)]);
        Ok(PopulatedArg {
            t: self.t,
            direction: self.direction,
            arg_val: b,
            memory_region: None,
        })
    }
}

#[pyclass(eq, eq_int, from_py_object, name = "ArgType")]
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum ParserArgType {
    None,
    Register,
    Memory,
    Immediate,
    //Other(String) //TODO: Add ability to define custom ParserArgTypes
}
#[pyclass(eq, eq_int, from_py_object, name = "ArgDirection")]
#[derive(PartialEq, Clone, Debug, Copy)]
pub enum ParserArgDirection {
    None,
    Source,
    Destination,
}

pub struct DisFormatter {
    pub lines: Vec<DisFormatterLine>,
    pub config: FormattingConfig,
}

#[pyclass(from_py_object, name = "FormatterLine")]
#[derive(Clone, Default)]
pub struct DisFormatterLine {
    pub hex: String,
    pub opcode: String,
    pub operands: Vec<String>,
}

#[serde_as]
#[derive(Deserialize, Debug, Clone)]
pub struct FormattingConfig {
    // hard to implement without uglifying code more
    // pydefine.rs:21
    //#[serde_as(as = "DefaultOnNull")]
    //pub operator_lowercase: bool,
    #[serde_as(as = "DefaultOnNull")]
    #[serde(default = "default_instruction_format")]
    pub instruction_format: String,

    #[serde_as(as = "DefaultOnNull")]
    #[serde(default = "default_operand_separator")]
    pub operand_separator: String,

    #[serde_as(as = "DefaultOnNull")]
    #[serde(default = "default_true")]
    pub include_hex: bool,
    // realized that it's completely unnecessary to use tabs, might bring this back but probably not...
    //#[serde_as(as = "DefaultOnNull")]
    //pub pad_with_tabs: bool,
}

impl Default for FormattingConfig {
    fn default() -> Self {
        Self {
            instruction_format: default_instruction_format(),
            operand_separator: default_operand_separator(),
            include_hex: true,
        }
    }
}

fn default_instruction_format() -> String {
    "{} {}".to_owned()
}

fn default_operand_separator() -> String {
    ",".to_owned()
}

fn deserialize_memory_permissions<'de, D>(d: D) -> Result<MemoryLayoutPermissions, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<String> = Option::<String>::deserialize(d)?;
    if opt.is_none() {
        return Ok(MemoryLayoutPermissions::default());
    }

    let perm_str = opt.unwrap();
    let mut obj = MemoryLayoutPermissions {
        read: false,
        write: false,
        execute: false,
    };
    for c in perm_str.chars() {
        match c {
            'r' => obj.read = true,
            'x' => obj.execute = true,
            'w' => obj.write = true,
            _ => panic!(
                "Non-permission char:  {}\nValid chars are 'w', 'r', and 'x'",
                c
            ),
        }
    }
    Ok(obj)
}

pub fn deserialize_range_hashmap<'de, D>(
    d: D,
) -> Result<HashMap<Range<usize>, MemoryLayout>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<HashMap<String, MemoryLayout>> =
        Option::<HashMap<String, MemoryLayout>>::deserialize(d)?;
    if opt.is_none() {
        return Ok(HashMap::default());
    }

    let map = opt.unwrap();
    let mut res = HashMap::with_capacity(map.len());

    for (key, val) in map {
        let parts: Vec<usize> = key
            .split("..")
            .map(|x| x.parse::<usize>().unwrap())
            .collect();
        res.insert(
            Range {
                start: parts[0],
                end: *parts.get(1).unwrap_or(&usize::MAX),
            },
            val,
        );
    }
    return Ok(res);
}

pub fn default_true() -> bool {
    true
}

pub fn default_1() -> u8 {
    1u8
}

/* -------------------------------------- */

#[pyclass(name = "emulator")]
pub struct Emulator {
    #[pyo3(get)]
    pub state: EmulatorState,

    // --- State --- //
    /*pub data: Bytes,

    #[pyo3(get)]
    pub memory: Vec<u8>,
    pub registers: HashMap<usize, EmuRegister>,
    pub pc: EmuRegister,
    pub sp: Option<EmuRegister>,
    pub flags: Option<EmuRegister>,

    // --- Status --- //
    #[pyo3(get)]
    pub halted: bool,
    #[pyo3(get)]
    pub paused: bool,
    #[pyo3(get)]
    pub started: bool,
*/
    // --- Config stuff --- //
    pub machine_config: MachineConfig,
    pub ops: Arc<OpMap>, // this allows us to access the ops without transferring ownership
    pub op_size: usize,

    // --- Runtime Stuff --- //
    pub watchpoints: HashMap<usize, EmuWatchpoint>,
    pub watchpoints_id: usize,
    pub breakpoints: HashMap<usize, EmuWatchpoint>,
    pub breakpoints_id: usize,
}

#[pyclass(from_py_object)]
#[derive(Clone, Debug)]
pub struct EmulatorState {
    // --- State --- //
    pub data: Bytes,

    #[pyo3(get, set)]
    pub memory: Vec<u8>,
    pub registers: HashMap<usize, EmuRegister>,
    #[pyo3(get)]
    pub pc: EmuRegister,
    #[pyo3(get)]
    pub sp: Option<EmuRegister>,
    #[pyo3(get)]
    pub flags: Option<EmuRegister>,

    // --- Status --- //
    #[pyo3(get)]
    pub halted: bool,
    #[pyo3(get)]
    pub paused: bool,
    /*#[pyo3(get)]
    pub started: bool,*/
}
#[derive(Debug, Default)]
pub struct EmuWatchpoint {
    pub address: usize,
    pub size: usize,
    pub callback: Option<Py<PyFunction>>,
    pub hit: usize,
    pub disabled: bool,
}

#[derive(Debug, Clone, IntoPyObject)]
pub struct EmuRegister {
    pub name: String,
    #[pyo3(into_py_with = bytes_into_pyobject)]
    pub data: Bytes,
    pub max_size: usize,
    pub write: bool,
}

fn bytes_into_pyobject<'py>(b: Cow<'_, Bytes>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyBytes::new(py, &b).into_any())
}

impl Default for EmuRegister {
    fn default() -> Self {
        EmuRegister {
            name: String::new(),
            data: Bytes::default(),
            max_size: 1usize,
            write: true,
        }
    }
}

pub struct Instruction {
    op_item: Arc<OpHandler>,
}
