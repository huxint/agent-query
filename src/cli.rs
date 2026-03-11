use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "agent-query",
    author,
    about = "Fast AST query tool for AI agents — understand project structure in seconds",
    long_about = "agent-query — Fast AST query tool for AI agents\n\n\
        Scans source repositories, extracts AST structure, and answers architectural questions.\n\
        Supports 12+ languages including Python, TypeScript, Rust, Go, Java, C/C++, and more.\n\n\
        Every analysis command can either:\n\
        • scan a repository directly (default: current directory)\n\
        • reuse a pre-generated AST JSON with `--ast`\n\n\
        Use `scan` when you want a reusable machine-readable snapshot for scripts or repeated queries.",
    after_long_help = "EXAMPLES:\n  \
        agent-query overview\n  \
        agent-query tree --repo ./my-project\n  \
        agent-query file src/main.rs\n  \
        agent-query search \"UserService\"\n  \
        agent-query deps src/api/routes.py --direction down --depth 1\n  \
        agent-query hotspots --days 180 --top 15\n  \
        agent-query hub --top 15\n  \
        agent-query summary\n  \
        agent-query scan -o ast.json\n  \
        agent-query markdown --ast ast.json --save structure.md",
    version
)]
pub struct Cli {
    #[arg(long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Clone, ValueEnum)]
pub enum DepDirection {
    Both,
    Up,
    Down,
}

#[derive(Clone, ValueEnum)]
pub enum MarkdownModeArg {
    Compact,
    Default,
    Full,
}

#[derive(Args, Clone, Default)]
pub struct SourceArgs {
    /// Repository path (defaults to current directory)
    #[arg(long, conflicts_with = "ast")]
    pub repo: Option<String>,

