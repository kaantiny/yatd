use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "td", version, about = "Todo tracker for AI agents")]
pub struct Cli {
    /// Output JSON
    #[arg(short = 'j', long = "json", global = true)]
    pub json: bool,

    /// Select a project explicitly (overrides cwd binding)
    #[arg(long, global = true)]
    pub project: Option<String>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Manage projects
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },

    /// Create a new task
    #[command(visible_alias = "add")]
    Create {
        /// Task title
        title: Option<String>,

        /// Priority (low, medium, high)
        #[arg(short, long, default_value = "medium")]
        priority: String,

        /// Effort (low, medium, high)
        #[arg(short, long, default_value = "medium")]
        effort: String,

        /// Task type
        #[arg(short = 't', long = "type", default_value = "task")]
        task_type: String,

        /// Description
        #[arg(short = 'd', long = "desc")]
        desc: Option<String>,

        /// Parent task ID (creates a subtask)
        #[arg(long)]
        parent: Option<String>,

        /// Labels (comma-separated)
        #[arg(short, long)]
        labels: Option<String>,
    },

    /// List tasks
    #[command(visible_alias = "ls")]
    List {
        /// Filter by status
        #[arg(short, long)]
        status: Option<String>,

        /// Filter by priority (low, medium, high)
        #[arg(short, long)]
        priority: Option<String>,

        /// Filter by effort (low, medium, high)
        #[arg(short, long)]
        effort: Option<String>,

        /// Filter by label
        #[arg(short, long)]
        label: Option<String>,
    },

    /// Show task details
    Show {
        /// Task ID
        id: String,
    },

    /// Append a work log entry to a task
    Log {
        /// Task ID
        id: String,
        /// Log entry body
        message: String,
    },

    /// Update a task
    Update {
        /// Task ID
        id: String,

        /// Set status
        #[arg(short, long)]
        status: Option<String>,

        /// Set priority (low, medium, high)
        #[arg(short, long)]
        priority: Option<String>,

        /// Set effort (low, medium, high)
        #[arg(short, long)]
        effort: Option<String>,

        /// Set title
        #[arg(short = 't', long)]
        title: Option<String>,

        /// Set description
        #[arg(short = 'd', long = "desc")]
        desc: Option<String>,
    },

    /// Mark task(s) as closed
    #[command(visible_alias = "close")]
    Done {
        /// Task IDs
        #[arg(required = true)]
        ids: Vec<String>,
    },

    /// Delete task(s)
    Rm {
        /// Skip warnings about dependents becoming unblocked
        #[arg(short, long)]
        force: bool,

        /// Delete the whole subtree (task and descendants)
        #[arg(short = 'r', long)]
        recursive: bool,

        /// Task IDs
        #[arg(required = true)]
        ids: Vec<String>,
    },

    /// Reopen task(s)
    Reopen {
        /// Task IDs
        #[arg(required = true)]
        ids: Vec<String>,
    },

    /// Manage dependencies / blockers
    Dep {
        #[command(subcommand)]
        action: DepAction,
    },

    /// Manage labels
    Label {
        #[command(subcommand)]
        action: LabelAction,
    },

    /// Search tasks by title or description
    Search {
        /// Search query
        query: String,
    },

    /// Show tasks with no open blockers
    Ready,

    /// Recommend next task(s) to work on
    Next {
        /// Scoring strategy: impact (default) or effort
        #[arg(short, long, default_value = "impact")]
        mode: String,

        /// Show signal breakdown and equation
        #[arg(short, long)]
        verbose: bool,

        /// Maximum number of results
        #[arg(short = 'n', long = "limit", default_value = "5")]
        limit: usize,
    },

    /// Show task statistics (always JSON)
    Stats,

    /// Compact accumulated delta files into the base snapshot
    Tidy,

    /// Export tasks to JSONL (one JSON object per line)
    Export,

    /// Import tasks from a JSONL file
    Import {
        /// Path to JSONL file (- for stdin)
        file: String,
    },

    /// Sync project state with a peer via magic wormhole
    Sync {
        /// Wormhole code to connect to a peer (omit to generate one)
        code: Option<String>,
    },

    /// Install the agent skill file (SKILL.md)
    Skill {
        /// Skills directory (writes managing-tasks-with-td/SKILL.md inside)
        #[arg(long)]
        dir: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum DepAction {
    /// Add a dependency (child is blocked by parent)
    Add {
        /// Task that is blocked
        child: String,
        /// Task that blocks it
        parent: String,
    },
    /// Remove a dependency
    Rm {
        /// Task that was blocked
        child: String,
        /// Task that was blocking
        parent: String,
    },
    /// Show child tasks
    Tree {
        /// Parent task ID
        id: String,
    },
}

#[derive(Subcommand)]
pub enum LabelAction {
    /// Add a label to a task
    Add {
        /// Task ID
        id: String,
        /// Label to add
        label: String,
    },
    /// Remove a label from a task
    Rm {
        /// Task ID
        id: String,
        /// Label to remove
        label: String,
    },
    /// List labels on a task
    List {
        /// Task ID
        id: String,
    },
    /// List all distinct labels
    ListAll,
}

#[derive(Subcommand)]
pub enum ProjectAction {
    /// Initialise a central project and bind the current directory to it
    Init {
        /// Project name
        name: String,
    },
    /// Bind the current directory to an existing project
    Bind {
        /// Project name
        name: String,
    },
    /// Remove the binding for the current directory
    Unbind,
    /// Delete a project from central storage and remove all directory bindings
    Delete {
        /// Project name
        name: String,
    },
    /// List all known projects in central storage
    List,
}
