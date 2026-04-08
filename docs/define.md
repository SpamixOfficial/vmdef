# vmdef.define

# Functions
## init() -> Define

Create a new `define` class

---
This function takes no parameters

## @define.op()

Bind function to opcode with `code` and operators `args`

---

| parameter       | doc                         | optional | used by     |
|-----------------|-----------------------------|----------|-------------|
| code            | Opcode                      | No       | all         |
| args            | Opcode args                 | Yes      | all         |
| rad             | RAD eval                    | Yes      | disassembly |
| name            | Opcode name override        | Yes      | disassembly |
| args_preprocess | Function to preprocess args | Yes      | all         |

All opcode `args` should begin with flags and should always end with a size (numeric format)

**Flags:**
- 'r' -> The argument is of the type Register
- 'i' -> The argument is of the type Immediate
- 'm' -> The argument is of the type Memory
- 's' -> The argument is a source
- 'd' -> The argument is a destination

The size specified should be in bytes. 

For example, `rd2` would be a **destination register** with a size of 2 bytes. Data which fits these 3 constraints could be `0x0200` which would be parsed to (*if little endian*) **register 2**

The `rad` parameter allows you to specify an eval to run when the opcode is reached. After populating all the arguments the RAD statement is ran. In this statement you can modify the internal register lookup table, which you can utilize later when writing your argument prettifier!

The `args_preprocess` parameter allows you to specify a function to pre-process arguments passed to the opcode. This is useful if you know this specific opcode reads eg. register indexes or memory addresses in a way that differs from the rest of your implementation.

For an example of all of these parameters being used, consult the example!

## @define.arg_formatter()

Mark this function as the argument formatter to be used by disassembly

---
This function does not take any arguments
