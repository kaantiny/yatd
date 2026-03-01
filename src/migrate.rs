//! Loro-backed storage does not use SQL schema migrations.
//!
//! The old SQLite migration flow has been replaced by document-level metadata
//! (`meta.schema_version`) in the Loro snapshot. This module remains only as a
//! compatibility shim for call sites that still invoke migration entry points.

use anyhow::Result;

/// No-op compatibility function for legacy call sites.
pub fn migrate_up<T>(_conn: &mut T) -> Result<()> {
    Ok(())
}

/// No-op compatibility function for legacy call sites.
pub fn migrate_down<T>(_conn: &mut T, _target_version: u32) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_noops_succeed() {
        let mut placeholder = ();
        migrate_up(&mut placeholder).unwrap();
        migrate_down(&mut placeholder, 0).unwrap();
    }
}
