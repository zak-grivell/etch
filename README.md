A Rust-based language for circuit design, simulation, and testing

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

Etch brings functions, simulation, and automated testing to circuit design.

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

## Checking, types, and simulation

`check` resolves and type-checks reachable imports without executing assertions,
simulations, or other top-level expressions. Source-read failures and compiler
messages include the affected file and byte span. Import paths normalize `.` and
`..` relative to the main file's directory.

Numeric literals retain unit labels (`2mV + 3mV` produces `0.005 V`). Named aliases
support numeric, object, array, tuple, optional, union, and function types. Object
arguments may contain additional fields; array literals are checked positionally
against tuples or element-by-element against array types. Optional types accept
the inner type or a no-value result. Scalars remain accepted by numeric unit
parameters for compatibility with existing component constructors.

Strings support `\"`, `\\`, `\n`, `\r`, `\t`, and `\0`. Unknown escapes are errors.

Simulation solves each linearized circuit with pivoting and checks voltage
convergence with absolute tolerance `1e-9` plus relative tolerance `1e-7`.
Nonlinear models have a limit of 128 iterations per step. Active floating or
ill-conditioned nets, invalid equations, and non-convergence return errors.
Nodes absent from all equations retain their prior voltage. Capacitor and
inductor models use backward-Euler companion equations. A failed `simulate` call
restores the complete state from before that call, including earlier steps in
the same call.

For Rust consumers, `Circuit::voltage` and `NodeValue::connections` now return
`Result`. Nodes from another circuit are rejected. Closures capture immutable
scope snapshots with explicit recursive bindings; internal circuit handles are
weak, while externally returned nodes keep their circuit alive.

## KiCad libraries and validation

PCB placement and routing use the installed footprint's physical pad positions,
sizes, and courtyard/body bounds. Export retains native pad shapes, drills,
layers, and footprint graphics. Set `KICAD_FOOTPRINT_DIR` and `KICAD_SYMBOL_DIR`
when libraries are outside the standard KiCad installation paths. Missing
footprints or mapped pads produce errors. SVG remains available for inspecting
an incomplete route; KiCad PCB export rejects unrouted connections.

References come from the symbol library and are assigned once for both exports.
Symbol property inheritance is preserved. Schematic export supports common pins
and unit 1 in the normal symbol style; mappings to other units/styles are rejected
with an explanatory error instead of producing disconnected labels.

Schematic wiring falls back to explicit net labels when a route would overlap
another net or cannot be completed. Placement adjustments stay within sections.

Run the regression checks in the Nix development environment:

```sh
nix develop --command cargo test --workspace
nix develop --command cargo clippy --workspace --all-targets -- -D warnings
nix develop --command python3 scripts/verify-kicad.py
nix develop --command scripts/verify-editor.sh
```

The KiCad check requires `kicad-cli` (or `KICAD_CLI`) and installed libraries. It
checks exported schematic connectivity and DRC for a divider, an LED circuit,
and a component with two ports sharing one node. DRC validates these fixtures
using the installed KiCad rules; other boards still need their own project rules
and validation.

## Helix grammar

The repository includes an Etch Tree-sitter grammar, generated parser, corpus,
and highlighting/indent/text-object queries. From the repository root:

```sh
nix develop --command scripts/build-editor-grammar.sh
HELIX_RUNTIME="$PWD/.helix/runtime" hx
```

The compiled grammar is a local build artifact. The verification script checks
all examples and standard-library sources and compiles each editor query.
