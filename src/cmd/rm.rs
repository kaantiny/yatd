use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use crate::db;

#[derive(Serialize)]
struct RmResult {
    requested_ids: Vec<String>,
    deleted_ids: Vec<String>,
    unblocked_ids: Vec<String>,
}

pub fn run(root: &Path, ids: &[String], recursive: bool, force: bool, json: bool) -> Result<()> {
    let store = db::open(root)?;
    let all = store.list_tasks_unfiltered()?;

    let mut to_delete = BTreeSet::new();
    for raw in ids {
        let id = db::resolve_task_id(&store, raw, false)?;
        if recursive {
            collect_subtree(&all, &id, &mut to_delete);
        } else {
            if all
                .iter()
                .any(|t| t.parent.as_ref() == Some(&id) && t.deleted_at.is_none())
            {
                bail!("task '{id}' has children; use --recursive to delete subtree");
            }
            to_delete.insert(id);
        }
    }

    let deleted_ids: Vec<db::TaskId> = to_delete.into_iter().collect();
    let deleted_set: HashSet<String> = deleted_ids
        .iter()
        .map(|id| id.as_str().to_string())
        .collect();

    let unblocked_ids: Vec<db::TaskId> = all
        .iter()
        .filter(|t| !deleted_set.contains(t.id.as_str()))
        .filter(|t| t.blockers.iter().any(|b| deleted_set.contains(b.as_str())))
        .map(|t| t.id.clone())
        .collect();

    let ts = db::now_utc();
    store.apply_and_persist(|doc| {
        let tasks = doc.get_map("tasks");

        for task_id in &deleted_ids {
            let task =
                db::get_task_map(&tasks, task_id)?.ok_or_else(|| anyhow!("task not found"))?;
            task.insert("deleted_at", ts.clone())?;
            task.insert("updated_at", ts.clone())?;
            task.insert("status", db::status_label(db::Status::Closed))?;
        }

        for task in store.list_tasks_unfiltered()? {
            if deleted_set.contains(task.id.as_str()) {
                continue;
            }
            if let Some(task_map) = db::get_task_map(&tasks, &task.id)? {
                let blockers = db::get_or_create_child_map(&task_map, "blockers")?;
                for deleted in &deleted_ids {
                    blockers.delete(deleted.as_str())?;
                }
            }
        }

        Ok(())
    })?;

    if !force && !unblocked_ids.is_empty() {
        let short: Vec<String> = unblocked_ids.iter().map(ToString::to_string).collect();
        eprintln!("warning: removed blockers from {}", short.join(", "));
    }

    if json {
        let out = RmResult {
            requested_ids: ids.to_vec(),
            deleted_ids: deleted_ids
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
            unblocked_ids: unblocked_ids
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
        };
        println!("{}", serde_json::to_string(&out)?);
    } else {
        let c = crate::color::stdout_theme();
        for id in deleted_ids {
            println!("{}deleted{} {id}", c.green, c.reset);
        }
    }

    Ok(())
}

fn collect_subtree(all: &[db::Task], root: &db::TaskId, out: &mut BTreeSet<db::TaskId>) {
    if !out.insert(root.clone()) {
        return;
    }
    for task in all {
        if task.parent.as_ref() == Some(root) && task.deleted_at.is_none() {
            collect_subtree(all, &task.id, out);
        }
    }
}
