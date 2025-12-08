use std::io::Write;
use chrono::Local;

/// Expands ~ in paths to the actual home directory.
pub fn expand_home(path: &str) -> String {
    if path.starts_with("~/")
        && let Ok(home) = std::env::var("HOME") {
            return format!("{}{}", home, &path[1..]);
        }
    path.to_string()
}

/// Logs a command transformation to the log file.
pub fn log_command(command: &str, formatted: &str) {
    let log_path = "/tmp/nvim-resurrect.log";
    let mut log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .unwrap();

    let timestamp = Local::now().format("%Y-%m-%d %I:%M:%S %p");
    writeln!(log_file, "\n---\nTimestamp: {}", timestamp).ok();
    writeln!(log_file, "Original command: {}", command).ok();
    writeln!(log_file, "Formatted command: {}", formatted).ok();
    writeln!(log_file).ok();
}
