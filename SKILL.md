---
name: managing-tasks-with-td
description: Manages tasks with the td CLI. Use when tracking work items, creating todos, managing task dependencies, or when the user mentions td, tasks, or todos in a project using td. When the project obviously uses something else, or the user doesn't mention td explicitly, do not read.
---

Don't forget the single quotes around `'EOF'` HEREDOCs; they disable shell
interpolation, preventing it from messing up Markdown in the description.

## Projects and initialisation

Storage is central (`~/.local/share/td/`), not per-directory. Each project
is a named Loro CRDT document. Directories are bound to projects via
`td init` or `td use`; the binding is resolved by longest-prefix match on
the canonical path. You can also override with `--project <name>` or the
`TD_PROJECT` env var.

For multi-machine setup, initialize only once. On the second machine, bootstrap
from the first with `td sync` instead of `td init` so both sides share the same
project identity.

```bash
td init myproject           # create project + bind cwd to it
td use myproject            # bind cwd to an existing project
td projects                 # list all known projects
td sync                     # machine A: print a wormhole code
td sync 7-goldfish-soccer   # machine B: bootstrap from machine A
td --project other list     # one-off override
TD_PROJECT=other td list    # env override
```

## General reference

These are not your most-used commands, nor are they the most critical, but you
still need reference for them. The sections following are more important than
this one and thus contain more instruction.

Task IDs are 26-character ULIDs. Most commands accept a unique prefix as
shorthand (e.g. the first 7 characters shown in table output). If the prefix
is ambiguous, td will tell you.

Deletion is a soft delete (sets `deleted_at` and marks the task closed).
Deleted tasks are hidden from `list`/`ready`/`next` but still visible to
`show` and included in `export`.

```bash
td rm <id> [<id>...]        # soft-delete one or many
td rm --recursive <parent>  # required for deleting task trees
td rm --force <blocker>     # suppress dependent-unblocked warnings
td list                     # all non-deleted tasks
td list -s open
td list -p high
td list -e low
td list -l frontend
td label add <id> frontend
td label rm <id> backend
td label list <id>
td label list-all
td show <id>                # read a task, its metadata, and logs
td ready                    # show open tasks with no active blockers
td search "smtp"            # substring match in title and description
```

## Creating tasks

Flags:

- -p priority: low, medium (default), high
- -e effort: low, medium (default), high
- -t type -d desc -l labels (csv)

Titles and bodies should stand on their own a year from now, not requiring
additional context to understand the nuances but including those nuances
directly at task creation or by adding them later through logs.

```bash
td create "Panic in token_refresh when OAuth provider returns HTTP 429" \
  -p high -e medium -t bug -d "$(cat <<'EOF'
Reproduction:
1. Point OAuth at a rate-limiting provider (or stub with httpbin/status/429)
2. Let the access token expire
3. Trigger any authenticated request

Expected: graceful retry with exponential backoff
Actual: unwrap() panics on the error response body

auth::refresh_token() assumes every response is valid JSON. Matching on
the status code before parsing would prevent this.

Relevant: src/auth/refresh.rs:84 (the unwrap), src/auth/client.rs RetryPolicy
EOF
)"

td create "Add STARTTLS for outbound SMTP per RFC 3207" \
  -e high -t feature -d "$(cat <<'EOF'
smtp::send() opens a plaintext socket and never upgrades. Per RFC 3207,
send EHLO, check for STARTTLS capability, then upgrade before AUTH.

rustls is already a dependency (used by the HTTP client), so the TLS
upgrade should be straightforward.

Relevant: src/smtp/transport.rs SmtpTransport::connect(),
          src/smtp/transport.rs:52 (socket open)
EOF
)"

td create "Flaky: test_concurrent_writes times out ~1/5 CI runs" \
  -p low -e low -t bug -l ci,flaky -d "$(cat <<'EOF'
Passes locally, times out on CI. Likely a race on the shared tempdir —
each spawn should use its own database file.

CI: https://builds.example.org/job/1284
Relevant: tests/db_stress.rs:88, db::open()
EOF
)"

td create "Child task" --parent td-a1b2c3 # for splitting tasks into smaller chunks
```

