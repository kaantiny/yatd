---
name: td-review
description: Evaluate if a task is ready for execution or needs more planning. Use after running td next or when the user wants to assess task readiness before starting work.
license: MIT
---

# td-review — Task Readiness Check

Evaluate whether the top task from `td next` is ready to execute or needs more planning.

## When to Use

- After `td next` to check the recommended task
- User asks "is this ready?"
- User asks "review this task"

## Ready Criteria

A task is **READY** when:
- Title is specific and actionable
- Description includes clear acceptance criteria
- Edge cases are identified or explicitly out of scope
- Dependencies are clear (blockers resolved or listed)
- No placeholders like "TODO" or "figure out later"

A task **NEEDS WORK** when:
- Scope is vague (e.g., "improve auth")
- Missing acceptance criteria
- Contains undecided questions

## Steps

1. Get the next task:
   ```bash
   td next -n 1
   ```

2. Read the task details:
   ```bash
   td show {id}
   ```

3. Evaluate against criteria above.

4. Report decision:
   - **READY**: "Task is ready. Run 'let's get started on {id}' to execute."
   - **NEEDS WORK**: "Task needs planning: [specific gaps]. Run 'let's plan out {id}' to refine."

## Notes

- This is a gate: READY → td-execute, NEEDS WORK → td-spec
- Be honest — better to plan more than execute poorly
- User may override your assessment
