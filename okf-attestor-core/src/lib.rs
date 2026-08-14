//! Offline, fail-closed verification for OKF v0.2 Attested Computations.
//!
//! Canonical OKF v0.2 does not persist invocation values, receipts, expected output,
//! or a portable executor ABI. This crate therefore reproduces only the documented
//! `okf:wasm@1` extension profile. Every other runtime is reported as
//! [`Verdict::Unattestable`]. The supported WebAssembly module receives no WASI or
//! other host imports, so filesystem, network, clock, and randomness are absent.

mod bundle;
mod contract;
mod sandbox;
mod verdict;

use std::path::Path;

use okf::Concept;
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub use contract::SUPPORTED_RUNTIME;
pub use verdict::{ReasonCode, Verdict};

use contract::load_profile;
use sandbox::Sandbox;

/// Version of the serialized [`BundleReport`] schema.
pub const REPORT_SCHEMA_VERSION: u32 = 1;

/// A report covering all selected Attested Computations in one bundle.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BundleReport {
    pub schema_version: u32,
    pub bundle: String,
    pub results: Vec<ConceptReport>,
}

impl BundleReport {
    /// Returns the process exit code defined by the CLI contract.
    pub fn exit_code(&self) -> u8 {
        if self
            .results
            .iter()
            .any(|result| result.verdict == Verdict::Diverged)
        {
            1
        } else if self
            .results
            .iter()
            .any(|result| result.verdict == Verdict::Unattestable)
        {
            2
        } else {
            0
        }
    }
}

/// Verification result for one OKF concept.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConceptReport {
    pub concept: String,
    pub verdict: Verdict,
    pub reason_code: ReasonCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<ArtifactDigests>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub difference: Option<OutputDifference>,
}

/// SHA-256 digests binding a report to the exact local artifacts read.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArtifactDigests {
    pub computation_sha256: String,
    pub input_sha256: String,
    pub expected_output_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_output_sha256: Option<String>,
}

/// Bounded, UTF-8-safe details for a deterministic output mismatch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct OutputDifference {
    pub expected_sha256: String,
    pub actual_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_utf8: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_utf8: Option<String>,
}

/// Errors that prevent a meaningful verification report from being produced.
#[derive(Debug, Error)]
pub enum AttestorError {
    #[error("could not load OKF bundle: {0}")]
    BundleLoad(String),
    #[error("bundle contains malformed Markdown frontmatter: {0}")]
    BundleParse(String),
    #[error("Attested Computation concept not found: {0}")]
    ConceptNotFound(String),
    #[error("could not initialize the deterministic sandbox: {0}")]
    SandboxInitialization(String),
}

/// Reusable verifier with a deterministically configured Wasmtime engine.
pub struct Verifier {
    sandbox: Sandbox,
}

impl Verifier {
    /// Builds a verifier. Failure here is a tool-level error, not a concept verdict.
    pub fn new() -> Result<Self, AttestorError> {
        let sandbox =
            Sandbox::new().map_err(|error| AttestorError::SandboxInitialization(error.message))?;
        Ok(Self { sandbox })
    }

    /// Verifies every Attested Computation in a local OKF bundle.
    pub fn verify_bundle(&self, path: impl AsRef<Path>) -> Result<BundleReport, AttestorError> {
        let path = path.as_ref();
        let bundle = bundle::load(path)?;
        let results = bundle
            .attested_computations()
            .map(|concept| self.verify_loaded_concept(bundle.root(), concept))
            .collect();
        Ok(BundleReport {
            schema_version: REPORT_SCHEMA_VERSION,
            bundle: path.display().to_string(),
            results,
        })
    }

    /// Verifies one Attested Computation selected by its bundle-relative concept id.
    pub fn verify_concept(
        &self,
        path: impl AsRef<Path>,
        concept_id: &str,
    ) -> Result<ConceptReport, AttestorError> {
        let bundle = bundle::load(path.as_ref())?;
        let concept = bundle::find_concept(&bundle, concept_id)?;
        Ok(self.verify_loaded_concept(bundle.root(), concept))
    }

    fn verify_loaded_concept(&self, root: &Path, concept: &Concept) -> ConceptReport {
        let id = concept.id.to_string();
        let Some(runtime) = concept.document.frontmatter.runtime() else {
            return unattestable(
                id,
                ReasonCode::MissingRuntime,
                "Attested Computation has no runtime",
            );
        };
        if runtime != SUPPORTED_RUNTIME {
            return unattestable(
                id,
                ReasonCode::UnsupportedRuntime,
                format!(
                    "runtime `{runtime}` is not supported; only `{SUPPORTED_RUNTIME}` is allowed"
                ),
            );
        }

        let profile = match load_profile(root, concept) {
            Ok(profile) => profile,
            Err(error) => return unattestable(id, error.code, error.message),
        };
        let mut digests = ArtifactDigests {
            computation_sha256: profile.module.sha256.clone(),
            input_sha256: profile.input.sha256.clone(),
            expected_output_sha256: profile.expected.sha256.clone(),
            actual_output_sha256: None,
        };

        let actual = match self
            .sandbox
            .execute(&profile.module.bytes, &profile.input.bytes)
        {
            Ok(actual) => actual,
            Err(error) => {
                return ConceptReport {
                    concept: id,
                    verdict: Verdict::Unattestable,
                    reason_code: error.code,
                    message: error.message,
                    artifacts: Some(digests),
                    difference: None,
                };
            }
        };
        let actual_sha256 = sha256(&actual);
        digests.actual_output_sha256 = Some(actual_sha256.clone());

        if actual == profile.expected.bytes {
            ConceptReport {
                concept: id,
                verdict: Verdict::Reproduced,
                reason_code: ReasonCode::OutputMatch,
                message: "deterministic output matched the expected bytes exactly".to_owned(),
                artifacts: Some(digests),
                difference: None,
            }
        } else {
            ConceptReport {
                concept: id,
                verdict: Verdict::Diverged,
                reason_code: ReasonCode::OutputMismatch,
                message:
                    "deterministic execution completed, but output differed from expected bytes"
                        .to_owned(),
                artifacts: Some(digests),
                difference: Some(OutputDifference {
                    expected_sha256: profile.expected.sha256,
                    actual_sha256,
                    expected_utf8: String::from_utf8(profile.expected.bytes).ok(),
                    actual_utf8: String::from_utf8(actual).ok(),
                }),
            }
        }
    }
}

/// Convenience function that verifies every Attested Computation in a bundle.
pub fn verify_bundle(path: impl AsRef<Path>) -> Result<BundleReport, AttestorError> {
    Verifier::new()?.verify_bundle(path)
}

/// Convenience function that verifies a single concept by id.
pub fn verify_concept(
    path: impl AsRef<Path>,
    concept_id: &str,
) -> Result<ConceptReport, AttestorError> {
    Verifier::new()?.verify_concept(path, concept_id)
}

fn unattestable(
    concept: String,
    reason_code: ReasonCode,
    message: impl Into<String>,
) -> ConceptReport {
    ConceptReport {
        concept,
        verdict: Verdict::Unattestable,
        reason_code,
        message: message.into(),
        artifacts: None,
        difference: None,
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
