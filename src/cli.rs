use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "td", version, about = "Todo tracker for AI agents")]
pub struct Cli {
    /// Output JSON
    #[arg(short = 'j', long = "json", global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize .td directory
    Init {
        /// Add .td/ to .gitignore
        #[arg(long)]
        stealth: bool,
    },

    /// Create a new task
    #[command(visible_alias = "add")]
    Create {
        /// Task title
        title: Option<String>,

        /// Priority level (1=high, 2=medium, 3=low)
        #[arg(short, long, default_value_t = 2)]
        priority: i32,

        /// Effort level (1=low, 2=medium, 3=high)
        #[arg(short, long, default_value_t = 2)]
        effort: i32,

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

        /// Filter by priority
        #[arg(short, long)]
        priority: Option<i32>,

        /// Filter by label
        #[arg(short, long)]
        label: Option<String>,
    },

    /// Show task details
    Show {
        /// Task ID
        id: String,
    },

    /// Update a task
    Update {
        /// Task ID
        id: String,

        /// Set status
        #[arg(short, long)]
        status: Option<String>,

        /// Set priority
        #[arg(short, long)]
        priority: Option<i32>,

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

    /// Show task statistics (always JSON)
    Stats,

    /// Vacuum the database
    Compact,

    /// Export tasks to JSONL (one JSON object per line)
    Export,

    /// Import tasks from a JSONL file
    Import {
        /// Path to JSONL file (- for stdin)
        file: String,
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
