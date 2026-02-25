# yatd, _yet another td_

There are many tds. This one is mine. It's in Rust, very small, very fast, very
simple, uses SQLite embedded in the binary, and includes a skill. I intend it
to be the bare minimum for something like a repo-specific issue tracker and
possibly complementary to tools like [OpenSpec].

[OpenSpec]: https://github.com/Fission-AI/OpenSpec

Install with `mise use -g cargo:https://git.secluded.site/holt@latest` or
`cargo install --git https://git.secluded.site/holt`. Tell your agent how/when
to use td by first installing the skill with `td skill`, then somehow referring
to td when telling the agent to do something involving td. It shouldn't invoke
the skill unless you mention td, allowing your agent to use other todo/issue
tools in other repos even with this global skill.

Inspired by [alosec/td].

[alosec/td]: https://github.com/alosec/td/

```
$ td --help
Todo tracker for AI agents

Usage: td [OPTIONS] <COMMAND>

Commands:
  init     Initialize .td directory
  create   Create a new task [aliases: add]
  list     List tasks [aliases: ls]
  show     Show task details
  update   Update a task
  done     Mark task(s) as closed [aliases: close]
  reopen   Reopen task(s)
  dep      Manage dependencies / blockers
  label    Manage labels
  search   Search tasks by title or description
  ready    Show tasks with no open blockers
  stats    Show task statistics (always JSON)
  compact  Vacuum the database
  export   Export tasks to JSONL (one JSON object per line)
  import   Import tasks from a JSONL file
  skill    Install the agent skill file (SKILL.md)
  help     Print this message or the help of the given subcommand(s)

Options:
  -j, --json     Output JSON
  -h, --help     Print help
  -V, --version  Print version
```
