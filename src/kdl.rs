use crate::nvim::format_nvim;
use chrono::Local;
use regex::Regex;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Changes {
    pub file_path: String,
    pub original_command: String,
    pub simplified_command: String,
}

/// Scans a directory recursively for session-layout.kdl files and simplifies nvim commands.
pub fn scan_layouts(dir_path: &str, verbose: bool, dry_run: bool) {
    let path = Path::new(dir_path);
    if !path.is_dir() {
        eprintln!("Error: {} is not a directory", dir_path);
        return;
    }

    if dry_run {
        println!("===============DRY RUN===============");
    }

    println!("Scanning {} for session-layout.kdl files...", dir_path);

    let mut changes = Vec::new();
    scan_dir_recursive(path, &mut changes, verbose, dry_run);

    print_summary(&changes, verbose, dry_run);

    // Log to file (only if not dry-run)
    if !dry_run
        && !changes.is_empty()
        && let Ok(mut log_file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/nvim-resurrect.log")
    {
        let timestamp = Local::now().format("%Y-%m-%d %I:%M:%S %p");
        let _ = writeln!(
            log_file,
            "\n[{}] Processed {} files",
            timestamp,
            changes.len()
        );
    }
}

/// Prints a summary of changes found and applied.
fn print_summary(changes: &[Changes], verbose: bool, dry_run: bool) {
    if changes.is_empty() {
        println!("\nNo changes needed.");
    } else {
        println!("\nFound {} file(s) to update.", changes.len());

        if !verbose && !dry_run {
            println!("Files updated:");
            for change in changes {
                println!("  {}", change.file_path);
            }
        } else if dry_run && !verbose {
            println!("Files that would be updated:");
            for change in changes {
                println!("  {}", change.file_path);
            }
        }

        if verbose {
            println!("\nDetailed changes:");
            for (idx, change) in changes.iter().enumerate() {
                println!("\n{}. {}", idx + 1, change.file_path);
                println!("   Original: {}", change.original_command);
                println!("   Simplified: {}", change.simplified_command);
            }
        }
    }
}

/// Recursively scans directories for session-layout.kdl files.
fn scan_dir_recursive(dir: &Path, changes: &mut Vec<Changes>, verbose: bool, dry_run: bool) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_dir_recursive(&path, changes, verbose, dry_run);
            } else if path.file_name().and_then(|n| n.to_str()) == Some("session-layout.kdl")
                && let Some(path_str) = path.to_str()
            {
                process_kdl_file(path_str, changes, verbose, dry_run);
            }
        }
    }
}

/// Processes a single KDL file, simplifying nvim commands.
fn process_kdl_file(file_path: &str, changes: &mut Vec<Changes>, verbose: bool, dry_run: bool) {
    if verbose {
        if dry_run {
            println!("Would process: {}", file_path);
        } else {
            println!("Processing: {}", file_path);
        }
    }

    match std::fs::read_to_string(file_path) {
        Ok(content) => {
            let (modified, mut file_changes) = process_kdl_content(&content);

            if !file_changes.is_empty() {
                // Add file path to all changes from this file
                for change in &mut file_changes {
                    change.file_path = file_path.to_string();
                }
                changes.extend(file_changes);

                if !dry_run && let Err(e) = std::fs::write(file_path, &modified) {
                    eprintln!("Error writing to {}: {}", file_path, e);
                }
            }
        }
        Err(e) => eprintln!("Error reading {}: {}", file_path, e),
    }
}

