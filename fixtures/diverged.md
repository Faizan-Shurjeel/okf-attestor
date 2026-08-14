---
type: Attested Computation
title: Diverged fixture
runtime: okf:wasm@1
computation: artifacts/constant-42.wasm
parameters: []
okf_attestor:
  protocol: 1
  input: artifacts/input.json
  expected_output: artifacts/expected-43.txt
---

# Computation

The module returns `42`, while this receipt claims `43`.
