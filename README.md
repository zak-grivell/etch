A rust based language for circuit design, simulation, and testing

## Architecture

The compiler and output backends communicate through the renderer-independent
`circuit_ir::CircuitDesign` model:

- `parser` compiles Etch source into a typed AST.
- `highlighting` provides tolerant, editor-independent syntax highlighting.
- `etch_lsp` serves compiler diagnostics and semantic highlighting over LSP.
- `evaluator` evaluates the AST, simulates behavior, and produces `CircuitDesign`.
- `schematic`, `pcb`, `kicad`, and `graph_render` are independent output crates.
- `cli` is the composition layer that selects an output crate.

Output crates do not depend on the evaluator, and the evaluator does not depend
on an output format or renderer.

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
cargo run -p cli -- check examples/voltage_divider.etch
cargo run -p cli -- run examples/voltage_divider.etch
cargo run -p cli -- simulate examples/voltage_divider.etch --steps 10 --delta-time 0.001
cargo run -p cli -- test examples/voltage_divider.etch
cargo run -p cli -- schematic examples/sectioned_system.etch --output schematic.svg
cargo run -p cli -- pcb examples/pcb_voltage_divider.etch --output pcb.svg
cargo run -p cli -- kicad-schematic examples/pcb_voltage_divider.etch --output board.kicad_sch
cargo run -p cli -- kicad-pcb examples/pcb_voltage_divider.etch --output board.kicad_pcb
cargo run -p cli -- display examples/rc_response.etch --output-dir displays
```

## Editor support

Build or run the language server over standard input/output:

```sh
cargo build -p etch_lsp
cargo run -p etch_lsp
```

Configure an editor LSP client to launch `target/debug/etch-lsp` for `*.etch`
files. The initial server supports full document synchronization, compiler
diagnostics, and full semantic tokens.

PCB generation starts with a board-level configuration in the circuit source:

```js
pcb_config(
    width: 30,
    height: 20,
    layers: 2,
    min_trace_width: 0.25,
    clearance: 0.2,
);
```

Dimensions are millimetres. The initial PCB backend automatically places
component symbols, groups connected pads into nets, and routes orthogonal traces
across the configured copper layers.

Components link to editable KiCad library parts through `use_symbol` metadata.
This works for user-defined components as well as the bundled standard library:

```js
use_symbol(
    kind: "resistor",
    ports: { a, b },
    kicad: {
        symbol: "Device:R",
        footprint: "Resistor_SMD:R_0603_1608Metric",
        pins: { a: "1", b: "2" },
    },
);
```

`symbol` and `footprint` use KiCad's `Library:Entry` identifiers. `pins` must
map every Etch port to its corresponding KiCad pin or pad number. A
schematic-only item such as a power symbol uses an empty footprint string.

The installed binary is named `etch`, so the equivalent installed commands use
`etch check`, `etch test`, and so on. Imports are resolved relative to the main
file's directory; bundled `std/*.txt` imports are always available.

Select the circuit component intended for rendering explicitly:

```js
render(component: { input: upper.a, output: upper.b, ground })
```

Bare file-level expressions are ordinary evaluated values and do not implicitly
become the render root.
