use crate::nvim::format_nvim;
use chrono::Local;
use kdl::{KdlDocument, KdlEntry, KdlNode, KdlValue};
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Changes {
    pub file_path: String,
    pub original_command: String,
    pub simplified_command: String,
}

/// Scans a directory recursively for session-layout.kdl files and simplifies nvim commands.
pub fn scan_layouts(dir_path: &str, verbose: bool, dry_run: bool, quiet: bool) {
    let path = Path::new(dir_path);
    if !path.is_dir() {
        eprintln!("Error: {} is not a directory", dir_path);
        return;
    }

    if dry_run {
        println!("===============DRY RUN===============");
    }

    if !quiet {
        println!("Scanning {} for session-layout.kdl files...", dir_path);
    }

    let mut changes = Vec::new();
    scan_dir_recursive(path, &mut changes, verbose, dry_run);

    if !quiet {
        print_summary(&changes, verbose, dry_run);
    }

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

/// Processes KDL content and simplifies nvim pane commands, and applies direnv wrapping for "dr " tabs.
/// Returns the modified content and a list of changes made.
pub fn process_kdl_content(content: &str) -> (String, Vec<Changes>) {
    let mut doc: KdlDocument = match content.parse() {
        Ok(doc) => doc,
        Err(_) => return (content.to_string(), Vec::new()),
    };

    let mut changes = Vec::new();

    // Recursively process all nodes to find tabs (handles layout wrapper)
    process_nodes_recursive(doc.nodes_mut(), &mut changes);

    (doc.to_string(), changes)
}

/// Recursively processes nodes to find tabs and panes.
/// This handles the `layout { ... }` wrapper that real session files have.
fn process_nodes_recursive(nodes: &mut [KdlNode], changes: &mut Vec<Changes>) {
    for node in nodes {
        if node.name().value() == "tab" {
            let is_dr_tab = is_direnv_tab(node);
            process_panes_in_node(node, is_dr_tab, changes);
        } else if node.name().value() == "pane" {
            // Top-level pane (not in a tab) - just apply nvim simplification
            process_single_pane(node, false, changes);
            // Also recurse into nested panes
            if let Some(children) = node.children_mut() {
                process_nodes_recursive(children.nodes_mut(), changes);
            }
        } else if let Some(children) = node.children_mut() {
            // Recurse into layout, swap_tiled_layout, new_tab_template, etc.
            process_nodes_recursive(children.nodes_mut(), changes);
        }
    }
}

/// Checks if a tab node has a name starting with "dr "
fn is_direnv_tab(node: &KdlNode) -> bool {
    for entry in node.entries() {
        if entry.name().map(|n| n.value()) == Some("name") {
            if let Some(name) = entry.value().as_string() {
                return name.starts_with("dr ");
            }
        }
    }
    false
}

/// Recursively processes all pane nodes within a node
fn process_panes_in_node(node: &mut KdlNode, is_dr_tab: bool, changes: &mut Vec<Changes>) {
    // Process children recursively
    if let Some(children) = node.children_mut() {
        for child in children.nodes_mut() {
            if child.name().value() == "pane" {
                process_single_pane(child, is_dr_tab, changes);
                // Also process nested panes
                process_panes_in_node(child, is_dr_tab, changes);
            }
        }
    }
}

/// Process a single pane node - either apply direnv transform or nvim simplification
fn process_single_pane(pane: &mut KdlNode, is_dr_tab: bool, changes: &mut Vec<Changes>) {
    // Find the command attribute
    let command_value = get_entry_string_value(pane, "command");
    let command = match command_value {
        Some(cmd) => cmd,
        None => return, // No command attribute, skip this pane
    };

    // Get existing args from children
    let existing_args = get_args_from_children(pane);

    if is_dr_tab {
        // Apply direnv transformation
        apply_direnv_transform(pane, &command, &existing_args, changes);
    } else {
        // Only apply nvim simplification for non-dr tabs
        if command.contains("nvim") {
            apply_nvim_simplification(pane, &command, &existing_args, changes);
        }
    }
}

/// Get a string value from a named entry (attribute)
fn get_entry_string_value(node: &KdlNode, name: &str) -> Option<String> {
    for entry in node.entries() {
        if entry.name().map(|n| n.value()) == Some(name) {
            return entry.value().as_string().map(|s| s.to_string());
        }
    }
    None
}

/// Extract args values from the "args" child node
fn get_args_from_children(pane: &KdlNode) -> Vec<String> {
    if let Some(children) = pane.children() {
        for child in children.nodes() {
            if child.name().value() == "args" {
                return child
                    .entries()
                    .iter()
                    .filter(|e| e.name().is_none()) // Only positional arguments
                    .filter_map(|e| e.value().as_string().map(|s| s.to_string()))
                    .collect();
            }
        }
    }
    Vec::new()
}

/// Apply direnv transformation to a pane
fn apply_direnv_transform(
    pane: &mut KdlNode,
    original_command: &str,
    existing_args: &[String],
    changes: &mut Vec<Changes>,
) {
    // Skip if already direnv wrapped
    if original_command == "direnv" {
        return;
    }

    // Simplify command if it's an nvim command
    let simplified_cmd = if original_command.contains("nvim") {
        // Build full command with args for nvim simplification
        let full_cmd = if existing_args.is_empty() {
            original_command.to_string()
        } else {
            format!("{} {}", original_command, existing_args.join(" "))
        };
        let formatted = format_nvim(&full_cmd);
        // Extract just the command part (nvim) and args separately
        formatted
    } else {
        original_command.to_string()
    };

    // Parse simplified_cmd to get the actual command name and any file args
    let (cmd_name, nvim_file_args) = if simplified_cmd.starts_with("nvim ") {
        (
            "nvim".to_string(),
            simplified_cmd
                .strip_prefix("nvim ")
                .unwrap_or("")
                .split_whitespace()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        )
    } else if simplified_cmd == "nvim" {
        ("nvim".to_string(), Vec::new())
    } else {
        // // Non-nvim command - use the last path component as the command name
        // let cmd_name = std::path::Path::new(&simplified_cmd)
        //     .file_name()
        //     .and_then(|n| n.to_str())
        //     .unwrap_or(&simplified_cmd)
        //     .to_string();
        // (cmd_name, Vec::new())
        (simplified_cmd.to_string(), Vec::new())
    };

    // Build new args: "exec" "." <cmd_name> [args...]
    let mut new_args: Vec<String> = vec!["exec".to_string(), ".".to_string(), cmd_name];

    if !nvim_file_args.is_empty() {
        // For nvim commands, use the file args from simplification
        new_args.extend(nvim_file_args);
    } else if !existing_args.is_empty() {
        // For other commands, preserve existing args
        new_args.extend(existing_args.iter().cloned());
    }

    // Update command attribute to "direnv"
    set_entry_string_value(pane, "command", "direnv");

    // Update args child node
    set_args_in_children(pane, &new_args);

    // Record the change
    let original_desc = if existing_args.is_empty() {
        original_command.to_string()
    } else {
        format!("{} {}", original_command, existing_args.join(" "))
    };
    changes.push(Changes {
        file_path: String::new(),
        original_command: original_desc,
        simplified_command: format!("direnv {}", new_args.join(" ")),
    });
}

/// Apply nvim simplification to a pane (for non-dr tabs)
fn apply_nvim_simplification(
    pane: &mut KdlNode,
    original_command: &str,
    existing_args: &[String],
    changes: &mut Vec<Changes>,
) {
    // Build full command with args
    let full_cmd = if existing_args.is_empty() {
        original_command.to_string()
    } else {
        format!("{} {}", original_command, existing_args.join(" "))
    };

    let formatted = format_nvim(&full_cmd);

    // Only apply if there's a change
    if formatted == full_cmd {
        return;
    }

    // Parse the formatted result - "nvim file1 file2"
    let files: Vec<String> = formatted
        .strip_prefix("nvim ")
        .unwrap_or("")
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    // Update command attribute to "nvim"
    set_entry_string_value(pane, "command", "nvim");

    // Update args child node
    if !files.is_empty() {
        set_args_in_children(pane, &files);
    } else {
        remove_args_from_children(pane);
    }

    changes.push(Changes {
        file_path: String::new(),
        original_command: full_cmd,
        simplified_command: formatted,
    });
}

/// Set a string value for a named entry (attribute)
fn set_entry_string_value(node: &mut KdlNode, name: &str, value: &str) {
    // Use the node's insert method which handles finding/replacing entries properly
    node.insert(name, value);
}

/// Set args values in the "args" child node
fn set_args_in_children(pane: &mut KdlNode, args: &[String]) {
    // Ensure children document exists
    if pane.children().is_none() {
        pane.set_children(KdlDocument::new());
    }

    // Find existing args node and its formatting
    let (found_idx, leading_text) = pane
        .children()
        .map(|c| {
            c.nodes()
                .iter()
                .enumerate()
                .find(|(_, child)| child.name().value() == "args")
                .map(|(idx, node)| (Some(idx), node.leading().map(|s| s.to_string())))
        })
        .flatten()
        .unwrap_or((None, None));

    // If no existing args node, try to get formatting from another child node
    let leading = leading_text.or_else(|| {
        pane.children()
            .and_then(|c| c.nodes().first())
            .and_then(|n| n.leading().map(|s| s.to_string()))
    });

    // Create new args node with proper formatting
    let mut args_node = KdlNode::new("args");
    if let Some(lead) = leading {
        args_node.set_leading(lead);
    }
    for arg in args {
        args_node
            .entries_mut()
            .push(KdlEntry::new(KdlValue::String(arg.clone())));
    }

    if let Some(children) = pane.children_mut() {
        if let Some(idx) = found_idx {
            // Replace existing args node
            children.nodes_mut()[idx] = args_node;
        } else {
            // Insert at the beginning - need to remove leading newline from the node that was first
            // to avoid double newlines, but keep its indentation
            if let Some(first_node) = children.nodes_mut().first_mut() {
                let new_leading = first_node
                    .leading()
                    .map(|l| l.trim_start_matches('\n').to_string());
                if let Some(trimmed) = new_leading {
                    first_node.set_leading(trimmed);
                }
            }
            children.nodes_mut().insert(0, args_node);
        }
    }
}

/// Remove args node from children
fn remove_args_from_children(pane: &mut KdlNode) {
    if let Some(children) = pane.children_mut() {
        children.nodes_mut().retain(|n| n.name().value() != "args");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
    fn test_tab_dr_direnv_single_pane() {
        let input = r#"
            tab name="dr mediactl" focus=true hide_floating_panes=true {
                pane command="/home/zach/.nix-profile/bin/nvim" cwd="mediactl" size="38%" {
                    args "--cmd" "lua vim.opt.packpath:prepend('/nix/store/hsw6sf19akpssa4gq9krkrikx2ivcvkg-mnw-configDir');vim.opt.runtimepath:prepend('/nix/store/hsw6sf19akpssa4gq9krkrikx2ivcvkg-mnw-configDir');vim.g.loaded_node_provider=0;vim.g.loaded_perl_provider=0;vim.g.loaded_python_provider=0;vim.g.loaded_python3_provider=0;vim.g.ruby_host_prog='/nix/store/iv1p4x799nsx06pbg8vhgx1lwh8ahlih-neovim-providers/bin/neovim-ruby-host'" "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                    start_suspended true
                }
            }
        "#;

        let expected = r#"
            tab name="dr mediactl" focus=true hide_floating_panes=true {
                pane command="direnv" cwd="mediactl" size="38%" {
                    args "exec" "." "nvim" "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                    start_suspended true
                }
            }
        "#;

        let (result, changes) = process_kdl_content(input);
        dbg!(&result);
        dbg!(&changes);

        assert_eq!(result.trim(), expected.trim());
    }

    #[test]
    fn test_tab_dr_direnv_single_pane2() {
        let input = r#"
            tab name="dr mediactl" focus=true hide_floating_panes=true {
                pane command="/home/zach/.nix-profile/bin/nvim" cwd="mediactl" size="38%" {
                    args "--cmd" "lua vim.opt.packpath:prepend('/nix/store/hsw6sf19akpssa4gq9krkrikx2ivcvkg-mnw-configDir');vim.opt.runtimepath:prepend('/nix/store/hsw6sf19akpssa4gq9krkrikx2ivcvkg-mnw-configDir');vim.g.loaded_node_provider=0;vim.g.loaded_perl_provider=0;vim.g.loaded_python_provider=0;vim.g.loaded_python3_provider=0;vim.g.ruby_host_prog='/nix/store/iv1p4x799nsx06pbg8vhgx1lwh8ahlih-neovim-providers/bin/neovim-ruby-host'" "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                    start_suspended true
                }
            }
        "#;

        let expected = r#"
            tab name="dr mediactl" focus=true hide_floating_panes=true {
                pane command="direnv" cwd="mediactl" size="38%" {
                    args "exec" "." "nvim" "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                    start_suspended true
                }
            }
        "#;

        let (result, changes) = process_kdl_content(input);
        dbg!(&result);
        dbg!(&changes);

        assert_eq!(result.trim(), expected.trim());
    }

    #[test]
    fn test_tab_dr_direnv_nvim_already_simplified() {
        let input = r#"
            tab name="dr mediactl" focus=true hide_floating_panes=true {
                pane command="nvim" cwd="/home/zach/Dev/mediactl" size="38%" {
                    args "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                    start_suspended true
                }
            }
        "#;

        let expected = r#"
            tab name="dr mediactl" focus=true hide_floating_panes=true {
                pane command="direnv" cwd="/home/zach/Dev/mediactl" size="38%" {
                    args "exec" "." "nvim" "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                    start_suspended true
                }
            }
        "#;

        let (result, changes) = process_kdl_content(input);
        dbg!(&result);
        dbg!(&changes);

        assert_eq!(result.trim(), expected.trim());
    }

    #[test]
    fn test_tab_dr_direnv_fully_already_simplified() {
        let input = r#"
            tab name="dr mediactl" focus=true hide_floating_panes=true {
                pane command="direnv" cwd="/home/zach/Dev/mediactl" size="38%" {
                    args "exec" "." "nvim" "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                    start_suspended true
                }
            }
        "#;

        let (result, changes) = process_kdl_content(input);
        dbg!(&result);
        dbg!(&changes);

        assert_eq!(result.trim(), input.trim());
    }

    #[test]
    fn test_tab_dr_direnv_multi_pane() {
        let input = r#"
            tab name="dr mediactl" focus=true hide_floating_panes=true {
                pane size=1 borderless=true {
                    plugin location="zellij:tab-bar"
                }
                pane split_direction="vertical" {
                    pane command="bacon" cwd="mediactl" size="33%" {
                        start_suspended true
                    }
                    pane command="/home/zach/.nix-profile/bin/nvim" cwd="mediactl" size="38%" {
                        args "--cmd" "lua vim.opt.packpath:prepend('/nix/store/hsw6sf19akpssa4gq9krkrikx2ivcvkg-mnw-configDir');vim.opt.runtimepath:prepend('/nix/store/hsw6sf19akpssa4gq9krkrikx2ivcvkg-mnw-configDir');vim.g.loaded_node_provider=0;vim.g.loaded_perl_provider=0;vim.g.loaded_python_provider=0;vim.g.loaded_python3_provider=0;vim.g.ruby_host_prog='/nix/store/iv1p4x799nsx06pbg8vhgx1lwh8ahlih-neovim-providers/bin/neovim-ruby-host'" "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                        start_suspended true
                    }
                    pane size="28%" {
                        pane command="target/debug/mediactl" cwd="mediactl" focus=true size="60%" {
                            args "tui"
                            start_suspended true
                        }
                        pane command="claude" cwd="mediactl" size="40%" {
                            start_suspended true
                        }
                    }
                }
                pane size=1 borderless=true {
                    plugin location="zellij:status-bar"
                }
            }
        "#;

        let expected = r#"
            tab name="dr mediactl" focus=true hide_floating_panes=true {
                pane size=1 borderless=true {
                    plugin location="zellij:tab-bar"
                }
                pane split_direction="vertical" {
                    pane command="direnv" cwd="mediactl" size="33%" {
                        args "exec" "." "bacon"
                        start_suspended true
                    }
                    pane command="direnv" cwd="mediactl" size="38%" {
                        args "exec" "." "nvim" "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                        start_suspended true
                    }
                    pane size="28%" {
                        pane command="direnv" cwd="mediactl" focus=true size="60%" {
                            args "exec" "." "target/debug/mediactl" "tui"
                            start_suspended true
                        }
                        pane command="direnv" cwd="mediactl" size="40%" {
                            args "exec" "." "claude"
                            start_suspended true
                        }
                    }
                }
                pane size=1 borderless=true {
                    plugin location="zellij:status-bar"
                }
            }
        "#;

        let (result, changes) = process_kdl_content(input);
        dbg!(&result);
        dbg!(&changes);

        assert_eq!(result.trim(), expected.trim());
    }

    #[test]
    fn test_multi_tab_one_dr_direnv() {
        let input = r#"
            tab name="dr mediactl" hide_floating_panes=true {
                pane command="/home/zach/.nix-profile/bin/nvim" cwd="/home/zach/Dev/mediactl" size="38%" {
                    args "--cmd" "lua vim.opt.packpath:prepend('/nix/store/hsw6sf19akpssa4gq9krkrikx2ivcvkg-mnw-configDir');vim.opt.runtimepath:prepend('/nix/store/hsw6sf19akpssa4gq9krkrikx2ivcvkg-mnw-configDir');vim.g.loaded_node_provider=0;vim.g.loaded_perl_provider=0;vim.g.loaded_python_provider=0;vim.g.loaded_python3_provider=0;vim.g.ruby_host_prog='/nix/store/iv1p4x799nsx06pbg8vhgx1lwh8ahlih-neovim-providers/bin/neovim-ruby-host'" "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                    start_suspended true
                }
            }
            tab name="mediactl" hide_floating_panes=true {
                pane command="/home/zach/.nix-profile/bin/nvim" cwd="/home/zach/Dev/mediactl" size="38%" {
                    args "--cmd" "lua vim.opt.packpath:prepend('/nix/store/hsw6sf19akpssa4gq9krkrikx2ivcvkg-mnw-configDir');vim.opt.runtimepath:prepend('/nix/store/hsw6sf19akpssa4gq9krkrikx2ivcvkg-mnw-configDir');vim.g.loaded_node_provider=0;vim.g.loaded_perl_provider=0;vim.g.loaded_python_provider=0;vim.g.loaded_python3_provider=0;vim.g.ruby_host_prog='/nix/store/iv1p4x799nsx06pbg8vhgx1lwh8ahlih-neovim-providers/bin/neovim-ruby-host'" "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                    start_suspended true
                }
            }
        "#;

        let expected = r#"
            tab name="dr mediactl" hide_floating_panes=true {
                pane command="direnv" cwd="/home/zach/Dev/mediactl" size="38%" {
                    args "exec" "." "nvim" "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                    start_suspended true
                }
            }
            tab name="mediactl" hide_floating_panes=true {
                pane command="nvim" cwd="/home/zach/Dev/mediactl" size="38%" {
                    args "notes.md" "working_notes.md" "src/main.rs" "src/lib.rs"
                    start_suspended true
                }
            }
        "#;

        let (result, changes) = process_kdl_content(input);
        dbg!(&result);
        dbg!(&changes);

        assert_eq!(result.trim(), expected.trim());
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