/// Processes KDL content and simplifies nvim pane commands.
/// Returns the modified content and a list of changes made.
pub fn process_kdl_content(content: &str) -> (String, Vec<Changes>) {
    let pane_pattern =
        Regex::new(r#"pane\s+command="([^"]*nvim[^"]*)"\s*([^{]*)\{\s*([\s\S]*?)\}"#).unwrap();

    let mut result = content.to_string();
    let mut changes = Vec::new();

    for caps in pane_pattern.captures_iter(content) {
        if let (Some(cmd_match), Some(attrs_match), Some(body_match)) =
            (caps.get(1), caps.get(2), caps.get(3))
        {
            let body = body_match.as_str();
            let full_command = reconstruct_command_from_body(cmd_match.as_str(), body);
            let formatted = format_nvim(&full_command);

            // Only track if it actually changes
            if formatted != full_command {
                changes.push(Changes {
                    file_path: "".to_string(), // Will be filled in by caller
                    original_command: full_command.clone(),
                    simplified_command: formatted.clone(),
                });
            }

            if let Some(whole_match) = caps.get(0) {
                let replacement = build_simplified_pane(&formatted, attrs_match.as_str(), body);
                result = result.replace(whole_match.as_str(), &replacement);
            }
        }
    }

    (result, changes)
}

/// Reconstructs the full command from the KDL pane block.
/// Combines the command with all quoted arguments.
fn reconstruct_command_from_body(cmd: &str, body: &str) -> String {
    let mut all_args = String::new();

    if let Some(args_start) = body.find("args") {
        let args_content = &body[args_start..];

        let content_until_next = if let Some(next_prop) = args_content[5..].find('\n') {
            let after_newline = &args_content[5 + next_prop..];
            if let Some(pos) = after_newline.find(|c: char| c.is_alphabetic()) {
                &args_content[..5 + next_prop + pos]
            } else {
                args_content
            }
        } else {
            args_content
        };

        let quote_pattern = Regex::new(r#""([^"]*)""#).unwrap();
        for m in quote_pattern.captures_iter(content_until_next) {
            if let Some(quoted) = m.get(1) {
                all_args.push(' ');
                all_args.push_str(quoted.as_str());
            }
        }
    }

    format!("{}{}", cmd, all_args)
}

/// Builds a simplified pane block with the new command.
/// Preserves attributes and other properties like start_suspended.
fn build_simplified_pane(formatted_command: &str, attrs: &str, body: &str) -> String {
    // Extract filenames from "nvim file1 file2" format
    let files = if let Some(stripped) = formatted_command.strip_prefix("nvim ") {
        stripped
    } else {
        ""
    };

    // Build args line with individual quoted filenames
    let args_line = if files.is_empty() {
        String::new()
    } else {
        let file_list: Vec<&str> = files.split_whitespace().collect();
        let quoted_files: Vec<String> = file_list.iter().map(|f| format!("\"{}\"", f)).collect();
        format!("            args {}\n", quoted_files.join(" "))
    };

    // Extract other attributes from body (like start_suspended)
    let mut other_attrs = String::new();
    let lines: Vec<&str> = body.lines().collect();
    for line in lines {
        let trimmed = line.trim();
        if !trimmed.starts_with("args") && !trimmed.is_empty() {
            other_attrs.push_str(line);
            other_attrs.push('\n');
        }
    }

    let attrs_trimmed = attrs.trim();
    let attrs_str = if attrs_trimmed.is_empty() {
        String::new()
    } else {
        format!(" {}", attrs_trimmed)
    };

    format!(
        "pane command=\"nvim\"{} {{\n{}{}}}\n",
        attrs_str, args_line, other_attrs
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_kdl_content_single_file() {
        let input = r#"pane command="/home/zach/.nix-profile/bin/nvim" {
            args "--cmd" "lua vim.opt.packpath:prepend('/nix/store/test')" "file.txt"
            start_suspended true
        }"#;

        let (result, changes) = process_kdl_content(input);

        // Should have command="nvim" with args "file.txt"
        assert!(result.contains(r#"command="nvim""#));
        assert!(result.contains(r#"args "file.txt""#));
        assert!(result.contains("start_suspended true"));
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_process_kdl_content_multiple_files() {
        let input = r#"
            pane command="/home/zach/.nix-profile/bin/nvim" focus=true size="50%" {
                args "--cmd" "lua vim.opt.packpath:prepend('/nix/store/7fcfmii0vli2ncrgw8phdj1r7zcxf0fc-mnw-configDir');vim.opt.runtimepath:prepend('/nix/store/7fcfmii0vli2ncrgw8phdj1r7zcxf0fc-mnw-configDir');vim.g.loaded_node_provider=0;vim.g.loaded_perl_provider=0;vim.g.loaded_python_provider=0;vim.g.loaded_python3_provider=0;vim.g.ruby_host_prog='/nix/store/4pm6h00i8jizd1vcfh90gkfsipd634rc-neovim-providers/bin/neovim-ruby-host'" "--cmd" "lua vim.opt.packpath:prepend('/nix/store/7fcfmii0vli2ncrgw8phdj1r7zcxf0fc-mnw-configDir');vim.opt.runtimepath:prepend('/nix/store/7fcfmii0vli2ncrgw8phdj1r7zcxf0fc-mnw-configDir');vim.g.loaded_node_provider=0;vim.g.loaded_perl_provider=0;vim.g.loaded_python_provider=0;vim.g.loaded_python3_provider=0;vim.g.ruby_host_prog='/nix/store/4pm6h00i8jizd1vcfh90gkfsipd634rc-neovim-providers/bin/neovim-ruby-host'" "--cmd" "lua vim.opt.packpath:prepend('/nix/store/7fcfmii0vli2ncrgw8phdj1r7zcxf0fc-mnw-configDir');vim.opt.runtimepath:prepend('/nix/store/7fcfmii0vli2ncrgw8phdj1r7zcxf0fc-mnw-configDir');vim.g.loaded_node_provider=0;vim.g.loaded_perl_provider=0;vim.g.loaded_python_provider=0;vim.g.loaded_python3_provider=0;vim.g.ruby_host_prog='/nix/store/4pm6h00i8jizd1vcfh90gkfsipd634rc-neovim-providers/bin/neovim-ruby-host'" "file1.rs" "file2.rs"
                start_suspended true
        }"#;

        let (result, changes) = process_kdl_content(input);
        dbg!(&result);
        dbg!(&changes);

        // Should have command="nvim" with args "file1.rs" "file2.rs"
        assert!(result.contains(r#"command="nvim""#));
        assert!(result.contains(r#"args "file1.rs" "file2.rs""#));
        assert!(result.contains("start_suspended true"));
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_process_kdl_content_no_files() {
        let input = r#"pane command="/usr/bin/nvim" {
            args
            start_suspended true
        }"#;

        let (result, changes) = process_kdl_content(input);
        dbg!(&result);
        dbg!(&changes);

        // Should have command="nvim" with no args line (no files)
        assert!(result.contains(r#"command="nvim""#));
        assert!(!result.contains("args"));
        assert!(result.contains("start_suspended true"));
        assert_eq!(changes.len(), 1);
    }

    #[test]
    fn test_process_kdl_content_already_simplified() {
        let input = r#"pane command="nvim" {
            args "asdf" "file.txt"
            start_suspended true
}"#;

        let (result, changes) = process_kdl_content(input);
        dbg!(&result);
        dbg!(&changes);

        // Should remain the same since it's already simplified
        assert_eq!(result.trim(), input.trim());
        assert_eq!(changes.len(), 0);
    }

    #[test]
    fn test_extract_files_from_formatted() {
        // Test inline extraction logic
        assert_eq!(
            "nvim file.txt".strip_prefix("nvim ").unwrap_or(""),
            "file.txt"
        );
        assert_eq!(
            "nvim file1.rs file2.rs".strip_prefix("nvim ").unwrap_or(""),
            "file1.rs file2.rs"
        );
        assert_eq!("nvim ".strip_prefix("nvim ").unwrap_or(""), "");
        assert_eq!("something else".strip_prefix("nvim ").unwrap_or(""), "");
    }
}
