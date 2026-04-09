---
name: td-execute
description: Begin execution on a ready task by exploring the codebase, identifying relevant files, and optionally scoping child tasks. Use when td-review confirms a task is ready or the user says "let's get started on".
license: MIT
---

# td-execute — Task Execution

Start work on a ready task: explore code, identify touch points, begin implementation.

## When to Use

- td-review says task is "ready"
- User says "let's get started on {id}"
- User says "begin work on {id}"

## Steps

1. Read the task:
   ```bash
   td show {id}
   ```

2. Mark as in progress:
   ```bash
   td update {id} -s in_progress
   ```

3. Explore the codebase:
   - Find files mentioned in the task
   - Identify related modules/components
   - Check existing patterns and tests

4. Log initial findings:
   ```bash
   td log {id} "Relevant files: ..."
   td log {id} "Pattern to follow: ..."
   td log {id} "Potential issue: ..."
   ```

5. Decide: direct work or further decomposition?

   **Direct work**: Start implementing if the task is small/clear.

   **Create child tasks**: If the task is large:
   ```bash
   td create "Child: do X" --parent {id}
   td create "Child: do Y" --parent {id}
   td done {id}  # close parent, work on children
   ```

## Logging

Log frequently during execution:
```bash
td log {id} "Discovered: code does X, not Y"
td log {id} "User preference: approach Z"
td log {id} "Constraint: must use pattern W"
```

This is your scratchpad for handoffs.

## Completion

Mark done when finished:
```bash
td done {id}
```
