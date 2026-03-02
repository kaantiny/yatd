//! Diagnose and repair CRDT document integrity.
//!
//! Detects dangling references, dependency cycles, and parent-chain
//! cycles that can arise from concurrent CRDT edits across peers.
//! With `--fix`, applies deterministic, non-destructive repairs.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::db;

/// Categories of integrity issues that doctor can detect.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum FindingKind {
    DanglingParent,
    DanglingBlocker,
    BlockerCycle,
    ParentCycle,
}

/// A single integrity issue found during diagnosis.
#[derive(Debug, Clone, Serialize)]
struct Finding {
    kind: FindingKind,
    /// Full ULID of the primarily affected task.
    task: String,
    /// Human-readable description of the issue.
    detail: String,
    /// Whether this finding affects runtime behavior.  Blocker cycles
    /// involving closed or tombstoned tasks are inert because
    /// `partition_blockers` already treats them as resolved.
    active: bool,
    /// Whether `--fix` repaired this finding.
    fixed: bool,
}

/// Aggregate counts for the doctor report.
#[derive(Debug, Clone, Default, Serialize)]
struct Summary {
    dangling_parents: usize,
    dangling_blockers: usize,
    blocker_cycles: usize,
    parent_cycles: usize,
    total: usize,
    fixed: usize,
}

/// Full doctor report, serialised as JSON when `--json` is passed.
#[derive(Debug, Clone, Serialize)]
struct Report {
    findings: Vec<Finding>,
    summary: Summary,
}

/// Repair action to apply when `--fix` is requested.
enum Repair {
    ClearParent(db::TaskId),
    RemoveBlocker(db::TaskId, db::TaskId),
}

pub fn run(root: &Path, fix: bool, json: bool) -> Result<()> {
    let store = db::open(root)?;
    let tasks = store.list_tasks_unfiltered()?;

    // Build lookup structures.
    let all_ids: HashSet<String> = tasks.iter().map(|t| t.id.as_str().to_string()).collect();
    let open_ids: HashSet<String> = tasks
        .iter()
        .filter(|t| t.status != db::Status::Closed && t.deleted_at.is_none())
        .map(|t| t.id.as_str().to_string())
        .collect();

    let mut findings: Vec<Finding> = Vec::new();
    let mut repairs: Vec<Repair> = Vec::new();

    check_dangling_parents(&tasks, &all_ids, &mut findings, &mut repairs);
    check_dangling_blockers(&tasks, &all_ids, &mut findings, &mut repairs);
    check_blocker_cycles(&tasks, &all_ids, &open_ids, &mut findings, &mut repairs)?;
    check_parent_cycles(&tasks, &all_ids, &mut findings, &mut repairs)?;

    if fix && !repairs.is_empty() {
        store.apply_and_persist(|doc| {
            let tasks_map = doc.get_map("tasks");
            let ts = db::now_utc();

            for repair in &repairs {
                match repair {
                    Repair::ClearParent(task_id) => {
                        let task = db::get_task_map(&tasks_map, task_id)?
                            .ok_or_else(|| anyhow!("task {} not found during repair", task_id))?;
                        task.insert("parent", "")?;
                        task.insert("updated_at", ts.clone())?;
                    }
                    Repair::RemoveBlocker(task_id, blocker_id) => {
                        let task = db::get_task_map(&tasks_map, task_id)?
                            .ok_or_else(|| anyhow!("task {} not found during repair", task_id))?;
                        let blockers = db::get_or_create_child_map(&task, "blockers")?;
                        blockers.delete(blocker_id.as_str())?;
                        task.insert("updated_at", ts.clone())?;
                    }
                }
            }
            Ok(())
        })?;

        for finding in &mut findings {
            if finding.active {
                finding.fixed = true;
            }
        }
    }

    let summary = build_summary(&findings);
    let report = Report { findings, summary };

    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print_human(&report, fix);
    }

    Ok(())
}

/// Detect parent fields that reference missing or tombstoned tasks.
///
/// Skips tombstoned tasks — their stale references are not actionable.
fn check_dangling_parents(
    tasks: &[db::Task],
    all_ids: &HashSet<String>,
    findings: &mut Vec<Finding>,
    repairs: &mut Vec<Repair>,
) {
    let task_map: std::collections::HashMap<&str, &db::Task> =
        tasks.iter().map(|t| (t.id.as_str(), t)).collect();

    for task in tasks {
        // Tombstoned tasks are already deleted; stale refs on them aren't actionable.
        if task.deleted_at.is_some() {
            continue;
        }

        let Some(parent) = &task.parent else {
            continue;
        };

        if !all_ids.contains(parent.as_str()) {
            findings.push(Finding {
                kind: FindingKind::DanglingParent,
                task: task.id.as_str().to_string(),
                detail: format!(
                    "parent references missing task {}",
                    db::TaskId::display_id(parent.as_str()),
                ),
                active: true,
                fixed: false,
            });
            repairs.push(Repair::ClearParent(task.id.clone()));
            continue;
        }

        // Parent exists but is tombstoned — the subtask is orphaned.
        if let Some(pt) = task_map.get(parent.as_str()) {
            if pt.deleted_at.is_some() {
                findings.push(Finding {
                    kind: FindingKind::DanglingParent,
                    task: task.id.as_str().to_string(),
                    detail: format!(
                        "parent references tombstoned task {}",
                        db::TaskId::display_id(parent.as_str()),
                    ),
                    active: true,
                    fixed: false,
                });
                repairs.push(Repair::ClearParent(task.id.clone()));
            }
        }
    }
}

