use serde::Serialize;

/// The only three outcomes produced for an Attested Computation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Execution completed deterministically and the bytes matched exactly.
    Reproduced,
    /// Execution completed deterministically, but the output bytes differed.
    Diverged,
    /// The computation could not be executed under the supported deterministic profile.
    Unattestable,
}

/// Stable, machine-readable explanation for a verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    OutputMatch,
    OutputMismatch,
    MissingRuntime,
    UnsupportedRuntime,
    InvalidProfile,
    UnsafeArtifactPath,
    MissingArtifact,
    ArtifactTooLarge,
    InvalidModule,
    ForbiddenImport,
    InvalidAbi,
    ExecutionFailed,
}
