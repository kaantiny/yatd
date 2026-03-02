//! Scoring engine for the `next` command.
//!
//! Scores ready tasks (open, no open blockers) using priority, effort,
//! and the transitive downstream impact through the blocker graph.

use std::collections::{HashMap, HashSet, VecDeque};

/// Scoring mode for ranking tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Critical path: downstream impact dominates, effort is dampened.
    ///
    /// `score = (downstream + 1.0) × priority_weight / effort_weight^0.25`
    Impact,

    /// Effort-weighted: effort dominates, downstream is dampened.
    ///
    /// `score = (downstream × 0.25 + 1.0) × priority_weight / effort_weight²`
    Effort,
}

/// Input task supplied by callers to [`rank`].
///
/// Separates the label strings (for output) from the integer scores (for
/// computation), so callers don't have to reverse-engineer labels from ints.
#[derive(Debug, Clone)]
pub struct TaskInput {
    pub id: String,
    pub title: String,
    /// Integer score used for weighting (1=high, 2=medium, 3=low for priority).
    pub priority_score: i32,
    /// Integer score used for weighting (1=low, 2=medium, 3=high for effort).
    pub effort_score: i32,
    /// Human-readable label, e.g. `"high"`, `"medium"`, `"low"`.
    pub priority_label: String,
    /// Human-readable label, e.g. `"low"`, `"medium"`, `"high"`.
    pub effort_label: String,
}

/// A lightweight snapshot of a task for scoring purposes.
#[derive(Debug, Clone)]
pub struct TaskNode {
    pub id: String,
    pub priority: i32,
    pub effort: i32,
}

/// Scored result for a single task.
#[derive(Debug, Clone)]
pub struct ScoredTask {
    pub id: String,
    pub title: String,
    /// Original priority label (`"high"` / `"medium"` / `"low"`).
    pub priority: String,
    /// Original effort label (`"low"` / `"medium"` / `"high"`).
    pub effort: String,
    pub score: f64,
    pub downstream_score: f64,
    pub priority_weight: f64,
    pub effort_weight: f64,
    /// Total number of open tasks transitively blocked by this one.
    pub total_unblocked: usize,
    /// Number of tasks directly blocked by this one.
    pub direct_unblocked: usize,
}

/// Convert DB priority (1=high, 2=medium, 3=low) to a scoring weight
/// where higher priority = higher weight.
fn priority_weight(p: i32) -> f64 {
    match p {
        1 => 3.0,
        2 => 2.0,
        3 => 1.0,
        _ => 2.0,
    }
}

/// Convert DB effort (1=low, 2=medium, 3=high) to a scoring weight.
fn effort_weight(e: i32) -> f64 {
    match e {
        1 => 1.0,
        2 => 2.0,
        3 => 3.0,
        _ => 2.0,
    }
}

/// Compute the transitive downstream score and count for a given task.
///
/// Walks the `blocks` adjacency list (task → set of tasks it blocks)
/// starting from `start`, summing priority weights of all reachable nodes.
/// Uses a visited set to handle any residual cycles defensively.
fn downstream(
    start: &str,
    blocks: &HashMap<String, HashSet<String>>,
    nodes: &HashMap<String, TaskNode>,
) -> (f64, usize, usize) {
    let mut score = 0.0;
    let mut total = 0usize;
    let mut direct = 0usize;

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start.to_string());

    if let Some(dependents) = blocks.get(start) {
        for dep in dependents {
            if visited.insert(dep.clone()) {
                queue.push_back(dep.clone());
                direct += 1;
            }
        }
    }

    while let Some(current) = queue.pop_front() {
        total += 1;
        if let Some(node) = nodes.get(&current) {
            score += priority_weight(node.priority);
        }
        if let Some(dependents) = blocks.get(&current) {
            for dep in dependents {
                if visited.insert(dep.clone()) {
                    queue.push_back(dep.clone());
                }
            }
        }
    }

    (score, total, direct)
}

