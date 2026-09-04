# Standard-library examples

These programs demonstrate the bundled `std/*.etch` components and double as
executable regression tests. Each example registers one or more language tests
with `test(name:, body:)`.

- `voltage_divider.etch` demonstrates voltage sources and resistors.
- `rc_response.etch` demonstrates capacitor state and transient simulation.
- `op_amp.etch` demonstrates an equation-driven analog component.
- `logic_gates.etch` demonstrates combinational digital logic.
- `d_flip_flop.etch` demonstrates timestep state and sequential logic.
- `sectioned_system.etch` demonstrates schematic sections and cross-section net labels.
- `complex_mixed_signal.etch` stress-tests several large anchors, passive
  waterfall branches, shared power rails, fan-out, and ground returns.
- `flight_computer/main.etch` is a large board-level STM32F407 flight-computer
  example with LoRa, GPS, RS-485, and three I2C sensors.

The evaluator test suite loads every example and requires all registered tests
to pass.

Every circuit has a shared `ground` node fixed at 0 V. Connect components
directly with expressions such as `resistor.b <- ground`; no `Ground()`
component needs to be created or imported.

Examples explicitly select their root component with `render(component: ...)`.
A bare final expression is still evaluated normally, but is not treated as the
component to render.

`sectioned_system.etch` also produces a section-aware schematic. After
evaluation, hosts can retrieve the complete SVG with
`output.schematic_svg()`. Components register symbols with `use_symbol(...)`,
while matching `net(label:, node:)` calls connect and label nets across section
boundaries.

`rc_response.etch` registers a transient graph with `display(...)`. Hosts run
registered displays with `output.run_displays()` and receive both raw sampled
trace data and a complete oscilloscope-style SVG graph.
