#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
scripts/build-editor-grammar.sh
cd tree-sitter-etch
tree-sitter test
tree-sitter parse --quiet ../examples/*.etch ../examples/flight_computer/*.etch ../evaluator/std/*.etch
for query in ../.helix/runtime/queries/etch/*.scm; do
    tree-sitter query "$query" ../examples/voltage_divider.etch >/dev/null
done
printf '%s\n' 'Editor grammar, corpus, examples, and queries verified.'
