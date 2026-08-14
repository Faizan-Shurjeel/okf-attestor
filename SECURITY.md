# Security Policy

## Reporting a vulnerability

Please do not open a public issue for a suspected sandbox escape, path traversal, resource-limit bypass, or incorrect Reproduced verdict. Use GitHub's private vulnerability reporting for this repository once enabled, or contact the repository owner privately.

Include the affected version, a minimal bundle or Wasm module when safe, observed behavior, and expected behavior. Do not include secrets or sensitive production bundles.

## Security boundary

Only the exact `okf:wasm@1` profile is executable. Modules with any imports are rejected. Unsupported runtimes and execution failures are Unattestable. See `README.md` for the full security model and current denial-of-service limitations.
