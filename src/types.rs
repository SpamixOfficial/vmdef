use anyhow::{Result, anyhow};
use bytes::{Buf, Bytes};
use pyo3::{IntoPyObject, Py, pyclass, types::PyFunction};
use serde::{Deserialize, Deserializer};
use std::{
    collections::{BTreeMap, HashMap},
    ops::Range,
};

#[pyclass]
pub struct Machine {
    pub config: MachineConfig,
    pub ops: HashMap<usize, OpHandler>,
    pub arg_handlers: BTreeMap<String, ArgHandler>, // <Name,HandlerObj>
    pub arg_formatter: ArgFormatter,
    pub primary_args_handler: String,
}

pub struct ArgHandler(pub Py<PyFunction>);
pub struct ArgFormatter(pub Py<PyFunction>);

pub struct OpHandler {
    pub func: Py<PyFunction>,
    pub parser_args: Vec<ParserArg>,
    pub op_name: String,
    pub args_handler: Py<PyFunction>,
}

#[derive(Deserialize, Debug)]
pub struct MachineConfig {
    pub registers: HashMap<String, Register>,
    pub memory_size: Option<usize>, // upper size limit for memory vector, None just lets it grow continuously
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_range_hashmap")]
    pub memory_layout: HashMap<String, Range<usize>>,
    pub instruction: InstructionConfig,
    pub little_endian: bool
}

#[derive(Deserialize, Debug)]
pub struct InstructionConfig {
    pub op_size: Option<usize>, // if None custom decoder will be enforced
}

#[derive(Deserialize, Debug)]
pub struct Register {
    #[serde(default = "RegisterAttribute::no_attribute")]
    pub attribute: RegisterAttribute,
    pub size: u8, // size in bits, max is sizeof(usize)
}

#[derive(Deserialize, Debug)]
pub enum RegisterAttribute {
    None,
    Sp, // StackPointer
    Pc, // ProgramCounter
    Flags,
}

impl RegisterAttribute {
    fn no_attribute() -> Self {
        RegisterAttribute::None
    }
}

pub fn deserialize_range_hashmap<'de, D>(d: D) -> Result<HashMap<String, Range<usize>>, D::Error>
where
    D: Deserializer<'de>,
{
    let map: HashMap<String, String> = HashMap::deserialize(d)?;
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

/// A ParserArg populated with a value, to be passed onto arg handlers and prettifier
#[derive(IntoPyObject, Clone)]
pub struct PopulatedArg {
    pub t: ParserArgType,
    pub arg_val: Vec<u8>,
}

#[derive(Clone)]
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
    pub fn populate(&self, buf: &Bytes) -> PopulatedArg {
        let mut b = Vec::with_capacity(self.arg_size as usize);
        b.copy_from_slice(&buf[..(self.arg_size as usize)]);
        PopulatedArg { t: self.t, arg_val: b }
    }
}

#[pyclass(eq, eq_int, from_py_object, name = "ArgType")]
#[derive(PartialEq, Clone, Copy)]
pub enum ParserArgType {
    None,
    Register,
    Memory,
    Immediate,
    //Other(String) //TODO: Add ability to define custom ParserArgTypes
}
#[derive(PartialEq, Clone)]
pub enum ParserArgDirection {
    None,
    Source,
    Destination,
}
