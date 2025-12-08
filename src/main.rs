mod cli;
mod kdl;
mod nvim;
mod utils;

use clap::Parser;
use cli::{Cli, Commands};
use kdl::scan_layouts;
use nvim::format_nvim;
use utils::{expand_home, log_command};

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::ScanLayouts { path, dry_run }) => {
            let expanded_path = expand_home(path);
            scan_layouts(&expanded_path, cli.verbose, *dry_run);
        }
        None => {
            // Original behavior
            let command = std::env::var("RESURRECT_COMMAND")
                .expect("RESURRECT_COMMAND not set");
            let formatted = format_nvim(&command);
            println!("{formatted}");

            log_command(&command, &formatted);
        }
    }
}
