# by default the args handler is the one you registered as the default
# you can specify a custom one with the "args_handler" parameter
#
# This declaration in itself is enough to disassemble the whole program. 
# In case you do not want any emulation you can simply set the function to "pass" or any other gibberisch (just know that it won't emulate the instruction!)
@op(code=0x0, args=["rs64"])
def push():
    pass
# a custom name can be passed with the name parameter
@op(code=0x1, args=["rs64"], name="pop")
def pop():
    pass

@op(code=0x2, args=["rd64","rs64"])
def mov():
    pass
