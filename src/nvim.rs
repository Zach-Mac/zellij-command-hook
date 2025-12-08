/// Formats a long nvim command into a simple "nvim filename" format.
/// Extracts filenames from the end of the command, ignoring flags and options.
pub fn format_nvim(command: &str) -> String {
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
        if part.ends_with("nvim") {
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

    format!("nvim {}", files)
}

/// Checks if a string could be a valid filename.
/// Returns false for forbidden characters that aren't allowed in POSIX filenames.
fn could_be_filename(s: &str) -> bool {
    if s.as_bytes().contains(&0) {
        return false;
    }

    let forbidden = ['<', '>', '"', ':', '|', '?', ';', '='];
    if s.chars().any(|c| forbidden.contains(&c)) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_filename() {
        let cases = [
            ("", true),
            ("foo.txt", true),
            ("a/b/c/foo.txt", true),
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

    #[test]
    fn test_format_nvim() {
        let cases = [
            (
                "/home/zach/.nix-profile/bin/nvim --cmd lua vim.opt.packpath:prepend('/nix/store/7fcfmii0vli2ncrgw8phdj1r7zcxf0fc-mnw-configDir');vim.opt.runtimepath:prepend('/nix/store/7fcfmii0vli2ncrgw8phdj1r7zcxf0fc-mnw-configDir');vim.g.loaded_node_provider=0;vim.g.loaded_perl_provider=0;vim.g.loaded_python_provider=0;vim.g.loaded_python3_provider=0;vim.g.ruby_host_prog='/nix/store/4pm6h00i8jizd1vcfh90gkfsipd634rc-neovim-providers/bin/neovim-ruby-host' asdf3 asdf4",
                "nvim asdf3 asdf4",
            ),
            ("nvim asdf", "nvim asdf"),
        ];

        for (input, expected) in cases.iter() {
            dbg!(input);
            assert_eq!(format_nvim(input), *expected, "Failed on input: {input}");
        }
    }
}