/// Detect blocker entries that reference tasks not present in the document.
///
/// Skips tombstoned tasks — their stale references are not actionable.
fn check_dangling_blockers(
    tasks: &[db::Task],
    all_ids: &HashSet<String>,
    findings: &mut Vec<Finding>,
    repairs: &mut Vec<Repair>,
) {
    for task in tasks {
        if task.deleted_at.is_some() {
            continue;
        }

        for blocker in &task.blockers {
            if !all_ids.contains(blocker.as_str()) {
                findings.push(Finding {
                    kind: FindingKind::DanglingBlocker,
                    task: task.id.as_str().to_string(),
                    detail: format!(
                        "blocker references missing task {}",
                        db::TaskId::display_id(blocker.as_str()),
                    ),
                    active: true,
                    fixed: false,
                });
                repairs.push(Repair::RemoveBlocker(task.id.clone(), blocker.clone()));
            }
        }
    }
}

/// Detect cycles in the blocker graph.
///
/// Iteratively finds one cycle at a time, records the deterministic
/// edge to break (lowest blocker ULID in the cycle), removes that edge
/// from the working graph, and repeats until acyclic.
///
/// Cycles where every node is open are "active" — they trap tasks out
/// of `ready`/`next`.  Cycles containing any closed or tombstoned node
/// are "inert" — `partition_blockers` already resolves them at runtime.
/// Only active cycles are repaired by `--fix`.
fn check_blocker_cycles(
    tasks: &[db::Task],
    all_ids: &HashSet<String>,
    open_ids: &HashSet<String>,
    findings: &mut Vec<Finding>,
    repairs: &mut Vec<Repair>,
) -> Result<()> {
    // Build adjacency: task → set of blockers (only edges where both
    // endpoints exist, to avoid mixing up dangling-ref findings).
    let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
    for task in tasks {
        for blocker in &task.blockers {
            if all_ids.contains(blocker.as_str()) {
                graph
                    .entry(task.id.as_str().to_string())
                    .or_default()
                    .insert(blocker.as_str().to_string());
            }
        }
    }

    loop {
        let Some(cycle) = find_cycle_dfs(&graph) else {
            break;
        };

        // Determine which edge to break: the one whose blocker ULID
        // is lexicographically lowest.  Edges in the cycle are
        // (cycle[0] → cycle[1]), (cycle[1] → cycle[2]), etc.
        let mut best: Option<(String, String)> = None;
        for pair in cycle.windows(2) {
            let blocker = &pair[1];
            if best.as_ref().is_none_or(|(_, b)| blocker < b) {
                best = Some((pair[0].clone(), blocker.clone()));
            }
        }
        let (task_id, blocker_id) = best.expect("cycle must have at least one edge");

        let active = cycle[..cycle.len() - 1]
            .iter()
            .all(|id| open_ids.contains(id));

        // Build a human-readable cycle string using short IDs.
        let display: Vec<String> = cycle.iter().map(|id| db::TaskId::display_id(id)).collect();
        let cycle_str = display.join(" → ");

        findings.push(Finding {
            kind: FindingKind::BlockerCycle,
            task: task_id.clone(),
            detail: cycle_str,
            active,
            fixed: false,
        });

        if active {
            repairs.push(Repair::RemoveBlocker(
                db::TaskId::parse(&task_id)?,
                db::TaskId::parse(&blocker_id)?,
            ));
        }

        // Remove the edge from the working graph so the next iteration
        // can find further cycles (or terminate).
        if let Some(set) = graph.get_mut(&task_id) {
            set.remove(&blocker_id);
        }
    }

    Ok(())
}

