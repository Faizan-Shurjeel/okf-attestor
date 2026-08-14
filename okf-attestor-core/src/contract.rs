use std::fs;
use std::path::{Component, Path, PathBuf};

use okf::Concept;
use sha2::{Digest, Sha256};

use crate::verdict::ReasonCode;

pub const SUPPORTED_RUNTIME: &str = "okf:wasm@1";
pub(crate) const MAX_MODULE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_VALUE_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub(crate) struct Profile {
    pub module: Artifact,
    pub input: Artifact,
    pub expected: Artifact,
}

#[derive(Debug)]
pub(crate) struct Artifact {
    pub bytes: Vec<u8>,
    pub sha256: String,
}

#[derive(Debug)]
pub(crate) struct ProfileError {
    pub code: ReasonCode,
    pub message: String,
}

impl ProfileError {
    fn new(code: ReasonCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub(crate) fn load_profile(bundle_root: &Path, concept: &Concept) -> Result<Profile, ProfileError> {
    let computation = concept.document.frontmatter.computation().ok_or_else(|| {
        ProfileError::new(
            ReasonCode::InvalidProfile,
            "okf:wasm@1 requires a file-backed computation",
        )
    })?;

    let extension = concept
        .document
        .frontmatter
        .get("okf_attestor")
        .and_then(|value| value.as_mapping())
        .ok_or_else(|| {
            ProfileError::new(ReasonCode::InvalidProfile, "missing okf_attestor mapping")
        })?;

    if extension.get("protocol").and_then(|value| value.as_int()) != Some(1) {
        return Err(ProfileError::new(
            ReasonCode::InvalidProfile,
            "okf_attestor.protocol must be integer 1",
        ));
    }

    let input = required_path(extension.get("input"), "okf_attestor.input")?;
    let expected = required_path(
        extension.get("expected_output"),
        "okf_attestor.expected_output",
    )?;

    let concept_dir = concept.path.parent().ok_or_else(|| {
        ProfileError::new(
            ReasonCode::UnsafeArtifactPath,
            "concept has no parent directory",
        )
    })?;

    Ok(Profile {
        module: read_artifact(bundle_root, concept_dir, &computation, MAX_MODULE_BYTES)?,
        input: read_artifact(bundle_root, concept_dir, input, MAX_VALUE_BYTES)?,
        expected: read_artifact(bundle_root, concept_dir, expected, MAX_VALUE_BYTES)?,
    })
}

fn required_path<'a>(
    value: Option<&'a okf::yaml::Value>,
    field: &str,
) -> Result<&'a str, ProfileError> {
    value
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            ProfileError::new(
                ReasonCode::InvalidProfile,
                format!("{field} must be a non-empty local path string"),
            )
        })
        .and_then(|value| {
            if value.is_empty() {
                Err(ProfileError::new(
                    ReasonCode::InvalidProfile,
                    format!("{field} must be a non-empty local path string"),
                ))
            } else {
                Ok(value)
            }
        })
}

fn read_artifact(
    bundle_root: &Path,
    concept_dir: &Path,
    raw: &str,
    limit: usize,
) -> Result<Artifact, ProfileError> {
    let relative = Path::new(raw);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        || raw.contains("://")
    {
        return Err(ProfileError::new(
            ReasonCode::UnsafeArtifactPath,
            format!("artifact path is not a safe relative path: {raw}"),
        ));
    }

    let canonical_root = fs::canonicalize(bundle_root).map_err(|error| {
        ProfileError::new(
            ReasonCode::MissingArtifact,
            format!("cannot resolve bundle root: {error}"),
        )
    })?;
    let canonical_dir = fs::canonicalize(concept_dir).map_err(|error| {
        ProfileError::new(
            ReasonCode::MissingArtifact,
            format!("cannot resolve concept directory: {error}"),
        )
    })?;
    if !canonical_dir.starts_with(&canonical_root) {
        return Err(ProfileError::new(
            ReasonCode::UnsafeArtifactPath,
            "concept directory resolves outside the bundle",
        ));
    }

    let mut candidate = PathBuf::from(concept_dir);
    for component in relative.components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(part) => candidate.push(part),
            _ => unreachable!("unsafe path components were rejected"),
        }
        let metadata = fs::symlink_metadata(&candidate).map_err(|error| {
            ProfileError::new(
                ReasonCode::MissingArtifact,
                format!("cannot access artifact {raw}: {error}"),
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(ProfileError::new(
                ReasonCode::UnsafeArtifactPath,
                format!("artifact path contains a symbolic link: {raw}"),
            ));
        }
    }

    let canonical = fs::canonicalize(&candidate).map_err(|error| {
        ProfileError::new(
            ReasonCode::MissingArtifact,
            format!("cannot resolve artifact {raw}: {error}"),
        )
    })?;
    if !canonical.starts_with(&canonical_root) {
        return Err(ProfileError::new(
            ReasonCode::UnsafeArtifactPath,
            format!("artifact resolves outside the bundle: {raw}"),
        ));
    }

    let metadata = fs::metadata(&canonical).map_err(|error| {
        ProfileError::new(
            ReasonCode::MissingArtifact,
            format!("cannot inspect artifact {raw}: {error}"),
        )
    })?;
    if !metadata.is_file() {
        return Err(ProfileError::new(
            ReasonCode::MissingArtifact,
            format!("artifact is not a regular file: {raw}"),
        ));
    }
    if metadata.len() > limit as u64 {
        return Err(ProfileError::new(
            ReasonCode::ArtifactTooLarge,
            format!("artifact exceeds the {limit}-byte limit: {raw}"),
        ));
    }

    let bytes = fs::read(&canonical).map_err(|error| {
        ProfileError::new(
            ReasonCode::MissingArtifact,
            format!("cannot read artifact {raw}: {error}"),
        )
    })?;
    if bytes.len() > limit {
        return Err(ProfileError::new(
            ReasonCode::ArtifactTooLarge,
            format!("artifact exceeds the {limit}-byte limit: {raw}"),
        ));
    }

    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    Ok(Artifact { bytes, sha256 })
}
