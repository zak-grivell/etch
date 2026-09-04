A rust based language for circuit design, simulation, and testing

## Architecture

The compiler and output backends communicate through the renderer-independent
`circuit_ir::CircuitDesign` model:

- `parser` compiles Etch source into a typed AST.
- `highlighting` provides tolerant, editor-independent syntax highlighting.
- `etch_lsp` serves compiler diagnostics and semantic highlighting over LSP.
- `evaluator` evaluates the AST, simulates behavior, and produces `CircuitDesign`.
- `schematic` owns logical placement and SVG rendering; `pcb` owns physical
  placement/routing and SVG rendering. `kicad` serializes those same resolved
  layouts into editable KiCad files, and `graph_render` handles plots.
- `cli` is the composition layer that selects an output crate.

Output crates do not depend on the evaluator, and the evaluator does not depend
on an output format or renderer.

lots of realy smart things have been developed in the software world like automated testing, mocking and functions. What if we could apply these concepts to circuit design? this is the motivation behind etch.

Example
```etch
from "std/sources.etch" import { VoltageSource };
from "std/passive.etch" import { Resistor };

let supply = VoltageSource(value: 5);
let upper = Resistor(resistance: 1000);
let lower = Resistor(resistance: 1000);

supply.positive <- upper.a;
upper.b <- lower.a;
lower.b <- ground;

test(name: "equal resistors divide voltage in half", body: () -> {
    simulate(steps: 1, delta_time: 0.001);

    assert_close(
        actual: voltage(node: upper.b),
        expected: 2.5,
        tolerance: 0.000001,
    )
});

render(component: { input: upper.a, output: upper.b, ground: lower.b })
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
cargo run -p cli -- export schematic examples/sectioned_system.etch --format svg --output schematic.svg
cargo run -p cli -- export schematic examples/pcb_voltage_divider.etch --format kicad --output board.kicad_sch
cargo run -p cli -- export pcb examples/pcb_voltage_divider.etch --format svg --output pcb.svg
cargo run -p cli -- export pcb examples/pcb_voltage_divider.etch --format kicad --output board.kicad_pcb
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

```etch
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

```etch
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
file's directory; bundled `std/*.etch` imports are always available.

Both formats consume the same resolved layout: `export schematic` shares
component placement between SVG and KiCad, while `export pcb` shares component,
pad, net, and trace geometry between SVG and KiCad.

Select the circuit component intended for rendering explicitly:

```etch
render(component: { input: upper.a, output: upper.b, ground })
```

Bare file-level expressions are ordinary evaluated values and do not implicitly
become the render root.
