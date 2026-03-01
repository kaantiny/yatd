# yatd, _yet another td_

There are many tds. This one is mine. It's in Rust, very small, very fast, very
simple, uses SQLite embedded in the binary, and includes a skill. I intend it
to be the bare minimum for something like a repo-specific issue tracker and
possibly complementary to tools like [OpenSpec].

[OpenSpec]: https://github.com/Fission-AI/OpenSpec

Install with `mise use -g cargo:https://git.secluded.site/yatd@latest` or
`cargo install --git https://git.secluded.site/yatd` or by cloning and running
`make install`. Tell your agent how/when to use td by first installing the
skill with `td skill`, then somehow referring to td when telling the agent to
do something involving td. It shouldn't invoke the skill unless you mention td,
allowing your agent to use other todo/issue tools in other repos even with this
global skill. Td IDs are prefixed with `td-`, so pasting the ID should be
enough of a mention.

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
  log      Append a work log entry to a task
  update   Update a task
  done     Mark task(s) as closed [aliases: close]
  rm       Delete task(s)
  reopen   Reopen task(s)
  dep      Manage dependencies / blockers
  label    Manage labels
  search   Search tasks by title or description
  ready    Show tasks with no open blockers
  next     Recommend next task(s) to work on
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

## Contributing

## Contributions

Patch requests are in [amolith/llm-projects] on [pr.pico.sh]. You don't need a
new account to contribute, you don't need to fork this repo, you don't need to
fiddle with `git send-email`, you don't need to faff with your email client to
get `git request-pull` working...

You just need:

- Git
- SSH
- An SSH key

If you're using LLM agents, you might instead want to give them [my pr.pico.sh
skill].

```sh
# Clone this repo, make your changes, and commit them
# Create a new patch request with
git format-patch origin/main --stdout | ssh pr.pico.sh pr create amolith/llm-projects
# After potential feedback, submit a revision to an existing patch request with
git format-patch origin/main --stdout | ssh pr.pico.sh pr add {prID}
# List patch requests
ssh pr.pico.sh pr ls amolith/llm-projects
```

See "How do Patch Requests work?" on [pr.pico.sh]'s home page for a more
complete example workflow.

[amolith/llm-projects]: https://pr.pico.sh/r/amolith/llm-projects
[pr.pico.sh]: https://pr.pico.sh
[my pr.pico.sh skill]: https://git.secluded.site/agent-skills#:~:text=collaborating%2Dthrough%2Dpr%2Dpico%2Dsh
