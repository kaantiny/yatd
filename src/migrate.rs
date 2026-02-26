//! Versioned schema migrations for the task database.
//!
//! Each migration has up/down SQL and optional post-hook functions that run
//! inside the same transaction. Schema version is tracked via SQLite's built-in
//! `PRAGMA user_version`.

use anyhow::{bail, Context, Result};
use rusqlite::Connection;

/// A single schema migration step.
struct Migration {
    up_sql: &'static str,
    down_sql: &'static str,
    post_hook_up: Option<fn(&rusqlite::Transaction) -> Result<()>>,
    post_hook_down: Option<fn(&rusqlite::Transaction) -> Result<()>>,
}

/// All migrations in order.  The array index is the version the database will
/// be at *after* the migration runs (1-indexed: migration 0 brings the DB to
/// version 1).
static MIGRATIONS: &[Migration] = &[
    // 0 → 1: initial schema
    Migration {
        up_sql: include_str!("migrations/0001_initial_schema.up.sql"),
        down_sql: include_str!("migrations/0001_initial_schema.down.sql"),
        post_hook_up: None,
        post_hook_down: None,
    },
    // 1 → 2: add effort column (integer-backed, default medium)
    Migration {
        up_sql: include_str!("migrations/0002_add_effort.up.sql"),
        down_sql: include_str!("migrations/0002_add_effort.down.sql"),
        post_hook_up: None,
        post_hook_down: None,
    },
    Migration {
        up_sql: include_str!("migrations/0003_blocker_fk.up.sql"),
        down_sql: include_str!("migrations/0003_blocker_fk.down.sql"),
        post_hook_up: None,
        post_hook_down: None,
    },
    Migration {
        up_sql: include_str!("migrations/0004_task_logs.up.sql"),
        down_sql: include_str!("migrations/0004_task_logs.down.sql"),
        post_hook_up: None,
        post_hook_down: None,
    },
    Migration {
        up_sql: include_str!("migrations/0005_cascade_fks.up.sql"),
        down_sql: include_str!("migrations/0005_cascade_fks.down.sql"),
        post_hook_up: None,
        post_hook_down: None,
    },
];

/// Read the current schema version from the database.
fn get_version(conn: &Connection) -> Result<u32> {
    let v: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    Ok(v)
}

/// Set the schema version inside an open transaction.
fn set_version(tx: &rusqlite::Transaction, version: u32) -> Result<()> {
    // PRAGMA cannot be parameterised, but the value is a u32 we control.
    tx.pragma_update(None, "user_version", version)?;
    Ok(())
}

/// Apply all pending up-migrations to bring the database to the latest version.
pub fn migrate_up(conn: &mut Connection) -> Result<()> {
    let current = get_version(conn)?;
    let latest = MIGRATIONS.len() as u32;

    if current > latest {
        bail!(
            "database is at version {current} but this binary only knows up to {latest}; \
             upgrade td or use a matching version"
        );
    }

    for (idx, m) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let target_version = (idx + 1) as u32;

        let tx = conn
            .transaction()
            .context("failed to begin migration transaction")?;

        if !m.up_sql.is_empty() {
            tx.execute_batch(m.up_sql)
                .with_context(|| format!("migration {target_version} up SQL failed"))?;
        }

        if let Some(hook) = m.post_hook_up {
            hook(&tx)
                .with_context(|| format!("migration {target_version} post-hook (up) failed"))?;
        }

        set_version(&tx, target_version)?;

        tx.commit()
            .with_context(|| format!("failed to commit migration {target_version}"))?;
    }

    Ok(())
}

/// Roll back migrations down to `target_version` (inclusive — the database
/// will be at `target_version` when this returns).
pub fn migrate_down(conn: &mut Connection, target_version: u32) -> Result<()> {
    let current = get_version(conn)?;

    if target_version >= current {
        bail!("target version {target_version} is not below current version {current}");
    }

    if target_version > MIGRATIONS.len() as u32 {
        bail!("target version {target_version} exceeds known migrations");
    }

    // Walk backwards: if we're at version 3 and want version 1, we undo
    // migration index 2 (v3→v2) then index 1 (v2→v1).
    for (idx, m) in MIGRATIONS
        .iter()
        .enumerate()
        .rev()
        .filter(|(i, _)| *i >= target_version as usize && *i < current as usize)
    {
        let from_version = (idx + 1) as u32;

        let tx = conn
            .transaction()
            .context("failed to begin down-migration transaction")?;

        if let Some(hook) = m.post_hook_down {
            hook(&tx)
                .with_context(|| format!("migration {from_version} post-hook (down) failed"))?;
        }

        if !m.down_sql.is_empty() {
            tx.execute_batch(m.down_sql)
                .with_context(|| format!("migration {from_version} down SQL failed"))?;
        }

        set_version(&tx, idx as u32)?;

        tx.commit()
            .with_context(|| format!("failed to commit down-migration {from_version}"))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_up_from_empty() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_up(&mut conn).unwrap();

        let version = get_version(&conn).unwrap();
        assert_eq!(version, MIGRATIONS.len() as u32);

        // Verify tables exist by querying them.
        conn.execute_batch("SELECT id FROM tasks LIMIT 0").unwrap();
        conn.execute_batch("SELECT task_id FROM labels LIMIT 0")
            .unwrap();
        conn.execute_batch("SELECT task_id FROM blockers LIMIT 0")
            .unwrap();
        conn.execute_batch("SELECT task_id FROM task_logs LIMIT 0")
            .unwrap();
    }

    #[test]
    fn migrate_up_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_up(&mut conn).unwrap();
        // Running again should be a no-op, not an error.
        migrate_up(&mut conn).unwrap();
        assert_eq!(get_version(&conn).unwrap(), MIGRATIONS.len() as u32);
    }

    #[test]
    fn migrate_down_to_zero() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_up(&mut conn).unwrap();
        migrate_down(&mut conn, 0).unwrap();
        assert_eq!(get_version(&conn).unwrap(), 0);

        // Tables should be gone.
        let result = conn.execute_batch("SELECT id FROM tasks LIMIT 0");
        assert!(result.is_err());
    }

    #[test]
    fn rejects_future_version() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        let err = migrate_up(&mut conn).unwrap_err();
        assert!(
            err.to_string().contains("999"),
            "error should mention the version: {err}"
        );
    }
}
