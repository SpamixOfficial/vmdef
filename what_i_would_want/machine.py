from vmdef import define


d = define.init()

# by default the args handler is the one you registered as the default
# you can specify a custom one with the "arg_handler" parameter
#
# This declaration in itself is enough to disassemble the whole program. 
# In case you do not want any emulation you can simply set the function to "pass" or any other gibberisch (just know that it won't emulate the instruction!)
@d.op(code=0x0, args=["rs1"])
def push(args):
    pass
# a custom name can be passed with the name parameter
@d.op(code=0x1, args=["rd1"], name="pop")
def pop(args):
    pass
# r = register
# d = destination
# s = source
# 1 = argument size (bytes)
# Any opcode with RAD specified will update the internal statemachine during disassembly
# the RAD parameter should be a valid eval, you can access your args through the `args` list
@d.op(code=0x2, args=["rd1","rs1"], rad="args[0]=args[1]")
def mov(args):
    pass

# i = immediate
# d = destination
# s = source
# 1 = argument size
# this will set rd to is, eg "movi a, 0x1" will set a to 0x1 in the statemachine
@d.op(code=0x3, args=["rd1","is1"], rad="args[0]=args[1]")
def movi(args):
    pass


# args will be the argspecifier from the args parameter above, but parsed by the library to give you a dictionary along with necessary data (check docs)
# should return a dictionary object like specified in the docs, emulator/RAD will process it accordingly
# buffer increments are not necessary since we already know the whole instruction length (cause you declared it correctly, right?)
@d.arg_handler
def arg_handler(args):
    pass

# this is only used during disassembly, and it is completely optional!
# NOTE: only one formatter can be defined, so make sure it is extensive!
# NOTE: Args is the same as args_handler, but should return a formatted string instead of a dictionary
@d.arg_formatter
def arg_prettifier(args, rad_state):
    res = []
    for arg in args:
        v = int.from_bytes(arg["value"])
        match arg["t"]:
            case ArgType.Register:
                res.append(format_register(v))
            case ArgType.Literal:
                res.append(hex(v))
            case ArgType.Memory:
                r = format_register(v)
                rad_v = rad_state[r]
                if v&0x80 == 0:
                    if rad_v == 0xfe:
                        res.append(f"[mmi]")
                    elif rad_v == 0xff:
                        res.append(f"[mmo]")
                    else:
                        res.append(f"[{r}]")
                else:
                    res.append(f"prog[{r}]")
    return res


def format_register(reg_val):
    match reg_val:
        case 0:
            return "a"
        case 1:
            return "b"
        case 2:
            return "c"
        case 3:
            return "d"
        case 4:
            return "sp"


# this is the load function, feel free to add any other stuff you would like to do here, but it must always return you Define class!
def load():
    return d