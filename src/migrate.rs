//! Forward-only document-schema upgrader for Loro state.
//!
//! Each project's Loro document carries `meta.schema_version`.  On every
//! [`Store::open`](crate::db::Store::open) call, [`ensure_current`] compares
//! that version to [`CURRENT_SCHEMA_VERSION`] and applies any registered
//! upgraders in sequence.  Upgraders are keyed by the source version they
//! transform *from*; each one is responsible for mutating the document so it
//! conforms to the next version's expectations.
//!
//! When a new schema change is introduced:
//! 1. Bump [`CURRENT_SCHEMA_VERSION`].
//! 2. Write an `upgrade_vN_to_vM` function.
//! 3. Add a match arm in [`upgrader_for`].

use anyhow::{anyhow, bail, Result};
use loro::{LoroDoc, LoroValue, ValueOrContainer};

/// The schema version that the running code expects every document to be at.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Check a document's schema version and apply any necessary upgrades.
///
/// Returns `Ok(true)` when at least one upgrade was applied (the caller
/// should persist the resulting delta), or `Ok(false)` when the document
/// was already current.
///
/// # Errors
///
/// - Document has no `meta.schema_version`.
/// - Document version is newer than `CURRENT_SCHEMA_VERSION`.
/// - No upgrader is registered for a version in the upgrade path.
/// - An upgrader itself fails.
pub fn ensure_current(doc: &LoroDoc) -> Result<bool> {
    let mut version = read_schema_version(doc)?;

    if version == CURRENT_SCHEMA_VERSION {
        return Ok(false);
    }

    if version > CURRENT_SCHEMA_VERSION {
        bail!(
            "document schema version {version} is newer than supported \
             ({CURRENT_SCHEMA_VERSION}); please upgrade td"
        );
    }

    while version < CURRENT_SCHEMA_VERSION {
        let next = version + 1;
        let upgrader = upgrader_for(version).ok_or_else(|| {
            anyhow!("no upgrader registered for schema version {version} → {next}")
        })?;
        upgrader(doc)?;
        version = next;
    }

    let meta = doc.get_map("meta");
    meta.insert("schema_version", CURRENT_SCHEMA_VERSION as i64)?;

    Ok(true)
}

/// Read `meta.schema_version` from an already-loaded Loro document.
///
/// Reads directly from the Loro map handler, avoiding a full-document
/// JSON serialisation.
pub fn read_schema_version(doc: &LoroDoc) -> Result<u32> {
    let meta = doc.get_map("meta");
    match meta.get("schema_version") {
        Some(ValueOrContainer::Value(LoroValue::I64(n))) => {
            u32::try_from(n).map_err(|_| anyhow!("meta.schema_version out of u32 range: {n}"))
        }
        Some(_) => bail!("meta.schema_version has unexpected type"),
        None => bail!("invalid or missing meta.schema_version"),
    }
}

/// Look up the upgrade function that transforms a document *from* the given
/// version to `version + 1`.  Returns `None` when no upgrader is registered.
// The wildcard-only match is intentional: each new schema version adds an
// arm here (e.g. `1 => Some(upgrade_v1_to_v2)`).  Clippy's
// match_single_binding lint fires because there are no concrete arms yet.
#[allow(clippy::match_single_binding)]
fn upgrader_for(version: u32) -> Option<fn(&LoroDoc) -> Result<()>> {
    match version {
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal Loro document stamped at the given schema version.
    fn doc_at_version(v: u32) -> LoroDoc {
        let doc = LoroDoc::new();
        let meta = doc.get_map("meta");
        meta.insert("schema_version", v as i64).unwrap();
        doc.commit();
        doc
    }

    #[test]
    fn current_version_is_noop() {
        let doc = doc_at_version(CURRENT_SCHEMA_VERSION);
        assert!(!ensure_current(&doc).unwrap());
    }

    #[test]
    fn future_version_rejected() {
        let doc = doc_at_version(CURRENT_SCHEMA_VERSION + 1);
        let err = ensure_current(&doc).unwrap_err();
        assert!(
            err.to_string().contains("newer than supported"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn old_version_without_upgrader_gives_clear_error() {
        let doc = doc_at_version(0);
        let err = ensure_current(&doc).unwrap_err();
        assert!(
            err.to_string().contains("no upgrader registered"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn idempotent_on_current_version() {
        let doc = doc_at_version(CURRENT_SCHEMA_VERSION);
        assert!(!ensure_current(&doc).unwrap());
        assert!(!ensure_current(&doc).unwrap());
    }

    #[test]
    fn missing_meta_is_error() {
        let doc = LoroDoc::new();
        doc.get_map("meta"); // exists but has no schema_version key
        doc.commit();
        let err = ensure_current(&doc).unwrap_err();
        assert!(
            err.to_string().contains("meta.schema_version"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn read_schema_version_returns_stamped_value() {
        let doc = doc_at_version(42);
        assert_eq!(read_schema_version(&doc).unwrap(), 42);
    }
}
