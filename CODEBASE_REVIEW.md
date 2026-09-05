# Full codebase review — 5 September 2026

## Scope and method

Reviewed all 84 tracked paths across the 13 workspace crates, bundled standard library, examples, documentation, manifests, Nix configuration, hidden editor settings, and lockfiles. All handwritten files were read; generated lockfile contents were checked structurally. `parser/src/cleaner.rs` was reviewed and removed as redundant. Build outputs, downloaded dependencies, Git internals, and ignored local files are outside this source review. No dependency vulnerability database scan or fabrication qualification was performed.

This report covers the cleanup already in the working tree and the subsequent complete file review. Full coverage does not mean all possible defects have been eliminated. The original findings and their implemented follow-ups are distinguished below.

## Implemented fixes

- Parser: interleaved calls and field access now work (`make().value().answer`); initializers resolve a previous shadowed binding while direct recursive lambdas still work; function-type parameters no longer escape into value scope; numeric aliases respect blocks; match bindings receive the scrutinee type; early block returns determine the result type; definition statements have the same no-value type as runtime; object merges produce merged fields with right-hand overrides; SI prefixes are not counted twice in token diagnostics.
- Evaluator: removed unreachable arithmetic branches, simplified block scope handling, made equality respect units and circuit identity, checked unexpected native arguments, rejected invalid simulation time and oversized step counts, and rejected conflicting fixed voltages both on one node and across connected nodes.
- Circuit IR: connection/component/label APIs register nodes; repeated net labels connect their nodes; component sections are registered; shared section-name collection removes duplicates and includes implicit sections.
- KiCad: diode and LED anode/cathode mappings corrected; symbol-to-page Y coordinates corrected; missing/empty/duplicate pin mappings fail with an error instead of indexing panics; loaded symbol pins validated before placement; duplicate export validation removed.
- PCB: configuration validated at the public layout boundary; router stays inside board bounds; cross-layer route joins target actual pads; placement candidate spacing agrees with its collision checks; unnecessary cloning removed.
- SVG schematic: unnamed cross-section nets receive visible generated labels; generic ports use the actual generic body geometry; overlapping-component movement preserves adjusted pin positions; vertical component hitboxes use rotated dimensions; reference numbering groups by prefix.
- Editor and graphs: multiline semantic tokens are retained; invalid UTF-16 positions are rejected; hover spans use exclusive ends; highlighting recognizes supported compound operators; invalid graph samples leave a gap instead of connecting through missing data.
- CLI and cleanup: main files without an `.etch` extension work; legacy export commands share validation; display filename collisions preserve every result; removed the identity-only parser cleaner, obsolete commented code, and macro boilerplate; documented current `check` and PCB export behavior.

## Follow-up — all 18 findings addressed

The requested follow-up is implemented. The multi-unit item uses the explicit
rejection option from the original review; support for additional units is not
claimed. No finding is left as an unimplemented follow-up.

