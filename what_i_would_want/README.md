# Example No.1

Hello!

This is the first example for vmDef, and it also works as a representation of what I'm aiming for.

As the project is incomplete, I've made the main file (solv.py) return before emulation starts, as that is not done.


Feel free to check out the rust source code as well, just be aware that it is currently a minor mess.


## How to test it

To test it you need to create a virtual environment and run the library in development mode

This is mainly because it is not ready for distribution just yet.

This is a nice oneliner to do all this:

```sh
python3 -m venv venv && source venv/bin/activate && pip install maturin
```

Voila, you have everything installed!

Now, I will assume you're in **this** folder. Simply run this command to build the library and test out the example script:
```sh
cd .. && maturin develop && cd what_i_would_want && python3 solv.py 
```
