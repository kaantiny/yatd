use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

use crate::db;

#[derive(Serialize)]
struct RmResult {
    requested_ids: Vec<String>,
    deleted_ids: Vec<String>,
    unblocked_ids: Vec<String>,
}

pub fn run(root: &Path, ids: &[String], recursive: bool, force: bool, json: bool) -> Result<()> {
    let mut conn = db::open(root)?;
    let tx = conn.transaction()?;

    let mut to_delete = BTreeSet::new();
    for id in ids {
        if !db::task_exists(&tx, id)? {
            bail!("task '{id}' not found");
        }

        if recursive {
            for subtree_id in load_subtree_ids(&tx, id)? {
                to_delete.insert(subtree_id);
            }
        } else {
            let child_count: i64 = tx.query_row(
                "SELECT COUNT(*) FROM tasks WHERE parent = ?1",
                [id],
                |row| row.get(0),
            )?;
            if child_count > 0 {
                bail!("task '{id}' has children; use --recursive to delete subtree");
            }
            to_delete.insert(id.clone());
        }
    }

    let deleted_ids: Vec<String> = to_delete.into_iter().collect();
    let unblocked_ids = detach_dependents(&tx, &deleted_ids)?;

    if !deleted_ids.is_empty() {
        delete_tasks(&tx, &deleted_ids)?;
    }

    tx.commit()?;

    if !force && !unblocked_ids.is_empty() {
        eprintln!(
            "warning: removed blockers from {}",
            unblocked_ids.join(", ")
        );
    }

    if json {
        let out = RmResult {
            requested_ids: ids.to_vec(),
            deleted_ids,
            unblocked_ids,
        };
        println!("{}", serde_json::to_string(&out)?);
    } else {
        let c = crate::color::stdout_theme();
        for id in &deleted_ids {
            println!("{}deleted{} {id}", c.green, c.reset);
        }
    }

    Ok(())
}

fn load_subtree_ids(tx: &rusqlite::Transaction, root_id: &str) -> Result<Vec<String>> {
    let mut stmt = tx.prepare(
        "WITH RECURSIVE subtree(id) AS (
             SELECT id FROM tasks WHERE id = ?1
             UNION ALL
             SELECT tasks.id
             FROM tasks
             JOIN subtree ON tasks.parent = subtree.id
         )
         SELECT id FROM subtree",
    )?;
    let ids = stmt
        .query_map([root_id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(ids)
}

fn detach_dependents(tx: &rusqlite::Transaction, deleted_ids: &[String]) -> Result<Vec<String>> {
    if deleted_ids.is_empty() {
        return Ok(Vec::new());
    }

    let in_placeholders = vec!["?"; deleted_ids.len()].join(", ");
    let sql = format!(
        "SELECT DISTINCT task_id
         FROM blockers
         WHERE blocker_id IN ({in_placeholders})
           AND task_id NOT IN ({in_placeholders})
         ORDER BY task_id"
    );
    let params = deleted_ids.iter().chain(deleted_ids.iter());
    let mut stmt = tx.prepare(&sql)?;
    let unblocked_ids = stmt
        .query_map(rusqlite::params_from_iter(params), |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;

    if unblocked_ids.is_empty() {
        return Ok(unblocked_ids);
    }

    let delete_sql = format!(
        "DELETE FROM blockers
         WHERE blocker_id IN ({in_placeholders})
           AND task_id NOT IN ({in_placeholders})"
    );
    let delete_params = deleted_ids.iter().chain(deleted_ids.iter());
    tx.execute(&delete_sql, rusqlite::params_from_iter(delete_params))?;

    let update_placeholders = vec!["?"; unblocked_ids.len()].join(", ");
    let update_sql = format!(
        "UPDATE tasks
         SET updated = ?1
         WHERE id IN ({update_placeholders})"
    );
    let ts = db::now_utc();
    let update_params = std::iter::once(&ts).chain(unblocked_ids.iter());
    tx.execute(&update_sql, rusqlite::params_from_iter(update_params))?;

    Ok(unblocked_ids)
}

fn delete_tasks(tx: &rusqlite::Transaction, deleted_ids: &[String]) -> Result<()> {
    let in_placeholders = vec!["?"; deleted_ids.len()].join(", ");
    let sql = format!("DELETE FROM tasks WHERE id IN ({in_placeholders})");
    tx.execute(&sql, rusqlite::params_from_iter(deleted_ids.iter()))?;
    Ok(())
}
