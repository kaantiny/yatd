---
name: managing-tasks-with-td
description: Manages tasks with the td CLI. Use when tracking work items, creating todos, managing task dependencies, or when the user mentions td, tasks, or todos in a project using td. When the project obviously uses something else, or the user doesn't mention td explicitly, do not read.
---

```bash
# New work — title should stand on its own a year from now
td create "panic in token_refresh when OAuth provider returns HTTP 429" \
  -p 1 -t bug -d "$(cat <<'DESC'
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
  -t feature -d "$(cat <<'DESC'
smtp::send() opens a plaintext socket and never upgrades. Per RFC 3207,
send EHLO, check for STARTTLS capability, then upgrade before AUTH.

rustls is already a dependency (used by the HTTP client), so the TLS
upgrade should be straightforward.

Relevant: src/smtp/transport.rs SmtpTransport::connect(),
          src/smtp/transport.rs:52 (socket open)
DESC
)"

td create "Flaky: test_concurrent_writes times out ~1/5 CI runs" \
  -p 3 -t bug -l ci,flaky -d "$(cat <<'DESC'
Passes locally, times out on CI. Likely a race on the shared tempdir —
each spawn should use its own database file.

CI: https://builds.example.org/job/1284
Relevant: tests/db_stress.rs:88, db::open()
DESC
)"

td create "Child task" --parent td-a1b2c3 # ID becomes <parent>.N
# -p priority: 1=high 2=medium 3=low  -t type  -d desc  -l labels (csv)

# What's on the board?
td list # all tasks
td list -s open # by status: open, in_progress, closed
td list -p 1 # high-priority only
td list -l frontend # by label

# Full context on a task
td show td-a1b2c3

# Task status or details changed
td update td-a1b2c3 -s in_progress
td update td-a1b2c3 -p 1 -t "Revised title" -d "Added context"

# Finished or needs reopening
td done td-a1b2c3 td-d4e5f6 # one or many
td reopen td-a1b2c3

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
```
