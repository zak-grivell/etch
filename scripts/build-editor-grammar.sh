#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
(cd tree-sitter-etch && tree-sitter generate)
mkdir -p .helix/runtime/grammars
cc -O2 -fPIC -shared -I tree-sitter-etch/src tree-sitter-etch/src/parser.c -o .helix/runtime/grammars/etch.so
printf '%s\n' 'Grammar built. Start Helix with: HELIX_RUNTIME="$PWD/.helix/runtime" hx'
