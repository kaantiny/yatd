use anyhow::{anyhow, bail, Result};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use crate::cli::DepAction;
use crate::db;

pub fn run(root: &Path, action: &DepAction, json: bool) -> Result<()> {
    let store = db::open(root)?;

    match action {
        DepAction::Add { child, parent } => {
            let child_id = db::resolve_task_id(&store, child, false)?;
            let parent_id = db::resolve_task_id(&store, parent, false)?;
            if child_id == parent_id {
                bail!("adding dependency would create a cycle");
            }
            if would_cycle(&store, &child_id, &parent_id)? {
                bail!("adding dependency would create a cycle");
            }
            let ts = db::now_utc();
            store.apply_and_persist(|doc| {
                let tasks = doc.get_map("tasks");
                let child_task = db::get_task_map(&tasks, &child_id)?
                    .ok_or_else(|| anyhow!("task not found"))?;
                let blockers = db::get_or_create_child_map(&child_task, "blockers")?;
                blockers.insert(parent_id.as_str(), true)?;
                child_task.insert("updated_at", ts.clone())?;
                Ok(())
            })?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({"child": child_id.as_str(), "blocker": parent_id.as_str()})
                );
            } else {
                let c = crate::color::stdout_theme();
                println!(
                    "{}{child_id}{} blocked by {}{parent_id}{}",
                    c.green, c.reset, c.yellow, c.reset
                );
            }
        }
        DepAction::Rm { child, parent } => {
            let child_id = db::resolve_task_id(&store, child, false)?;
            let parent_id = db::resolve_task_id(&store, parent, true)?;
            let ts = db::now_utc();
            store.apply_and_persist(|doc| {
                let tasks = doc.get_map("tasks");
                let child_task = db::get_task_map(&tasks, &child_id)?
                    .ok_or_else(|| anyhow!("task not found"))?;
                let blockers = db::get_or_create_child_map(&child_task, "blockers")?;
                blockers.delete(parent_id.as_str())?;
                child_task.insert("updated_at", ts.clone())?;
                Ok(())
            })?;
            if !json {
                let c = crate::color::stdout_theme();
                println!(
                    "{}{child_id}{} no longer blocked by {}{parent_id}{}",
                    c.green, c.reset, c.yellow, c.reset
                );
            }
        }
        DepAction::Tree { id } => {
            let root_id = db::resolve_task_id(&store, id, true)?;
            println!("{}", root_id);
            let mut children: Vec<_> = store
                .list_tasks_unfiltered()?
                .into_iter()
                .filter(|t| t.parent.as_ref() == Some(&root_id))
                .map(|t| t.id)
                .collect();
            children.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            for child in children {
                println!("  {child}");
            }
        }
    }

    Ok(())
}

fn would_cycle(store: &db::Store, child: &db::TaskId, parent: &db::TaskId) -> Result<bool> {
    let tasks = store.list_tasks_unfiltered()?;
    let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
    for task in tasks {
        for blocker in task.blockers {
            graph
                .entry(task.id.as_str().to_string())
                .or_default()
                .insert(blocker.as_str().to_string());
        }
    }
    graph
        .entry(child.as_str().to_string())
        .or_default()
        .insert(parent.as_str().to_string());

    let mut seen = HashSet::new();
    let mut queue = VecDeque::from([parent.as_str().to_string()]);
    while let Some(node) = queue.pop_front() {
        if node == child.as_str() {
            return Ok(true);
        }
        if !seen.insert(node.clone()) {
            continue;
        }
        if let Some(nexts) = graph.get(&node) {
            queue.extend(nexts.iter().cloned());
        }
    }

    Ok(false)
}
