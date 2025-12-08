use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zellij-command-hook")]
#[command(about = "Simplify nvim commands in zellij layouts")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scan and simplify session layout files
    ScanLayouts {
        /// Path to scan
        #[arg(default_value = "~/.cache/zellij")]
        path: String,

        /// Dry run - don't make changes, just show what would change
        #[arg(short, long)]
        dry_run: bool,
    },
}