| Original priority | Finding | Resolution and evidence |
|---|---|---|
| P1 | Physical footprint geometry | Native footprint loader feeds placement, routing, and export; pad shapes/drills/layers are retained. Export ordering/rotation matches native KiCad. Divider, LED, and shared-node fixtures pass connectivity and DRC checks. |
| P1 | Multiple ports sharing one node | Schematic positions are keyed by port name; PCB pads are distinct records retaining node identity. Unit regressions and the shared-node KiCad fixture preserve both pads and connect them. |
| P1 | Multi-unit symbols | Only common pins and normal-style unit 1 pins are selectable. Mappings from other units/styles fail before placement. A multi-unit library fixture tests this boundary. |
| P1 | Solver convergence | Pivoted linear solve, floating/ill-conditioned-net checks, finite-equation validation, explicit absolute/relative tolerances, and a bounded nonlinear loop. Analytic resistor-ladder, oscillating, and floating-net regressions pass. Bundled reactive components use implicit companion equations. |
| P2 | Literal units and type coverage | Quantity AST nodes retain units; named types resolve all supported type forms. Directional compatibility handles objects, arrays/tuples, optionals, unions, and function types. Type names cannot be used as values. Positive/negative regressions cover these forms. |
| P2 | Retained evaluator cycles | Closure snapshots exclude their own binding; recursive calls inject an explicit self binding. Internal node/native handles are weak. Tests repeatedly release circuit and closure weak probes, preserve escaping closures, and call aliased recursive functions. |
| P2 | Cross-circuit nodes | Voltage, wire, connection, native node arguments, and symbol-node boundaries validate circuit identity. Public voltage/connection queries return errors. Foreign-node regression verifies no mutation. |
| P2 | Simulation partial state | The entire simulation call is transactional. Hook, floating-net, and non-convergence failures restore voltages, time, pending state, and design state. Regression also covers a failure after an earlier successful step. |
| P2 | Check side effects | Independent module checker follows imports and validates exported names/types without evaluation. Assertions deliberately failing in imported/main modules do not execute during `check`. |
| P2 | Source loading and diagnostics | Filesystem provider reads only reachable sources, normalizes paths, preserves I/O errors, and carries compiler diagnostics across the evaluator boundary. Tests include an unrelated invalid UTF-8 file and imported type errors. |
| P2 | Strings | Lexer decodes quote, backslash, newline, carriage-return, tab, and NUL escapes; unknown escapes fail. Lexer/evaluator/highlighter and grammar fixtures use matching escape behavior. |
| P2 | Symbol inheritance | Derived properties replace base properties by name while retaining geometry and other base fields. A derived-symbol regression verifies footprint and value overrides. |
| P2 | Editor update ordering | Updates and publication are serialized with document versions; stale versions and changes to closed documents are ignored. Async regression checks stale writes and late changes after close. |
| P2 | Dense schematic routing | Alignment/compaction are section-local. Candidate wire trees are checked against other nets; incomplete/conflicting trees use explicit endpoint labels without partial wire output. Crossing and pin-identity regressions cover this behavior. |
| P2 | Export references | One shared reference assignment uses symbol-library reference prefixes for both outputs, before physical filtering. KiCad fixtures verify matching references and pin nets in schematic and board. |
| P2 | Extreme graph values | Scaled bounds avoid overflow, tick interpolation stays finite, and legends wrap with an expanding canvas. Tests include positive/negative `f64::MAX`, constant extremes, and 20 long names. |
| P3 | Repeated trace model | Both evaluator and renderer re-export `circuit_ir::TraceSeries`; CLI conversion boilerplate is removed. |
| P3 | Editor grammar fallback | Real Etch grammar replaces JavaScript. Generated parser builds, corpus passes, every example/library source parses, and all Helix queries compile. |

## Validation

- Workspace regression suite: all 94 tests passed (`cargo test --workspace`).
- Clippy passed with warnings denied (`cargo clippy --workspace --all-targets -- -D warnings`); formatting (`cargo fmt --all -- --check`) and whitespace (`git diff --check`) validation passed.
- `scripts/verify-kicad.py`: divider, LED, and shared-node fixtures each have **zero DRC violations and zero unconnected items**, with expected schematic pin connectivity. Tested with installed KiCad 10.0.5. Local fontconfig warnings do not affect successful export/DRC results.
- `scripts/verify-editor.sh`: corpus, all example and bundled-library sources, native grammar build, and all three Helix query files pass.

These checks establish the listed behaviors and fixtures, not that arbitrary
circuits are fabrication-qualified. KiCad export rejects incomplete routing;
additional symbol units remain an explicit unsupported-input error. Public API
changes and simulation/type semantics are documented in README.