    /// Use a pre-generated AST JSON instead of scanning a repository
    #[arg(long, conflicts_with = "repo")]
    pub ast: Option<String>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Project overview — languages, file counts, top-level structure
    #[command(
        long_about = "Show a high-level overview of the project — languages, file counts, line counts,\n\
        and top-level directory structure.\n\n\
        Examples:\n  \
        agent-query overview\n  \
        agent-query overview --repo ./my-project\n  \
        agent-query overview --ast ast.json"
    )]
    Overview {
        #[command(flatten)]
        source: SourceArgs,

        /// Max nodes when scanning a repository
        #[arg(long, default_value = "500")]
        max_nodes: usize,

        /// Save output to file
        #[arg(long)]
        save: Option<String>,
    },

    /// Annotated file tree — files with language and line counts
    #[command(
        long_about = "Display the project file tree with annotations showing language and line counts for each file.\n\n\
        Examples:\n  \
        agent-query tree\n  \
        agent-query tree --repo ./my-project\n  \
        agent-query tree --ast ast.json"
    )]
    Tree {
        #[command(flatten)]
        source: SourceArgs,

        /// Max nodes when scanning a repository
        #[arg(long, default_value = "500")]
        max_nodes: usize,

        /// Save output to file
        #[arg(long)]
        save: Option<String>,
    },

    /// Show file structure and imports
    #[command(
        long_about = "Show the complete structure of a source file — classes, functions, and import dependencies.\n\n\
        Examples:\n  \
        agent-query file src/server/handler.py\n  \
        agent-query file src/lib.rs --repo ./my-project\n  \
        agent-query file src/main.rs --ast ast.json"
    )]
    File {
        /// File path to query
        file_path: String,

        #[command(flatten)]
        source: SourceArgs,

        /// Max nodes when scanning a repository
        #[arg(long, default_value = "500")]
        max_nodes: usize,

        /// Save output to file
        #[arg(long)]
        save: Option<String>,
    },

    /// Search for symbol definitions across the project
    #[command(
        long_about = "Search for class and function definitions by name across all files in the project.\n\n\
        Examples:\n  \
        agent-query search \"UserService\"\n  \
        agent-query search handler --repo ./my-project\n  \
        agent-query search Parser --ast ast.json"
    )]
    Search {
        /// Search pattern (case-insensitive substring match)
        pattern: String,

        #[command(flatten)]
        source: SourceArgs,

        /// Max nodes when scanning a repository
        #[arg(long, default_value = "500")]
        max_nodes: usize,

        /// Save output to file
        #[arg(long)]
        save: Option<String>,
    },

    /// Transitive dependency chain — upstream and downstream
    #[command(long_about = "Show the dependency chain for a file or module.\n\n\
        Upstream: internal modules this file transitively depends on.\n\
        Downstream: internal modules that transitively depend on this file.\n\
        Direct external imports are also shown on upstream queries.\n\
        Cycle detection is built in — circular dependencies are flagged.\n\n\
        Examples:\n  \
        agent-query deps src/server/handler.py\n  \
        agent-query deps src/lib.rs --depth 5\n  \
        agent-query deps src/api/routes.py --direction down --depth 1 --repo ./my-project\n  \
        agent-query deps src/main.rs --ast ast.json")]
    Deps {
        /// File path or module to analyze
        file_path: String,

        #[command(flatten)]
        source: SourceArgs,

        /// Maximum traversal depth
        #[arg(long, default_value = "3")]
        depth: usize,

        /// Traversal direction: both, up (imports), or down (importers)
        #[arg(long, default_value = "both")]
        direction: DepDirection,

        /// Max nodes when scanning a repository
        #[arg(long, default_value = "500")]
        max_nodes: usize,

        /// Save output to file
        #[arg(long)]
        save: Option<String>,
    },

    /// Identify high fan-in/fan-out hub modules
    #[command(long_about = "Identify hub modules with the highest connectivity.\n\n\
        Examples:\n  \
        agent-query hub\n  \
        agent-query hub --top 15 --repo ./my-project\n  \
        agent-query hub --ast ast.json")]
    Hub {
        #[command(flatten)]
        source: SourceArgs,

        /// Number of top modules to show
        #[arg(long, default_value = "10")]
        top: usize,

        /// Max nodes when scanning a repository
        #[arg(long, default_value = "500")]
        max_nodes: usize,

        /// Save output to file
        #[arg(long)]
        save: Option<String>,
    },

    /// Per-directory structural summary
    #[command(
        long_about = "Show a structural summary grouped by top-level directories.\n\n\
        Examples:\n  \
        agent-query summary\n  \
        agent-query summary --repo ./my-project\n  \
        agent-query summary --ast ast.json"
    )]
    Summary {
        #[command(flatten)]
        source: SourceArgs,

        /// Max nodes when scanning a repository
        #[arg(long, default_value = "500")]
        max_nodes: usize,

        /// Save output to file
        #[arg(long)]
        save: Option<String>,
    },

    /// Git hotspots — files with the most change activity
    #[command(
        long_about = "Analyze Git history and rank files by change activity.\n\n\
        Hotspots are sorted by revision count first, then by total churn.\n\
        Use this to find risky or frequently-touched files before asking an LLM to modify a project.\n\n\
        Examples:\n  \
        agent-query hotspots\n  \
        agent-query hotspots --days 180\n  \
        agent-query hotspots --repo ./my-project --top 20"
    )]
    Hotspots {
        /// Repository path (defaults to current directory)
        #[arg(long)]
        repo: Option<String>,

        /// Limit history to the last N days
        #[arg(long)]
        days: Option<u32>,

        /// Number of files to show
        #[arg(long, default_value = "10")]
        top: usize,

        /// Save output to file
        #[arg(long)]
        save: Option<String>,
    },

    /// Generate a fixed-format Markdown project summary
    #[command(
        long_about = "Generate a fixed-format Markdown summary for LLM prompts or docs.\n\n\
        Modes:\n  \
        compact  Minimal architecture brief for fast prompting\n  \
        default  Balanced structure + symbol summary (default)\n  \
        full     Expand every source file with full symbol details\n\n\
        Examples:\n  \
        agent-query markdown\n  \
        agent-query markdown --mode compact\n  \
        agent-query markdown --repo ./my-project --save structure.md\n  \
        agent-query markdown --ast ast.json --mode full --save structure.md"
    )]
    Markdown {
        #[command(flatten)]
        source: SourceArgs,

        /// Markdown detail mode: compact, default, or full
        #[arg(long, value_enum, default_value = "default")]
        mode: MarkdownModeArg,

        /// Max nodes when scanning a repository
        #[arg(long, default_value = "500")]
        max_nodes: usize,

        /// Save output to file
        #[arg(long)]
        save: Option<String>,
    },

    /// Scan a repository and write AST JSON
    #[command(
        long_about = "Scan a repository, extract AST structure, and write JSON.\n\n\
        Examples:\n  \
        agent-query scan -o ast.json\n  \
        agent-query scan --repo ./my-project -o ast.json"
    )]
    Scan {
        /// Repository path (defaults to current directory)
        #[arg(long)]
        repo: Option<String>,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Max nodes in output
        #[arg(long, default_value = "500")]
        max_nodes: usize,
    },
}
