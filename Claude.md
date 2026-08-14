# CLAUDE.md — OKF Attestor

## What this project is

A Rust CLI + library that re-executes OKF v0.2 Attested Computation concepts inside a sandboxed, deterministic runtime and reports whether the result matches what the bundle claims. Three-state verdict: **Reproduced**, **Diverged**, **Unattestable**. No network calls, ever, during verification.

Full product context, goals, non-goals, and open questions: `PRD.md`. Read that before making any scope decision this file doesn't cover.

## Status

Initial `0.1.0` implementation complete: Cargo workspace, `okf` parser integration, import-free Wasmtime sandbox, `okf:wasm@1` extension profile, CLI/JSON output, three verdict fixtures, and cross-platform CI. The project is not published yet.

## The one invariant that overrides everything else

A **Reproduced** verdict is a safety claim, not a UX nicety. If there's any doubt about whether the Wasmtime sandbox is fully deterministic for a given execution — network, filesystem, clock, randomness — the correct output is **Unattestable**, never **Reproduced**. Never loosen the sandbox to make a test pass. Never add a "trust me" fallback path. If this invariant and a feature request conflict, the invariant wins and the PRD gets updated to say so explicitly, not silently.

## Current layout (Cargo workspace)

```
okf-attestor/
├── Cargo.toml                 # workspace root
├── Claude.md
├── PRD.md
├── README.md
├── okf-attestor-core/         # library: parsing, sandboxing, verdicts
│   └── src/
│       ├── lib.rs
│       ├── bundle.rs          # thin wrapper around the chosen OKF parser crate
│       ├── contract.rs        # Attested Computation contract types
│       ├── sandbox.rs         # import-free Wasmtime setup and deterministic limits
│       └── verdict.rs         # Verdict enum + comparison logic
├── okf-attestor-cli/           # binary: verify, --concept, --format json
│   └── src/main.rs
└── fixtures/                  # hand-built bundles for tests — at minimum one
                                # Reproduced, one Diverged, one Unattestable case
```

## Commands

- `cargo build --workspace`
- `cargo test --workspace` — must exercise the three fixture cases above. A PR that changes verification behavior without adding or updating a fixture should be treated as incomplete, not just under-tested.
- `cargo run -p okf-attestor -- verify fixtures --format json`

## Dependency decisions

- **OKF parsing:** `okf` 0.2.1. It provides typed Attested Computation discovery plus inline/file computation extraction without a custom parser. `okf-graph` does not extract inline code, and `okf-normative` is not implemented/published.
- **Sandbox:** `wasmtime` 47. The supported profile is deliberately not linked to WASI: modules with any import are Unattestable, which removes network/filesystem/clock/random capabilities structurally. Do not swap in a container-based runtime "for flexibility".

## What NOT to do

- Don't add network access to the sandbox for convenience, even behind a flag, without updating the PRD's Goals section first — zero network calls (G4) is a product commitment, not an implementation detail up for negotiation mid-PR.
- Don't add a new `runtime` to the allowlist without a corresponding fixture bundle proving it round-trips deterministically across platforms.
- Don't start on the MCP server or the desktop UI (Tauri/GPUI). Both are explicitly deferred — see Non-Goals in the PRD. If either becomes worth doing, that's a new PRD, not a scope-creep PR on this one.

## Reference

- Full product context: `PRD.md`
- OKF spec: Google Cloud's `knowledge-catalog` repo, `okf/SPEC.md` — §10 for Attested Computation, §5 for trust/provenance fields this project deliberately does not touch.
