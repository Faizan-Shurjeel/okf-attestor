# PRD: OKF Attestor

## 1. Introduction/Overview

OKF (Open Knowledge Format) v0.2 introduces the **Attested Computation** concept type, meant to prove a reported figure was produced by a sanctioned calculation rather than invented by an agent. As written today, an Attested Computation is just a claim: the bundle states what `runtime`, `parameters`, and `computation` were supposedly used, and who executed and attested it — but nothing in the current ecosystem actually re-runs it. Anyone with write access to the bundle (including an agent) can forge the record, and a consumer has no way to tell.

OKF Attestor closes that gap. It's a Rust CLI and library that takes an OKF bundle, finds every Attested Computation concept in it, re-executes the declared computation inside a sandboxed, deterministic runtime, and reports whether the result actually matches what the bundle claims — entirely offline, no network calls, in the same spirit as Verascope's approach to C2PA: don't trust the claim, verify it yourself, locally.

## 2. Goals

- **G1:** Given a bundle, correctly re-execute every Attested Computation whose declared runtime is supported, and report a verdict per concept.
- **G2:** Never produce a false "Reproduced" verdict. If the sandbox can't guarantee determinism for a given case, fail closed into "Unattestable" — never a false pass.
- **G3:** Ship as a Rust library first, with a thin CLI on top, so the same verification engine can later power a desktop UI or other integration without a rewrite.
- **G4:** Zero network calls during verification — matches Verascope's privacy-by-architecture principle, not just a policy promise.
- **G5:** Runnable in CI with a machine-readable output mode and meaningful exit codes.

## 3. User Stories

- As a developer maintaining an OKF bundle for my team's finance metrics, I want to run one command before merging a PR that touches an Attested Computation, so I catch a wrong formula before an agent ever cites it.
- As a CI pipeline, I want the build to fail if any Attested Computation in the bundle diverges from its receipt, so a forged or stale computation never ships silently.
- As someone auditing an AI agent's report after the fact, I want to point this tool at the bundle the agent cited and get a plain verdict on whether the number was actually computed the declared way.
- As a contributor to OKF Attestor itself, I want a library API, not just a CLI, so the same verification logic can later be embedded in a desktop app or another tool without duplicating it.

## 4. Functional Requirements

1. The system must load an OKF bundle from a local directory path, using an existing OKF-parsing crate — it must not implement a new bundle parser from scratch.
2. The system must identify every concept whose `type` matches the spec's Attested Computation marker (§10.1).
3. For each Attested Computation concept, the system must parse its canonical contract fields. Because OKF v0.2 defines receipt field names but no persisted receipt/output wire format, executable concepts must additionally use the documented `okf_attestor` extension profile.
4. The system must maintain an explicit allowlist of supported `runtime` values for v1. Any concept whose runtime is not on the allowlist must be reported as **Unattestable** — never silently skipped, and never silently passed.
5. For a supported runtime, the system must execute the declared computation with Wasmtime and no linked imports. WASI is deliberately absent, structurally removing network, filesystem, wall-clock, randomness, environment, argument, and stdio capabilities.
6. The system must compare the sandbox's output byte-for-byte against the extension profile's local expected-output artifact and classify the concept into exactly one of three states: **Reproduced**, **Diverged**, or **Unattestable**.
7. The CLI must support `okf-attestor verify <bundle-path>` to check every Attested Computation in a bundle, and a `--concept <id>` flag to check a single one.
8. The CLI must support a `--format json` flag emitting a machine-readable report (one entry per concept, with its verdict and any diverging values) for CI consumption.
9. The CLI must exit non-zero if any concept is Diverged, and must make an Unattestable-only result distinguishable from a fully clean pass (via exit code or explicit flag).
10. The library must expose a typed `Verdict` enum (`Reproduced`, `Diverged`, `Unattestable`) plus functions to verify a single concept or an entire bundle, independent of the CLI.
11. The system must never execute a computation whose runtime isn't on the explicit allowlist, even in a "best effort" mode. Unsupported means Unattestable, full stop — there is no partial-trust execution path.

## 5. Non-Goals (Out of Scope for v1)

- No MCP server. Deferred until the CLI/library is stable; may not happen at all if it doesn't add enough value over the CLI.
- No desktop UI (Tauri or GPUI) in v1. The library is structured to support one later; v1 ships CLI-only.
- No support for non-WASM runtimes (SQL engines, shell, Python, containers) in v1. One deterministic runtime done well beats five done poorly.
- No cryptographic signing or bundle-level notarization. That's a separate concern (bundle/actor identity, not computation correctness) and is intentionally not this tool's job.
- No verification of `verified`/trust-tier fields or provenance chains — a different tool's job.
- No network fetching of remote `computation` references in v1. If a computation isn't fully self-contained in the bundle, treat it as Unattestable rather than fetching it.

## 6. Design Considerations

- CLI output should echo Verascope's three-state, non-binary framing: a clean pass is never phrased as "verified TRUE," and an unsupported runtime is never phrased as suspicious — it's explicitly "not evidence of anything," mirroring Verascope's "No Provenance" state.
- The JSON output schema should be stable and documented starting in v1, since it's the integration surface for CI today and for any future UI.
- If a desktop UI is ever built, it should reuse Verascope's visual language (colored state badges, any heuristic signal kept in a clearly separated, non-authoritative panel) rather than invent a new one.

## 7. Technical Considerations

- Use `okf` 0.2.1 for bundle parsing and Attested Computation discovery. It was selected over `okf-graph` (which leaves inline computation extraction to consumers) and the currently unimplemented/unpublished `okf-normative`.
- Use `wasmtime` for the sandbox: pure Rust, embeddable, no external runtime to install, and cross-platform on Windows/macOS/Linux.
- Determinism is the load-bearing requirement, not a nice-to-have. The `okf:wasm@1` profile rejects every module import and instantiates with an empty linker instead of configuring a broad WASI context. Input/output cross the narrow memory ABI as bytes.
- No telemetry, no auto-update fetching, no phone-home of any kind — "privacy by architecture," matching Verascope, not "privacy by policy."
- Ship as a Cargo workspace from day one (`okf-attestor-core` library and `okf-attestor` CLI package in the `okf-attestor-cli` directory) so `cargo install okf-attestor` has the intended package name and a GUI crate can be added later without restructuring.

## 8. Success Metrics

**Planning milestone:** complete. The initial implementation and its profile decisions are documented in `README.md`.

**v1 product milestone (once built):**
- Correctly classifies a hand-built fixture bundle containing at least one Reproduced, one Diverged, and one Unattestable Attested Computation.
- Runs clean in a CI job with no network access enabled, proving the "zero network calls" claim isn't just marketing copy.
- `cargo install okf-attestor` works on a clean machine on all three major OSes with no additional runtime installs required.

## 9. Resolved Decisions and Open Questions

Resolved for v1:
- Canonical OKF v0.2 has no persisted receipt/output field or portable attester ABI. OKF Attestor uses the producer extension documented in `README.md`.
- The sole allowlisted runtime is the exact, case-sensitive `okf:wasm@1` value.
- Executable computation, input, and expected output must be local regular files. Inline computation and external URIs are Unattestable.

Still open:
- Tauri vs. GPUI for the eventual UI — no need to decide now; revisit at the start of a UI phase.
- Whether this lives as a new standalone repo/brand or explicitly as a sibling project to Verascope (shared org, shared design language) — unresolved, worth settling before the README is written.
- Whether an MCP server ever gets built, and if so, whether it wraps the CLI or calls the core library directly.
