# Standard-library examples

These programs demonstrate the bundled `std/*.txt` components and double as
executable regression tests. Each example registers one or more language tests
with `test(name:, body:)`.

- `voltage_divider.etch` demonstrates voltage sources and resistors.
- `rc_response.etch` demonstrates capacitor state and transient simulation.
- `op_amp.etch` demonstrates an equation-driven analog component.
- `logic_gates.etch` demonstrates combinational digital logic.
- `d_flip_flop.etch` demonstrates timestep state and sequential logic.
- `sectioned_system.etch` demonstrates schematic sections and cross-section net labels.

The evaluator test suite loads every example and requires all registered tests
to pass.

`sectioned_system.etch` also produces a section-aware schematic. After
evaluation, hosts can retrieve the complete SVG with
`output.schematic_svg()`. Components register symbols with `use_symbol(...)`,
while matching `net(label:, node:)` calls connect and label nets across section
boundaries.

`rc_response.etch` registers a transient graph with `display(...)`. Hosts run
registered displays with `output.run_displays()` and receive both raw sampled
trace data and a complete oscilloscope-style SVG graph.