/// Score and rank ready tasks.
///
/// # Arguments
/// * `open_tasks` — all open tasks, including both scoring integers and display labels
/// * `blocker_edges` — `(task_id, blocker_id)` pairs among open tasks
/// * `exclude` — task IDs to exclude from candidates (still counted in
///   downstream scores). Used to filter parent tasks that have open subtasks.
/// * `mode` — scoring strategy
/// * `limit` — maximum number of results to return
pub fn rank(
    open_tasks: &[TaskInput],
    blocker_edges: &[(String, String)],
    exclude: &HashSet<String>,
    mode: Mode,
    limit: usize,
) -> Vec<ScoredTask> {
    // Build node map and label lookup.
    let mut nodes: HashMap<String, TaskNode> = HashMap::new();
    let mut titles: HashMap<String, String> = HashMap::new();
    let mut priority_labels: HashMap<String, String> = HashMap::new();
    let mut effort_labels: HashMap<String, String> = HashMap::new();
    for t in open_tasks {
        nodes.insert(
            t.id.clone(),
            TaskNode {
                id: t.id.clone(),
                priority: t.priority_score,
                effort: t.effort_score,
            },
        );
        titles.insert(t.id.clone(), t.title.clone());
        priority_labels.insert(t.id.clone(), t.priority_label.clone());
        effort_labels.insert(t.id.clone(), t.effort_label.clone());
    }

    // Build adjacency lists.
    // blocked_by: task_id → set of blocker_ids (who blocks this task)
    // blocks: blocker_id → set of task_ids (who this task blocks)
    let mut blocked_by: HashMap<String, HashSet<String>> = HashMap::new();
    let mut blocks: HashMap<String, HashSet<String>> = HashMap::new();

    for (task_id, blocker_id) in blocker_edges {
        // Only include edges where both ends are open tasks.
        if nodes.contains_key(task_id) && nodes.contains_key(blocker_id) {
            blocked_by
                .entry(task_id.clone())
                .or_default()
                .insert(blocker_id.clone());
            blocks
                .entry(blocker_id.clone())
                .or_default()
                .insert(task_id.clone());
        }
    }

    // Find ready tasks: open tasks with no open blockers, excluding
    // parent tasks that still have open subtasks.
    let ready: Vec<&TaskNode> = nodes
        .values()
        .filter(|n| !blocked_by.contains_key(&n.id) && !exclude.contains(&n.id))
        .collect();

    // Score each ready task.
    let mut scored: Vec<ScoredTask> = ready
        .iter()
        .map(|node| {
            let (ds, total_unblocked, direct_unblocked) = downstream(&node.id, &blocks, &nodes);
            let pw = priority_weight(node.priority);
            let ew = effort_weight(node.effort);

            let score = match mode {
                Mode::Impact => (ds + 1.0) * pw / ew.powf(0.25),
                Mode::Effort => (ds * 0.25 + 1.0) * pw / (ew * ew),
            };

            ScoredTask {
                id: node.id.clone(),
                title: titles.get(&node.id).cloned().unwrap_or_default(),
                priority: priority_labels.get(&node.id).cloned().unwrap_or_default(),
                effort: effort_labels.get(&node.id).cloned().unwrap_or_default(),
                score,
                downstream_score: ds,
                priority_weight: pw,
                effort_weight: ew,
                total_unblocked,
                direct_unblocked,
            }
        })
        .collect();

    // Sort descending by score, then by id for stability.
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.id.cmp(&b.id))
    });

    scored.truncate(limit);
    scored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, title: &str, pri: i32, eff: i32) -> TaskInput {
        // Map integer scores back to labels for test convenience.
        let priority_label = match pri {
            1 => "high",
            2 => "medium",
            _ => "low",
        };
        let effort_label = match eff {
            1 => "low",
            2 => "medium",
            _ => "high",
        };
        TaskInput {
            id: id.to_string(),
            title: title.to_string(),
            priority_score: pri,
            effort_score: eff,
            priority_label: priority_label.to_string(),
            effort_label: effort_label.to_string(),
        }
    }

    fn edge(task_id: &str, blocker_id: &str) -> (String, String) {
        (task_id.to_string(), blocker_id.to_string())
    }

    #[test]
    fn single_task_no_deps() {
        let tasks = vec![task("a", "Alpha", 1, 1)];
        let result = rank(&tasks, &[], &HashSet::new(), Mode::Impact, 5);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "a");
        // (0 + 1.0) * 3.0 / 1.0^0.25 = 3.0
        assert!((result[0].score - 3.0).abs() < f64::EPSILON);
        assert_eq!(result[0].total_unblocked, 0);
        assert_eq!(result[0].direct_unblocked, 0);
    }

    #[test]
    fn blocker_scores_higher_than_leaf() {
        // A blocks B. Both are ready-eligible but only A has no blockers.
        // A is ready (blocks B), B is blocked.
        let tasks = vec![task("a", "Blocker", 2, 2), task("b", "Blocked", 1, 1)];
        let edges = vec![edge("b", "a")];
        let result = rank(&tasks, &edges, &HashSet::new(), Mode::Impact, 5);

        // Only A is ready.
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "a");
        // downstream of A = priority_weight(B) = 3.0
        // score = (3.0 + 1.0) * 2.0 / 2.0^0.25 = 8.0 / 1.1892 ≈ 6.727
        let expected = (3.0 + 1.0) * 2.0 / 2.0_f64.powf(0.25);
        assert!((result[0].score - expected).abs() < 1e-10);
        assert_eq!(result[0].total_unblocked, 1);
        assert_eq!(result[0].direct_unblocked, 1);
    }

    #[test]
    fn transitive_downstream_counted() {
        // A blocks B, B blocks C. Only A is ready.
        let tasks = vec![
            task("a", "Root", 2, 2),
            task("b", "Mid", 2, 2),
            task("c", "Leaf", 1, 1),
        ];
        let edges = vec![edge("b", "a"), edge("c", "b")];
        let result = rank(&tasks, &edges, &HashSet::new(), Mode::Impact, 5);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "a");
        // downstream = pw(b) + pw(c) = 2.0 + 3.0 = 5.0
        // score = (5.0 + 1.0) * 2.0 / 2.0^0.25
        let expected = (5.0 + 1.0) * 2.0 / 2.0_f64.powf(0.25);
        assert!((result[0].score - expected).abs() < 1e-10);
        assert_eq!(result[0].total_unblocked, 2);
        assert_eq!(result[0].direct_unblocked, 1);
    }

    #[test]
    fn diamond_graph_no_double_counting() {
        // A and B both block C. A and B are ready.
        let tasks = vec![
            task("a", "Left", 1, 1),
            task("b", "Right", 2, 2),
            task("c", "Sink", 1, 1),
        ];
        let edges = vec![edge("c", "a"), edge("c", "b")];
        let result = rank(&tasks, &edges, &HashSet::new(), Mode::Impact, 5);

        assert_eq!(result.len(), 2);
        // Both A and B see C as downstream.
        // A: downstream = pw(c) = 3.0, score = (3+1)*3/1^0.25 = 12.0
        // B: downstream = pw(c) = 3.0, score = (3+1)*2/2^0.25
        assert_eq!(result[0].id, "a");
        assert!((result[0].score - 12.0).abs() < f64::EPSILON);
        let expected_b = (3.0 + 1.0) * 2.0 / 2.0_f64.powf(0.25);
        assert_eq!(result[1].id, "b");
        assert!((result[1].score - expected_b).abs() < 1e-10);
    }

    #[test]
    fn effort_mode_dampens_downstream() {
        // A blocks B. A is ready.
        let tasks = vec![
            task("a", "Blocker", 1, 3), // high pri, high effort
            task("b", "Blocked", 1, 1),
        ];
        let edges = vec![edge("b", "a")];

        let impact = rank(&tasks, &edges, &HashSet::new(), Mode::Impact, 5);
        let effort = rank(&tasks, &edges, &HashSet::new(), Mode::Effort, 5);

        // Impact: (3.0 + 1.0) * 3.0 / 3.0^0.25
        let expected_impact = (3.0 + 1.0) * 3.0 / 3.0_f64.powf(0.25);
        assert!((impact[0].score - expected_impact).abs() < 1e-10);
        // Effort: (3.0 * 0.25 + 1.0) * 3.0 / 9.0 = 1.75 * 3.0 / 9.0 ≈ 0.583
        assert!((effort[0].score - (1.75 * 3.0 / 9.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn effort_mode_prefers_low_effort() {
        // Two standalone tasks: A is high-effort, B is low-effort. Same priority.
        let tasks = vec![task("a", "Heavy", 2, 3), task("b", "Light", 2, 1)];
        let result = rank(&tasks, &[], &HashSet::new(), Mode::Effort, 5);

        assert_eq!(result.len(), 2);
        // B should rank higher (low effort).
        assert_eq!(result[0].id, "b");
        assert_eq!(result[1].id, "a");
    }

    #[test]
    fn limit_truncates() {
        let tasks = vec![
            task("a", "A", 1, 1),
            task("b", "B", 2, 2),
            task("c", "C", 3, 3),
        ];
        let result = rank(&tasks, &[], &HashSet::new(), Mode::Impact, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn empty_input() {
        let result = rank(&[], &[], &HashSet::new(), Mode::Impact, 5);
        assert!(result.is_empty());
    }

    #[test]
    fn stable_sort_by_id() {
        // Two tasks with identical scores should sort by id.
        let tasks = vec![task("b", "Second", 2, 2), task("a", "First", 2, 2)];
        let result = rank(&tasks, &[], &HashSet::new(), Mode::Impact, 5);
        assert_eq!(result[0].id, "a");
        assert_eq!(result[1].id, "b");
    }

    #[test]
    fn excluded_tasks_not_candidates() {
        // A and B are both standalone ready tasks, but A is excluded
        // (simulating a parent with open subtasks).
        let tasks = vec![task("a", "Parent", 1, 1), task("b", "Leaf", 2, 2)];
        let exclude: HashSet<String> = ["a".to_string()].into();
        let result = rank(&tasks, &[], &exclude, Mode::Impact, 5);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "b");
    }

    #[test]
    fn excluded_task_still_counted_in_downstream() {
        // A blocks B (the excluded parent). B blocks C.
        // A is ready. B is excluded but its downstream weight should still
        // flow through the graph — A should see both B and C downstream.
        let tasks = vec![
            task("a", "Root", 2, 2),
            task("b", "Parent", 1, 1),
            task("c", "Leaf", 2, 2),
        ];
        let edges = vec![edge("b", "a"), edge("c", "b")];
        let exclude: HashSet<String> = ["b".to_string()].into();
        let result = rank(&tasks, &edges, &exclude, Mode::Impact, 5);

        // Only A is ready (B is blocked and also excluded, C is blocked).
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "a");
        // Downstream still includes B and C.
        assert_eq!(result[0].total_unblocked, 2);
    }

    #[test]
    fn impact_prefers_priority_over_effort_for_standalone() {
        // Regression: high-pri/high-eff must rank above medium-pri/low-eff
        // in impact mode. Previously effort canceled priority entirely.
        let tasks = vec![
            task("a", "HighPriHighEff", 1, 3),
            task("b", "MedPriLowEff", 2, 1),
        ];
        let result = rank(&tasks, &[], &HashSet::new(), Mode::Impact, 5);
        assert_eq!(result[0].id, "a");
    }

    #[test]
    fn parent_with_all_children_closed_remains_candidate() {
        // A standalone task that would be in the open_tasks list but NOT
        // in the exclude set (because all its children are closed, the
        // caller wouldn't put it there). Verify it's still a candidate.
        let tasks = vec![task("a", "Parent done kids", 1, 1)];
        let result = rank(&tasks, &[], &HashSet::new(), Mode::Impact, 5);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "a");
    }
}
