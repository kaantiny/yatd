---
name: td-spec
description: Deep-dive on a single task to fill gaps, clarify scope, add implementation notes, and log planning decisions. Use when td-review indicates a task needs work or the user says "let's plan out".
license: MIT
---

# td-spec — Task Specification

Add detail to a vague task through interview and exploration.

## When to Use

- td-review says a task "needs work"
- User says "let's plan out {id}"
- User says "spec out {id}" or "refine {id}"

## Steps

1. Read the task:
   ```bash
   td show {id}
   ```

2. Identify gaps:
   - Unclear scope?
   - Missing acceptance criteria?
   - Undecided approach?
   - Unknown edge cases?

3. Interview the user to fill gaps.

4. Update the task:
   ```bash
   td update {id} -d "$(cat <<'EOF'
   ## Acceptance Criteria
   - [ ] ...

   ## Implementation Notes
   - ...

   ## Edge Cases
   - ...
   EOF
   )"
   ```

5. Log all decisions:
   ```bash
   td log {id} "Decision: ..."
   td log {id} "Approach: ..."
   ```

## Iteration

You may cycle through td-spec → td-review multiple times. Each pass adds more detail until the task is ready.

## Next Step

After spec'ing, run td-review again to confirm readiness, then proceed to td-execute.
