A rust based language for circuit design, simulation, and testing

lots of realy smart things have been developed in the software world like automated testing, mocking and functions. What if we could apply these concepts to circuit design? this is the motivation behind etch.

proposed DSL
```js

module VoltageDivider (r_a: Number, r_b: Number) {
  let { a: V_IN, b:V_MID } = Resistor(r_a)
  let { b: V_OUT } = Resistor(r_b) { a: V_MID }
  
  return {
    V_MID,
    V_IN,
    V_OUT
  };
}

module XOR {
  let { x, y, a  } = AND;
  let { z: b} = OR { x, y };
  let { z } = OR { a, b };
  
  return {
    x, y, z
  }
}

```


I want to investigate pipes for when small values tho

CURRENT PLAN:
- Parser: Code -> AST
- Compiler: AST -> Node Structure
- Simulator: Simulates Node Structures

## CLI

Run the command through Cargo during development:

```sh
cargo run -p cli -- check examples/stdlib/voltage_divider.etch
cargo run -p cli -- run examples/stdlib/voltage_divider.etch
cargo run -p cli -- simulate examples/stdlib/voltage_divider.etch --steps 10 --delta-time 0.001
cargo run -p cli -- test examples/stdlib/voltage_divider.etch
cargo run -p cli -- schematic examples/stdlib/sectioned_system.etch --output schematic.svg
cargo run -p cli -- display examples/stdlib/rc_response.etch --output-dir displays
```

The installed binary is named `etch`, so the equivalent installed commands use
`etch check`, `etch test`, and so on. Imports are resolved relative to the main
file's directory; bundled `std/*.txt` imports are always available.