Grammar implementation references: [Tree-sitter grammar DSL](https://tree-sitter.github.io/tree-sitter/creating-parsers/2-the-grammar-dsl.html), [Helix language configuration](https://docs.helix-editor.com/languages.html).

## File coverage

Checked means reviewed, not necessarily modified. Lockfiles have generated/structural review status.

- [x] `.envrc` — reviewed
- [x] `.gitignore` — reviewed
- [x] `.helix/languages.toml` — reviewed
- [x] `.helix/runtime/queries/etch/highlights.scm` — reviewed
- [x] `.helix/runtime/queries/etch/indents.scm` — reviewed
- [x] `.helix/runtime/queries/etch/textobjects.scm` — reviewed
- [x] `Cargo.lock` — generated dependency data reviewed
- [x] `Cargo.toml` — reviewed
- [x] `README.md` — reviewed
- [x] `ast/Cargo.toml` — reviewed
- [x] `ast/src/ast.rs` — reviewed
- [x] `ast/src/expression.rs` — reviewed
- [x] `ast/src/lib.rs` — reviewed
- [x] `ast/src/pattern.rs` — reviewed
- [x] `ast/src/primative.rs` — reviewed
- [x] `ast/src/program.rs` — reviewed
- [x] `ast/src/result.rs` — reviewed
- [x] `ast/src/statement.rs` — reviewed
- [x] `ast/src/types.rs` — reviewed
- [x] `ast_macros/Cargo.toml` — reviewed
- [x] `ast_macros/src/lib.rs` — reviewed
- [x] `circuit_ir/Cargo.toml` — reviewed
- [x] `circuit_ir/src/connectivity.rs` — reviewed
- [x] `circuit_ir/src/lib.rs` — reviewed
- [x] `circuit_ir/src/model.rs` — reviewed
- [x] `cli/Cargo.toml` — reviewed
- [x] `cli/src/lib.rs` — reviewed
- [x] `cli/src/main.rs` — reviewed
- [x] `cli/src/test.rs` — reviewed
- [x] `etch_lsp/Cargo.toml` — reviewed
- [x] `etch_lsp/src/main.rs` — reviewed
- [x] `evaluator/Cargo.toml` — reviewed
- [x] `evaluator/src/circuit.rs` — reviewed
- [x] `evaluator/src/lib.rs` — reviewed
- [x] `evaluator/src/native.rs` — reviewed
- [x] `evaluator/src/test.rs` — reviewed
- [x] `evaluator/src/value.rs` — reviewed
- [x] `evaluator/std/analog.etch` — reviewed
- [x] `evaluator/std/digital.etch` — reviewed
- [x] `evaluator/std/passive.etch` — reviewed
- [x] `evaluator/std/sources.etch` — reviewed
- [x] `examples/README.md` — reviewed
- [x] `examples/complex_mixed_signal.etch` — reviewed
- [x] `examples/d_flip_flop.etch` — reviewed
- [x] `examples/flight_computer/README.md` — reviewed
- [x] `examples/flight_computer/components.etch` — reviewed
- [x] `examples/flight_computer/main.etch` — reviewed
- [x] `examples/logic_gates.etch` — reviewed
- [x] `examples/op_amp.etch` — reviewed
- [x] `examples/pcb_voltage_divider.etch` — reviewed
- [x] `examples/rc_response.etch` — reviewed
- [x] `examples/sectioned_system.etch` — reviewed
- [x] `examples/voltage_divider.etch` — reviewed
- [x] `flake.lock` — generated dependency data reviewed
- [x] `flake.nix` — reviewed
- [x] `graph_render/Cargo.toml` — reviewed
- [x] `graph_render/src/lib.rs` — reviewed
- [x] `highlighting/Cargo.toml` — reviewed
- [x] `highlighting/src/lib.rs` — reviewed
- [x] `kicad/Cargo.toml` — reviewed
- [x] `kicad/src/common.rs` — reviewed
- [x] `kicad/src/lib.rs` — reviewed
- [x] `kicad/src/pcb.rs` — reviewed
- [x] `kicad/src/schematic.rs` — reviewed
- [x] `kicad/src/symbol_library.rs` — reviewed
- [x] `parser/Cargo.toml` — reviewed
- [x] `parser/src/cleaner.rs` — reviewed; redundant pass removed
- [x] `parser/src/lexer.rs` — reviewed
- [x] `parser/src/lib.rs` — reviewed
- [x] `parser/src/parser.rs` — reviewed
- [x] `parser/src/resolver.rs` — reviewed
- [x] `parser/src/semantic.rs` — reviewed
- [x] `parser/src/test.rs` — reviewed
- [x] `pcb/Cargo.toml` — reviewed
- [x] `pcb/src/lib.rs` — reviewed
- [x] `render_utils/Cargo.toml` — reviewed
- [x] `render_utils/src/lib.rs` — reviewed
- [x] `schematic/Cargo.toml` — reviewed
- [x] `schematic/src/drawing.rs` — reviewed
- [x] `schematic/src/lib.rs` — reviewed
- [x] `schematic/src/placement.rs` — reviewed
- [x] `schematic/src/routing.rs` — reviewed
- [x] `schematic/src/tests.rs` — reviewed
- [x] `schematic/src/topology.rs` — reviewed

## Added follow-up files

The implementation adds `circuit_ir/src/sexpr.rs`, `pcb/src/footprint.rs`,
`scripts/verify-kicad.py`, `scripts/build-editor-grammar.sh`,
`scripts/verify-editor.sh`, and the `tree-sitter-etch` grammar/corpus/generated
parser files. Generated parser files are validated through generation, native
compilation, corpus parsing, and query compilation.
