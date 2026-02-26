---
name: managing-tasks-with-td
description: Manages tasks with the td CLI. Use when tracking work items, creating todos, managing task dependencies, or when the user mentions td, tasks, or todos in a project using td. When the project obviously uses something else, or the user doesn't mention td explicitly, do not read.
---

```bash
# New work — title should stand on its own a year from now
# -p priority: low, medium (default), high
# -e effort: low, medium (default), high
# -t type  -d desc  -l labels (csv)
td create "panic in token_refresh when OAuth provider returns HTTP 429" \
  -p high -e medium -t bug -d "$(cat <<'DESC'
Reproduction:
1. Point OAuth at a rate-limiting provider (or stub with httpbin/status/429)
2. Let the access token expire
3. Trigger any authenticated request

Expected: graceful retry with exponential backoff
Actual: unwrap() panics on the error response body

auth::refresh_token() assumes every response is valid JSON. Matching on
the status code before parsing would prevent this.

Relevant: src/auth/refresh.rs:84 (the unwrap), src/auth/client.rs RetryPolicy
DESC
)"

td create "Add STARTTLS for outbound SMTP per RFC 3207" \
  -e high -t feature -d "$(cat <<'DESC'
smtp::send() opens a plaintext socket and never upgrades. Per RFC 3207,
send EHLO, check for STARTTLS capability, then upgrade before AUTH.

rustls is already a dependency (used by the HTTP client), so the TLS
upgrade should be straightforward.

Relevant: src/smtp/transport.rs SmtpTransport::connect(),
          src/smtp/transport.rs:52 (socket open)
DESC
)"

td create "Flaky: test_concurrent_writes times out ~1/5 CI runs" \
  -p low -e low -t bug -l ci,flaky -d "$(cat <<'DESC'
Passes locally, times out on CI. Likely a race on the shared tempdir —
each spawn should use its own database file.

CI: https://builds.example.org/job/1284
Relevant: tests/db_stress.rs:88, db::open()
DESC
)"

td create "Child task" --parent td-a1b2c3 # ID becomes <parent>.N

# What's on the board?
td list # all tasks
td list -s open # by status: open, in_progress, closed
td list -p high # high-priority only
td list -e low # low-effort tasks
td list -l frontend # by label

# Full context on a task
td show td-a1b2c3

# Append notes as you go, not just once at the end.
# Focus on why: what the user asked for, what decision was made, and why.
# Keep file paths as context, not the headline.
td log td-a1b2c3 "User asked for logs in show/export/import but not in list --json. Kept list untouched to avoid breaking existing scripts that parse its output."
td log td-a1b2c3 "User wanted task validation before insert. Added it so 'task not found' is explicit and immediate instead of a less obvious DB failure."
td log td-a1b2c3 "Kept ON DELETE CASCADE only on task_logs because the user scoped labels/blockers cascade changes to a separate task."
td log td-a1b2c3 "Stayed with positional message input (no stdin) because that was explicitly requested and is easier to replay from shell history."
td log td-a1b2c3 "Used timestamp+id ordering so handoff readers get a stable timeline even when two entries land in the same second."
td log td-a1b2c3 "Import replaces logs for the task to keep repeated imports deterministic and consistent with label/blocker import behavior."
td log td-a1b2c3 "CI failed only on migration version assertions after adding 0004; updated expected version and re-ran full suite to confirm no behavioral regressions."
td log td-a1b2c3 "Ran manual round-trip (init -> create -> log -> show -> export/import -> show) to prove logs survive transfer and stay in chronological order."

# Task status or details changed
td update td-a1b2c3 -s in_progress
td update td-a1b2c3 -p high -e low -t "Revised title" -d "Added context"

# Finished or needs reopening
td done td-a1b2c3 td-d4e5f6 # one or many
td reopen td-a1b2c3

# Delete tasks (always non-interactive)
td rm td-a1b2c3                           # delete one or many IDs
td rm td-a1b2c3 td-d4e5f6
td rm --recursive td-parent               # required for deleting task trees
td rm --force td-blocker                  # suppress dependent-unblocked warnings
td --json rm td-a1b2c3                    # machine-readable deleted/unblocked IDs

# Blocked by something else
td dep add td-child td-blocker # child waits for blocker to close
td dep rm td-child td-blocker
td dep tree td-parent # subtask tree

# Tag for cross-cutting concerns
td label add td-a1b2c3 urgent
td label rm td-a1b2c3 urgent
td label list td-a1b2c3
td label list-all

# What can be worked on right now?
td ready # open with all blockers resolved
td search "smtp" # substring match in title and description

# What should I work on next?
td next                    # top 5 by critical path (default)
td next --mode effort      # top 5 by effort-weighted scoring
td next --verbose          # show scoring breakdown per task
td next -n 3               # limit to top 3
td next --mode effort -v   # combine flags
```
