# you should not HAVE to use a solve script to interact with it, a CLI should also be available
from vmdef import machine

def callback_1(em):
    # do something...
    a = 1 + em.get_register("a")
    print(a)
    em.unpause() # will check for em.halted by itself. Only reason we do it in the loop down below is to check if we should stop the loop


data = open("vmdata.bin", "rb").read()

# by default your implementation file is in machine.json, but it is overridable through init
m = machine.init(d="machine.json")#, i="machine.py")

print(m.disassemble(data=data))

# you are also able to set init state such as registers and memory
# emulation will allocate and zero either config.initial_memory_size or 4096 bytes initially.
em = m.create_emulation(data=data)

# provide address (and callback, but that's optional, if no callback is provided you must unpause it manually after em.start() (if you dont do that in your callback))
em.set_breakpoint(0x100, callback_1)
# size is 1 byte by default
# (psst, this is a memory address, use breakpoints for data-space!)
em.set_watchpoint(0x1000, size=8)

em.start() # this is blocking
while True:
    if em.halted:
        break
    print(em.memory)
    em.unpause() # this is also blocking