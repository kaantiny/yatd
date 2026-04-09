---
name: td-plan
description: Interview the user about a new feature, explore edge cases, constraints, and acceptance criteria. Use when the user says "let's think about" a feature or wants to plan something new with td. Creates a planning task and logs all decisions.
license: MIT
---

# td-plan — Feature Planning

Interview the user to flesh out a feature before breaking it into tasks.

## When to Use

- User says "let's think about [feature]"
- User says "I want to add [feature]"
- Starting a new feature that needs requirements gathering

## Steps

1. Create a planning task:
   ```bash
   td create "Plan: {feature name}" -p medium -t planning
   ```

2. Interview the user:
   - What problem does this solve?
   - What are the acceptance criteria?
   - What edge cases exist?
   - What constraints or dependencies?
   - What's explicitly out of scope?

3. Log decisions to the task:
   ```bash
   td log {id} "User decided: ..."
   td log {id} "Edge case: ..."
   td log {id} "Out of scope: ..."
   ```

4. Update the task description with a summary of the plan.

## Notes

- Keep asking questions until the feature feels "complete"
- Log the "why", not just the "what"
- Get explicit confirmation before moving to td-decompose