/// Detect cycles in the parent-child hierarchy.
///
/// Follows each task's parent chain; if we revisit a task already in the
/// current path, we have a cycle.  Repairs clear the parent field on the
/// task with the lexicographically lowest ULID in the cycle.
fn check_parent_cycles(
    tasks: &[db::Task],
    all_ids: &HashSet<String>,
    findings: &mut Vec<Finding>,
    repairs: &mut Vec<Repair>,
) -> Result<()> {
    let parent_map: HashMap<String, String> = tasks
        .iter()
        .filter_map(|t| {
            t.parent.as_ref().and_then(|p| {
                if all_ids.contains(p.as_str()) {
                    Some((t.id.as_str().to_string(), p.as_str().to_string()))
                } else {
                    None // dangling parents handled separately
                }
            })
        })
        .collect();

    let mut globally_visited: HashSet<String> = HashSet::new();

    for task in tasks {
        let start = task.id.as_str().to_string();
        if globally_visited.contains(&start) {
            continue;
        }

        let mut path: Vec<String> = Vec::new();
        let mut path_set: HashSet<String> = HashSet::new();
        let mut current = Some(start.clone());

        while let Some(node) = current {
            if path_set.contains(&node) {
                // Found a cycle.  Extract just the cycle portion.
                let pos = path.iter().position(|n| *n == node).unwrap();
                let mut cycle: Vec<String> = path[pos..].to_vec();
                cycle.push(node); // close the loop

                let display: Vec<String> =
                    cycle.iter().map(|id| db::TaskId::display_id(id)).collect();
                let cycle_str = display.join(" → ");

                // The task with the lowest ULID gets its parent cleared.
                let lowest = cycle[..cycle.len() - 1]
                    .iter()
                    .min()
                    .expect("cycle must have at least one node")
                    .clone();

                findings.push(Finding {
                    kind: FindingKind::ParentCycle,
                    task: lowest.clone(),
                    detail: cycle_str,
                    active: true,
                    fixed: false,
                });
                repairs.push(Repair::ClearParent(db::TaskId::parse(&lowest)?));
                break;
            }
            if globally_visited.contains(&node) {
                break;
            }
            path_set.insert(node.clone());
            path.push(node.clone());
            current = parent_map.get(&node).cloned();
        }

        for node in &path {
            globally_visited.insert(node.clone());
        }
    }

    Ok(())
}

/// Find a single cycle in a directed graph using DFS.
///
/// Returns the cycle as a vec of node IDs where the first and last
/// elements are the same (e.g. `[A, B, C, A]`), or `None` if acyclic.
fn find_cycle_dfs(graph: &HashMap<String, HashSet<String>>) -> Option<Vec<String>> {
    let mut visited: HashSet<String> = HashSet::new();

    // Sort keys for deterministic traversal order.
    let mut nodes: Vec<&String> = graph.keys().collect();
    nodes.sort();

    for start in nodes {
        if visited.contains(start) {
            continue;
        }
        let mut path: Vec<String> = Vec::new();
        let mut path_set: HashSet<String> = HashSet::new();
        if let Some(cycle) = dfs_visit(start, graph, &mut visited, &mut path, &mut path_set) {
            return Some(cycle);
        }
    }
    None
}

/// Recursive DFS helper for cycle detection.
fn dfs_visit(
    node: &str,
    graph: &HashMap<String, HashSet<String>>,
    visited: &mut HashSet<String>,
    path: &mut Vec<String>,
    path_set: &mut HashSet<String>,
) -> Option<Vec<String>> {
    visited.insert(node.to_string());
    path_set.insert(node.to_string());
    path.push(node.to_string());

    if let Some(neighbors) = graph.get(node) {
        // Sort neighbors for deterministic cycle detection.
        let mut sorted: Vec<&String> = neighbors.iter().collect();
        sorted.sort();

        for neighbor in sorted {
            if path_set.contains(neighbor.as_str()) {
                let pos = path.iter().position(|n| n == neighbor).unwrap();
                let mut cycle = path[pos..].to_vec();
                cycle.push(neighbor.clone());
                return Some(cycle);
            }
            if !visited.contains(neighbor.as_str()) {
                if let Some(cycle) = dfs_visit(neighbor, graph, visited, path, path_set) {
                    return Some(cycle);
                }
            }
        }
    }

    path_set.remove(node);
    path.pop();
    None
}

fn build_summary(findings: &[Finding]) -> Summary {
    let mut s = Summary::default();
    for f in findings {
        match f.kind {
            FindingKind::DanglingParent => s.dangling_parents += 1,
            FindingKind::DanglingBlocker => s.dangling_blockers += 1,
            FindingKind::BlockerCycle => s.blocker_cycles += 1,
            FindingKind::ParentCycle => s.parent_cycles += 1,
        }
        s.total += 1;
        if f.fixed {
            s.fixed += 1;
        }
    }
    s
}

fn print_human(report: &Report, fix: bool) {
    let c = crate::color::stderr_theme();

    if report.findings.is_empty() {
        eprintln!("{}info:{} no issues found", c.green, c.reset);
        return;
    }

    for f in &report.findings {
        let short = db::TaskId::display_id(&f.task);
        let kind_label = match f.kind {
            FindingKind::DanglingParent => "dangling parent",
            FindingKind::DanglingBlocker => "dangling blocker",
            FindingKind::BlockerCycle => {
                if f.active {
                    "blocker cycle (active)"
                } else {
                    "blocker cycle (inert)"
                }
            }
            FindingKind::ParentCycle => "parent cycle",
        };

        if f.fixed {
            eprintln!(
                "{}fixed:{} {}: {} — {}",
                c.green, c.reset, kind_label, short, f.detail
            );
        } else {
            eprintln!(
                "{}issue:{} {}: {} — {}",
                c.yellow, c.reset, kind_label, short, f.detail
            );
        }
    }

    eprintln!();
    let n = report.summary.total;
    let issue_word = if n == 1 { "issue" } else { "issues" };
    if fix {
        eprintln!(
            "{} {issue_word} found, {} fixed",
            report.summary.total, report.summary.fixed,
        );
    } else {
        eprintln!("{n} {issue_word} found. Run with --fix to repair.");
    }
}
