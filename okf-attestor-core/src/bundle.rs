use std::path::Path;

use okf::{Bundle, Concept};

use crate::AttestorError;

pub(crate) fn load(path: &Path) -> Result<Bundle, AttestorError> {
    let bundle =
        Bundle::load(path).map_err(|error| AttestorError::BundleLoad(error.to_string()))?;
    if !bundle.parse_errors().is_empty() {
        let details = bundle
            .parse_errors()
            .iter()
            .map(|(path, error)| format!("{}: {error}", path.display()))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(AttestorError::BundleParse(details));
    }
    Ok(bundle)
}

pub(crate) fn find_concept<'a>(bundle: &'a Bundle, id: &str) -> Result<&'a Concept, AttestorError> {
    bundle
        .attested_computations()
        .find(|concept| concept.id.to_string() == id)
        .ok_or_else(|| AttestorError::ConceptNotFound(id.to_owned()))
}
