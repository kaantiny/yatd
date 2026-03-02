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

## Sync Bootstrapping

When bringing a project to a second machine, do **not** run `td init` again.
Initialize once on the first machine, then bootstrap the second machine by
running `td sync` and entering the wormhole code from the first machine.

```sh
# Machine A (already initialized project)
td sync

# Machine B (same repo checkout, no td project yet)
td sync <code-from-machine-a>
```

Running `td init` on both machines creates different `project_id` values and
prevents sync from merging them.

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

## Contributions

I'm trying [the Jujutsu VCS] out for this project. I'm enjoying it and LLMs
seem to do pretty well with it too. The collaboration story is a bit less
convenient, especially since I'm also trying [pr.pico.sh]. It works _very_ well
with git projects, but jujutsu is missing some things git has which
[pr.pico.sh] relies on. When cloning this repo, do so with `jj git clone
--colocate git@git.secluded.site:yatd.git` and the relevant git commands should
work fine.

[the Jujutsu VCS]: https://www.jj-vcs.dev/latest/

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
# Clone this repo
jj git clone --colocate git@git.secluded.site:yatd.git

# Create a new change and describe what it does
jj new -m "Add fancy new thing" # Imperative, kernel-style commits, not Conventional Commits

# When ready, create a new patch request
git format-patch origin/main --stdout | ssh pr.pico.sh pr create amolith/llm-projects

# After potential feedback, revise and submit a new patchset
jj amend
git format-patch origin/main --stdout | ssh pr.pico.sh pr add {prID}

# List patch requests
ssh pr.pico.sh pr ls amolith/llm-projects --mine
```

See "How do Patch Requests work?" on [pr.pico.sh]'s home page for a more
complete example workflow.

[amolith/llm-projects]: https://pr.pico.sh/r/amolith/llm-projects
[pr.pico.sh]: https://pr.pico.sh
[my pr.pico.sh skill]: https://git.secluded.site/agent-skills#:~:text=collaborating%2Dthrough%2Dpr%2Dpico%2Dsh
