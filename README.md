# OKF Attestor

Offline, fail-closed reproduction of [Open Knowledge Format (OKF) v0.2](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md) Attested Computations.

`okf-attestor` is a Rust library and CLI. It discovers every concept whose exact type is `Attested Computation`, executes only an explicitly supported deterministic WebAssembly profile, and emits one of three verdicts:

- **Reproduced** — deterministic execution completed and output matched exactly.
- **Diverged** — deterministic execution completed, but output differed.
- **Unattestable** — the runtime, contract, artifact, ABI, or execution could not satisfy the deterministic profile.

The verifier never fetches remote content and gives guest WebAssembly no network, filesystem, clock, randomness, environment, arguments, or host stdio capabilities.

> [!IMPORTANT]
> `Reproduced` means “internally reproducible from these exact bundle artifacts.” It does not prove who authorized or signed those artifacts. An editor able to replace the module and expected output together can create a newly reproducible bundle. Cryptographic identity and bundle notarization are intentionally separate concerns.

## Status

This repository implements an initial `0.1.0` profile and is not yet published. The package layout is already publication-ready:

- `okf-attestor-core` — reusable verification library.
- `okf-attestor` — installable CLI package.

## Try it

```console
cargo run -p okf-attestor -- verify fixtures
cargo run -p okf-attestor -- verify fixtures --concept reproduced
cargo run -p okf-attestor -- verify fixtures --format json
```

The full fixture bundle intentionally exits `1` because it contains a Diverged concept.

Once published:

```console
cargo install okf-attestor
okf-attestor verify path/to/bundle --format json
```

## CLI

```text
okf-attestor verify <BUNDLE_PATH> [--concept <ID>] [--format human|json]
```

Exit codes are stable:

| Code | Meaning |
|---:|---|
| `0` | Every selected concept is Reproduced, or the bundle has no Attested Computations. |
| `1` | At least one selected concept Diverged. |
| `2` | Nothing Diverged, but at least one selected concept is Unattestable. |
| `3` | Bundle/tool failure prevented a meaningful report. |

Divergence takes precedence over Unattestable when both occur.

JSON reports use `schema_version: 1`, one result per concept, stable snake-case verdict/reason values, SHA-256 artifact digests, and bounded UTF-8 expected/actual values when available.

## Library

```rust,no_run
use okf_attestor_core::{Verdict, verify_bundle};

let report = verify_bundle("path/to/bundle")?;
for result in report.results {
    match result.verdict {
        Verdict::Reproduced => println!("{} reproduced", result.concept),
        Verdict::Diverged => println!("{} diverged", result.concept),
        Verdict::Unattestable => println!("{} is unattestable", result.concept),
    }
}
# Ok::<(), okf_attestor_core::AttestorError>(())
```

Use `Verifier` when checking more than one bundle so its configured Wasmtime engine can be reused.

## Why an extension profile is necessary

Canonical OKF v0.2 defines `runtime`, parameter declarations, executor instructions, receipt **field names**, and an attester resource. It does not define persisted invocation values, a receipt wire format, expected output, or a portable executor/attester ABI. Generic BigQuery, dbt, Python, and similar concepts therefore cannot be safely re-executed offline from a bundle alone and are reported as Unattestable.

OKF permits producer extension keys. This project defines the following narrow profile.

## `okf:wasm@1` profile

An attestable concept has a file-backed local computation and an `okf_attestor` extension:

```yaml
---
type: Attested Computation
runtime: okf:wasm@1
computation: artifacts/calculation.wasm
parameters: []
okf_attestor:
  protocol: 1
  input: artifacts/input.json
  expected_output: artifacts/expected.json
---
```

Profile rules:

1. `runtime` must exactly equal `okf:wasm@1`.
2. `computation`, `input`, and `expected_output` are regular files resolved relative to the concept document.
3. Paths must stay inside the bundle. Absolute paths, URLs, parent traversal, and symbolic links are rejected.
4. Module, input, and expected bytes are read into memory with fixed size limits before execution.
5. Input and output are opaque bytes. Comparison is byte-for-byte; no newline or JSON normalization occurs.
6. The module must be a core WebAssembly binary with **zero imports**. It is hosted by Wasmtime but is deliberately not linked to WASI.

### Wasm ABI

The module exports:

```text
memory: WebAssembly linear memory
okf_attestor_alloc(input_len: i32) -> i32
okf_attestor_compute(input_ptr: i32, input_len: i32) -> i64
```

The host calls `okf_attestor_alloc`, writes input bytes to the returned memory address, then calls `okf_attestor_compute`. The returned `i64` packs the output region as:

```text
bits 63..32: unsigned output pointer
bits 31..0:  unsigned output length
```

A trap, invalid region, forbidden import, invalid ABI, or deterministic resource-limit exhaustion is Unattestable—not Diverged.

Current fixed limits:

- Wasm module: 4 MiB.
- Input, expected output, and actual output: 64 KiB each.
- Linear memory: 16 MiB.
- Execution fuel: 10,000,000 units.
- One instance and one linear memory per execution.
- SIMD, relaxed SIMD, memory64, multi-memory, tail calls, custom page sizes, and wide arithmetic disabled.
- NaN canonicalization enabled.

These values are part of the `okf:wasm@1` behavior and should not be loosened without a profile/version change and cross-platform fixtures.

## Security model

The strongest isolation property is structural: imported functions and memories are rejected before instantiation, and an empty linker is used. Core WebAssembly has no syscall instruction, so an import-free module has no route to host network, files, time, entropy, environment, or processes.

Fuel and store limits bound guest execution and runtime memory growth. Wasmtime validation/compilation still occurs in-process, so v0.1 does not claim strong denial-of-service isolation against arbitrarily hostile internet-sourced binaries. A future hardened mode may compile and execute in a resource-limited worker process.

Malformed bundle frontmatter is a tool-level error rather than being silently skipped. Unsupported runtimes are always Unattestable and are never executed.

## Development

Requires Rust `1.94` or newer (the MSRV of Wasmtime 47).

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p okf-attestor -- verify fixtures --format json
```

Before the first publication, validate the core package and inspect the CLI archive plan:

```console
cargo package -p okf-attestor-core
cargo publish --dry-run -p okf-attestor-core
cargo package -p okf-attestor --list
```

Publish `okf-attestor-core` first and wait for its `0.1.0` index entry. Cargo cannot fully package or dry-run the CLI before that registry dependency exists. Then run:

```console
cargo package -p okf-attestor
cargo publish --dry-run -p okf-attestor
cargo publish -p okf-attestor
```

## License

Apache-2.0.
