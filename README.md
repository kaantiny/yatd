# yatd, _yet another td_

There are many tds. This one is mine. It's in Rust, very fast, very
small, fairly simple, and includes a skill. I intend it to be the bare
minimum for something like a repo-specific issue tracker and possibly
complementary to tools like [OpenSpec].

[OpenSpec]: https://github.com/Fission-AI/OpenSpec

Install with `mise use -g cargo:https://git.secluded.site/yatd@latest` or
`cargo install --git https://git.secluded.site/yatd` or by cloning and running
`make install`. Tell your agent how/when to use td by first installing the
skill with `td skill`, then somehow referring to td when telling the agent to
do something involving td. It shouldn't invoke the skill unless you mention td,
allowing your agent to use other todo/issue tools in other repos even with this
global skill.

Inspired by [alosec/td].

[alosec/td]: https://github.com/alosec/td/

## Bootstrapping

When syncing a project to another machine, do **not** run `td init` on
the other machine. Initialize just once on the first machine, then
bootstrap others by running `td sync` on the first machine, then `td
sync wormhole-code` on another.

```sh
# Machine A (already initialized)
td sync

# Machine B (same repo checkout, no td project yet)
td sync 5-lurid-gecko
```

Running `td init` on both machines creates different `project_id` values and
prevents sync from merging them.

## Usage

There are many ways to use something like this and I won't say any one
is better than another. However, I tend to use it in a particular way
and that may lead its design to faciliate that way particularly well.

<details><summary>td on its own (click to expand)</summary>

I first think of a feature, then tell the agent about it and ask it to
interview me about any gaps. We go back and forth, me nitpicking things
about what it said, it nitpicking things about what I said, until it
feels right. Then I say something like "let's think about how we can
break this up into the smallest units of work and create tasks with
appropriate dependency relationships to communicate the order in which
we must tackle them." It creates a bunch of tasks, links them, and
probably describes what it did. I run td next to see what's bubbled up
to the top, read through it, and decide whether it needs more work or is
good to go as-is. If it needs more work, I start a new session with
"let's think about {id} and plan it out some more, then log those
decisions to the task". If it's ready, the new session begins with
"let's get started on {id} by having a look around at the relevant code
then breaking it down into smaller child tasks". Then depending on the
context window and task scale, I either have it get started in this same
session or a new one.

I said it's complementary to things like OpenSpec at the beginning, but
realise my workflow almost entirely obviates OpenSpec 🥴 I do intend to
use them together soon and will describe whatever workflow I end up with
then.

</details>

```
$ td --help
Todo tracker for AI agents

Usage: td [OPTIONS] <COMMAND>

Commands:
  init      Initialize a central project and bind the current directory to it
  use       Bind the current directory to an existing project
  projects  List all known projects in central storage
  create    Create a new task [aliases: add]
  list      List tasks [aliases: ls]
  show      Show task details
  log       Append a work log entry to a task
  update    Update a task
  done      Mark task(s) as closed [aliases: close]
  rm        Delete task(s)
  reopen    Reopen task(s)
  dep       Manage dependencies / blockers
  label     Manage labels
  search    Search tasks by title or description
  ready     Show tasks with no open blockers
  next      Recommend next task(s) to work on
  stats     Show task statistics (always JSON)
  compact   Vacuum the database
  export    Export tasks to JSONL (one JSON object per line)
  import    Import tasks from a JSONL file
  sync      Sync project state with a peer via magic wormhole
  skill     Install the agent skill file (SKILL.md)
  help      Print this message or the help of the given subcommand(s)

Options:
  -j, --json               Output JSON
      --project <PROJECT>  Select a project explicitly (overrides cwd binding)
  -h, --help               Print help
  -V, --version            Print version
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