## Dependencies

These are a core part of td; use them liberally, but appropriately. The `next` and `ready` commands both filter tasks with blockers so you have an easier time ordering work. Blocker relationships also make obvious opportunities for parallel work. When there are opportunities for parallelisation, and you have a subagent tool with write capabilities, and the tasks are small, ask the user whether they want you to invoke subagents for them. If the tasks aren't quite scoped well enough yet, ask whether they would like you to interview/question them regarding the gaps that need filling. DO NOT parallelise work without EXPLICIT user confirmation.

```bash
td dep add td-child td-blocker # child waits for blocker to close
td dep rm td-child td-blocker
td dep tree td-parent # subtask tree
```

## Updating tasks

_As soon as_ it's obvious you're working on a particular task, mark it `in_progress`. If that was wrong, the user will say so.

```bash
td update td-a1b2c3 -s in_progress
td update td-a1b2c3 -p high -e low -t "Revised title" -d "Added context"
td done td-a1b2c3 td-d4e5f6 # one or many
td reopen td-a1b2c3 # only on explicit user request
```

## Logging and looking for work

These, and `show` of course, but that one's self-explanatory, are your most-run `td` commands.

The `log`s are your scratchpad from which another agent or human might pick up after you. Any time you discover something, like maybe the code doesn't actually line up with the spec or some constraint forces you to implement something differently from initially described or whatever else, this is where you note it down. As you're discussing things with the user, they'll communicate preferences or make decisions or tell you to do things a certain way. Any time the task doesn't already reflect those, this is where you record them. Do not EVER let these signals go unrecorded. In case there's a network outage or a power failure or any other kind of weird issue, the user needs to be able to hand this ticket in whatever form YOU left it to what will effectively be an amensic agent. It'll be competent, but like a completely fresh consultant, need lots and lots of context to get up to speed. The ticket and the logs you attach to it _massively_ help anyone coming after you. While the "what" is definitely important, be sure not to skimp on the much more important "why", such as "User said ..." or "Discovered that [code symbol] actually ...".

```bash
td log td-a1b2c3 "User asked for logs in show/export/import but not in list --json. Kept list untouched to avoid breaking existing scripts that parse its output."
td log td-a1b2c3 "User wanted task validation before insert. Added it so 'task not found' is explicit and immediate instead of a less obvious DB failure."
td log td-a1b2c3 "Kept ON DELETE CASCADE only on task_logs because the user scoped labels/blockers cascade changes to a separate task."
td log td-a1b2c3 "Stayed with positional message input (no stdin) because that was explicitly requested and is easier to replay from shell history."
td log td-a1b2c3 "Used timestamp+id ordering so handoff readers get a stable timeline even when two entries land in the same second."
td log td-a1b2c3 "Import replaces logs for the task to keep repeated imports deterministic and consistent with label/blocker import behavior."
td log td-a1b2c3 "CI failed only on migration version assertions after adding 0004; updated expected version and re-ran full suite to confirm no behavioral regressions."
td log td-a1b2c3 "Ran manual round-trip (init -> create -> log -> show -> export/import -> show) to prove logs survive transfer and stay in chronological order."
```

The `next` command traverses the graph using an algorithm that weighs tasks such that the top-scoring task is likely the one you want to start with. The algorithm accounts for unblocking other tasks, priority, and effort signals. It defaults to sorting by tasks which make the impact. Effort mode still accounts for all the signals, buts weighs them such that tasks with lower effort values bubble to the top. Only use effort mode if explicitly requested. Passing the verbose flag will show the equation resulting in the score, if the user is curious why things are sorted as they are. Only use the verbose flag if the user is curious about sorting/scoring.

```bash
# What should I work on next?
td next # top 5 by critical path (default)
td next --mode effort
td next --verbose # show scoring breakdown per task
td next -n 3 # limit to top 3
td next --mode effort -v # combine flags
```

Reminder: update the status to "in*progress" \_before* starting and _frequently_ attach high-signal, low-noise logs to the task at hand.
