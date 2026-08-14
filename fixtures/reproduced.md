---
type: Attested Computation
title: Reproduced fixture
runtime: okf:wasm@1
computation: artifacts/constant-42.wasm
parameters: []
okf_attestor:
  protocol: 1
  input: artifacts/input.json
  expected_output: artifacts/expected-42.txt
---

# Computation

Returns the bytes `42` through the okf:wasm@1 ABI.
