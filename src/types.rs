use anyhow::{Result, anyhow};
use bytes::{Buf, Bytes};
use pyo3::{FromPyObject, IntoPyObject, Py, pyclass, types::PyFunction};
use serde::{Deserialize, Deserializer};
use serde_with::{DefaultOnNull, serde_as};
use std::{collections::HashMap, ops::Range, path::PathBuf, sync::Mutex};

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
    pub ops: OpMap,
    pub arg_handler: Option<ArgHandler>,
    pub arg_formatter: Option<ArgFormatter>,
    pub disassembler: Option<Disassembler>, //pub primary_args_handler: String,
}

#[derive(Debug)]
pub struct ArgHandler(pub Py<PyFunction>);

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
#[derive(Deserialize, Debug)]
pub struct MachineConfig {
    #[serde_as(as = "DefaultOnNull<HashMap<_, DefaultOnNull<_>>>")]
    pub registers: HashMap<String, Register>,

    pub memory_size: Option<usize>, // upper size limit for memory vector, None just lets it grow continuously

    #[serde(default)]
    #[serde(deserialize_with = "deserialize_range_hashmap")]
    pub memory_layout: HashMap<String, Range<usize>>,
    pub instruction: InstructionConfig,

    #[serde_as(as = "DefaultOnNull")]
    #[serde(default = "default_true")]
    pub little_endian: bool,

    pub implementation: Option<PathBuf>,
}

#[derive(Deserialize, Debug)]
pub struct InstructionConfig {
    pub op_size: Option<usize>, // if None custom decoder will be enforced
}

#[derive(Deserialize, Debug, Default)]
#[serde_as]
pub struct Register {
    #[serde(default)]
    #[serde_as(as = "DefaultOnNull")]
    pub attribute: RegisterAttribute,

    #[serde(default)]
    #[serde_as(as = "DefaultOnNull")]
    pub size: u8, // size in bits, max is sizeof(usize)
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum RegisterAttribute {
    #[default]
    None,
    Sp, // StackPointer
    Pc, // ProgramCounter
    Flags,
}

pub fn deserialize_range_hashmap<'de, D>(d: D) -> Result<HashMap<String, Range<usize>>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt: Option<HashMap<String, String>> = Option::<HashMap<String, String>>::deserialize(d)?;
    if opt.is_none() {
        return Ok(HashMap::default());
    }

    let map = opt.unwrap();
    let mut res = HashMap::with_capacity(map.len());

    for (key, val) in map {
        let parts: Vec<usize> = val
            .split("..")
            .map(|x| x.parse::<usize>().unwrap())
            .collect();
        res.insert(
            key,
            Range {
                start: parts[0],
                end: *parts.get(1).unwrap_or(&usize::MAX),
            },
        );
    }
    return Ok(res);
}

pub fn default_true() -> bool {
    true
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
