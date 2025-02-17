use changelog::{Changelog, ChangeType};
use clap::{CommandFactory, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show version information
    Version {
        #[command(subcommand)]
        command: VersionCommands,
    },
    /// Show changelog entry
    Entry {
        /// Version to show (latest, unreleased, or specific version)
        version: String,
    },
    /// Add a new changelog entry
    Add {
        /// Description of the change
        description: String,
        /// Type of change
        #[arg(short, long, default_value = "changed")]
        r#type: ChangeType,
        /// Version to add the change to (defaults to unreleased)
        #[arg(short, long)]
        version: Option<String>,
    },
    /// Release a new version
    Release {
        /// Version or change type (major, minor, patch) to release
        version_or_type: String,
        /// Release date (defaults to today)
        #[arg(short, long)]
        date: Option<String>,
    },
    /// Review commits and add them to changelog
    Review {
        /// Version to add changes to
        #[arg(short, long)]
        version: Option<String>,
    },
    /// Format the changelog file
    Fmt,
    /// Initialize a new changelog
    Init,
    /// Generate shell completion scripts
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Subcommand)]
enum VersionCommands {
    /// Show the latest version
    Latest,
    /// List all versions
    List,
    /// Show git revision range for a version
    Range {
        /// Version to show range for (defaults to HEAD)
        version: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Add {
            description,
            r#type,
            version,
        } => {
            let changelog = Changelog::new();
            if let Err(e) = changelog.add(description, r#type, version.as_deref(), true) {
                eprintln!("Error adding changelog entry: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Release {
            version_or_type,
            date,
        } => {
            let changelog = Changelog::new();
            if let Err(e) = changelog.release(version_or_type, date.as_deref()) {
                eprintln!("Error releasing version: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Review { version } => {
            let changelog = Changelog::new();
            if let Err(e) = changelog.review(version.as_deref()) {
                eprintln!("Error reviewing changes: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Fmt => {
            let changelog = Changelog::new();
            if let Err(e) = changelog.fmt() {
                eprintln!("Error formatting changelog: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Init => {
            let changelog = Changelog::new();
            if let Err(e) = changelog.init() {
                eprintln!("Error initializing changelog: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Entry { version } => {
            let changelog = Changelog::new();
            if let Err(e) = changelog.version_show(version) {
                eprintln!("Error showing entry: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Version { command } => {
            let changelog = Changelog::new();
            match command {
                VersionCommands::Latest => {
                    if let Err(e) = changelog.version_latest() {
                        eprintln!("Error showing latest version: {}", e);
                        std::process::exit(1);
                    }
                }
                VersionCommands::List => {
                    if let Err(e) = changelog.version_list() {
                        eprintln!("Error listing versions: {}", e);
                        std::process::exit(1);
                    }
                }
                VersionCommands::Range { version } => {
                    if let Err(e) = changelog.range(version.as_deref()) {
                        eprintln!("Error showing range: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Commands::Completion { shell } => {
            clap_complete::generate(
                shell,
                &mut Cli::command(),
                env!("CARGO_PKG_NAME"),
                &mut std::io::stdout(),
            );
        }
    }
}
