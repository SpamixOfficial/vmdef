# vmdef.machine

# Functions
## init() -> Machine

Create a new `machine` class

---

| parameter | doc                          | optional |
|-----------|------------------------------|----------|
| d         | Path to define json file     | No       |
| i         | Override implementation file | Yes      |
| verbose   | Output log messages          | Yes      |

## machine.disassemble() -> String

Disassemble provided data

---

| parameter | doc                                    | optional |
|-----------|----------------------------------------|----------|
| data      | Bytes object with data to be processed | No       |