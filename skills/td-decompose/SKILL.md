---
name: td-decompose
description: Break a planned feature into the smallest units of work and create tasks with appropriate dependency relationships. Use when the user says "break this into tasks" or wants to decompose work with td.
license: MIT
---

# td-decompose — Task Decomposition

Break a feature into actionable, independent tasks with proper dependencies.

## When to Use

- User says "break this into tasks"
- User says "decompose [feature]"
- User says "create tasks from [plan]"

## Steps

1. Read the planning task:
   ```bash
   td show {plan-id}
   ```

2. Identify work units:
   - Each task should be completable in one sitting (30 min - 4 hours)
   - Clear acceptance criteria per task
   - Use imperative mood for titles ("Add...", "Fix...", "Refactor...")

3. Create tasks with dependencies:
   ```bash
   td create "Implement X" -p high --parent {plan-id}
   td create "Add Y support" -p medium --parent {plan-id}
   td dep add td-child td-blocker
   ```

4. Verify the task graph:
   ```bash
   td dep tree {plan-id}
   ```

5. Optionally close the planning task:
   ```bash
   td done {plan-id}
   ```

## Tips

- Task titles should stand alone (understandable a year from now)
- Dependencies enable `td next` to surface unblocked work
- Look for parallelization opportunities
