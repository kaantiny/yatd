# AGENTS.md

## What this repo is
- Rust CLI app (`td`) for local task tracking aimed at agent workflows.
- Storage is Loro CRDT in central storage (`~/.local/share/td/projects/<name>/`). Directory bindings map working directories to projects.
- Entry flow is `src/main.rs` → `yatd::run()` (`src/lib.rs`) → `cmd::dispatch()` (`src/cmd/mod.rs`).
- We use jj for version control, not git. DO NOT use Conventional Commits. Do use imperative, Linux kernel-style commits. Always check `jj st` before starting. If there are changes in progress, run `jj new -m ...` to create a new working copy describing the pending work. If there are no changes in the working copy, run `jj desc -m ...` to describe the pending work. `jj diff --git` is more understandable.
- JSON mode is not for "agent consumers". It's for wiring things together, whether the wirer is human or LLM.
- Contributions are through pr.pico.sh. If you're asked to open a PR or contribute or review a PR or similar and you have the skill, read that. If you don't have the skill, somehow fetch the contents of `https://git.secluded.site/agent-skills/blob/main/skills/collaborating-through-pr-pico-sh/SKILL.md?raw=1`. It's markdown, but served as raw plain text, so curl or any other UA will do fine. If you're not contributing/reviewing yet, don't read it yet.

## Essential commands
- Format: `jj fix` (rustfmt via repo-level jj config)
- Lint (warnings are errors): `cargo clippy --quiet -- -D warnings`
- Verify after changes: `make verify` (formats, typechecks, lints, and tests in one pass)

## Implementation details
- Project resolution: most commands resolve the project via `db::resolve_project_name()` which checks `--project` flag, `TD_PROJECT` env var, or the directory binding in `~/.local/share/td/bindings.json`. Without a resolved project, commands fail with "no project selected". Only `project` and `skill` subcommands avoid this check.
- Storage is Loro CRDT documents (`base.loro` + incremental changes in `changes/`). There is no SQL database; `src/db.rs` is the Loro document store and task model, not a database layer.
- Schema versioning lives in `src/migrate.rs` (`CURRENT_SCHEMA_VERSION`); migrations operate on the Loro document structure.
- Task IDs: all tasks (top-level and subtasks) get ULIDs via `db::gen_id()`, displayed to the user as short `td-XXXXXXX` forms. Subtasks link to their parent via a `parent` field.

## Command/module map
- CLI definition and argument shapes: `src/cli.rs`
- Dispatch wiring: `src/cmd/mod.rs`
- Command implementations live in `src/cmd/*.rs` (one module per subcommand: `create`, `dep`, `doctor`, `done`, `export`, `import`, `label`, `list`, `log`, `next`, `project`, `ready`, `reopen`, `rm`, `search`, `show`, `skill`, `stats`, `sync`, `tidy`, `update`).
- Loro document store, task model, and domain helpers (`Task`, `TaskId`, `Status`, etc.): `src/db.rs`
- Schema migrations for the Loro document structure: `src/migrate.rs`
- Priority/readiness scoring logic: `src/score.rs`
- Terminal colour helpers: `src/color.rs`

## Testing approach
- Integration-style CLI tests in `tests/cli_*.rs` using:
  - `assert_cmd` for invoking `td`
  - `tempfile::TempDir` for isolated workdirs
  - `predicates` for stdout/stderr assertions
- Typical pattern:
  1. create temp dir
  2. run `td project init`
  3. invoke command under test
  4. assert output and/or inspect central storage in `~/.local/share/td/projects/<name>/` directly
- When adding/changing behavior, prefer extending these CLI tests rather than only unit tests.

## Conventions and patterns to preserve
- Error handling uses `anyhow::Result` throughout command functions.
- Keep command functions shaped as `run(...) -> Result<()>` with argument parsing in `cli.rs` and routing in `cmd/mod.rs`.
- JSON structs rely on serde naming alignment (notably `Task.task_type` renamed to `"type"`). Maintain compatibility for import/export and tests.

## Gotchas
- Running commands without a project binding (and without `--project` or `TD_PROJECT`) yields `no project selected`. Tests should set `current_dir` to directories with initialized bindings.

## Contributions

