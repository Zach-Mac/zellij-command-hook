use std::{env, path::Path};

fn main() {
    let command = env::var("RESURRECT_COMMAND").unwrap_or_default();
    // dbg!(&command);
    let formatted = format_nvim(&command);
    println!("{formatted}");
}

fn format_nvim(command: &str) -> String {
    // split by space. first entry, check if ends with nvim
    // if so, find file names by looking in reverse order through split
    // stop at first entry that isn't a file.
    let parts: Vec<&str> = command.split(' ').collect();
    if parts.is_empty() {
        return command.to_string();
    }
    let first = parts[0];
    if !first.ends_with("nvim") && !first.ends_with("nvim.exe") {
        return command.to_string();
    }

    let mut file_names = Vec::new();
    for part in parts.iter().rev() {
        if part.starts_with('-') {
            break;
        }
        if could_be_filename(part) {
            file_names.push(*part);
        } else {
            break;
        }
    }

    let files = file_names
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<&str>>()
        .join(" ");

    return format!("nvim {}", files);
}

fn could_be_filename(s: &str) -> bool {
    if s.as_bytes().contains(&0) {
        return false;
    } // NUL not allowed in POSIX filenames

    let forbidden = ['<', '>', '"', ':', '|', '?', ';', '='];
    if s.chars().any(|c| forbidden.contains(&c)) {
        return false;
    }

    true
}

mod tests {
    use super::*;
    #[test]
    fn test_looks_like_filename() {
        let cases = [
            ("", true),
            ("foo.txt", true),
            ("a/b/c/foo.txt", true),
            // ("C:\\path\\to\\file.md", true),
            ("..", true),
            (".", true),
            ("valid_name.rs", true),
            ("inva|id.txt", false),
            ("another:bad?.txt", false),
            ("just_a_name", true),
            ("\0invalid", false),
            (
                "lua vim.opt.packpath:prepend('/nix/store/142frdk214ir45zhxynmhpvh50khnc09-mnw-configDir');vim.opt.runtimepath:prepend('/nix/store/142frdk214ir45zhxynmhpvh50khnc09-mnw-configDir');vim.g.loaded_node_provider=0;vim.g.loaded_perl_provider=0;vim.g.loaded_python_provider=0;vim.g.loaded_python3_provider=0;vim.g.ruby_host_prog='/nix/store/vycxz6dfdb34mdcz0x15fflyqxavdz05-neovim-providers/bin/neovim-ruby-host'",
                false,
            ),
        ];
        for (input, expected) in cases.iter() {
            dbg!(input);
            assert_eq!(
                could_be_filename(input),
                *expected,
                "Failed on input: {input}"
            );
        }
    }
}
