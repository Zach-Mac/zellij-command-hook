# zellij-command-hook

Simplifies verbose nvim wrapper commands in Zellij. Written in Rust for fun.

## Problem

When using [Zellij](https://github.com/zellij-org/zellij/), NixOS wrappers (such
as [nvf](https://github.com/notashelf/nvf)) generate extremely verbose neovim
commands that get picked up by Zellij's command discovery and cause issues on
resurrection.

This tool strips them down to just `nvim file1.txt file2.txt`.

Built for nvim but could be adapted for other wrapped commands. Will prob do
this in the future.

## Installation

### Nix Flake

Add as a flake input:

```nix
zellij-command-hook = {
    url = "github:Zach-Mac/zellij-command-hook";
    inputs.nixpkgs.follows = "nixpkgs";
};
```

Then add to your packages (NixOS or home-manager):

```nix
environment.systemPackages = with pkgs; [
    inputs."zellij-command-hook".packages.x86_64-linux.default
];
```

```nix
home.packages = with pkgs; [
    inputs."zellij-command-hook".packages.x86_64-linux.default
];
```

### From source

```bash
cargo build --release
cp target/release/zellij-command-hook ~/.local/bin/
```

## Usage

### Zellij command hook (primary)

Add to your `config.kdl`:

```kdl
post_command_discovery_hook "zellij-command-hook"
```

When a session is resurrected, Zellij sets the `RESURRECT_COMMAND` env var and
runs the hook. The tool outputs the simplified command.

### Scan existing layouts (fallback)

The command hook doesn't always work properly, haven't figured out why yet. Use
`scan-layouts` to clean up existing session files:

```bash
# Scan default location (~/.cache/zellij)
zellij-command-hook scan-layouts

# Preview changes without modifying files
zellij-command-hook scan-layouts --dry-run

# Verbose output
zellij-command-hook --verbose scan-layouts
```

## CLI Reference

```
Usage: zellij-command-hook [OPTIONS] [COMMAND]

Commands:
  scan-layouts  Scan and simplify session layout files
  help          Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose  Verbose output
  -h, --help     Print help


Usage: zellij-command-hook scan-layouts [OPTIONS] [PATH]

Arguments:
  [PATH]  Path to scan [default: ~/.cache/zellij]

Options:
  -d, --dry-run  Dry run - don't make changes, just show what would change
  -q, --quiet    Quiet - don't print anything
  -v, --verbose  Verbose output
  -h, --help     Print help
```

## How It Works

1. Recursively finds `session-layout.kdl` files in the target directory
2. Parses KDL `pane` blocks containing nvim commands
3. Extracts actual filenames from the command arguments (filtering out Lua code
   and paths)
4. Rewrites the pane blocks with simplified `nvim file1 file2` commands
5. Logs changes to `/tmp/nvim-resurrect.log`
