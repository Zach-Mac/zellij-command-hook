# zellij-command-hook

Simplifies verbose nvim wrapper commands in Zellij and adds direnv support for
project tabs. Written in Rust for fun. Heavy AI help cuz I have more important
projects I wanna work on.

## Problem

1. When using [Zellij](https://github.com/zellij-org/zellij/), NixOS wrappers
   (such as [nvf](https://github.com/notashelf/nvf)) generate extremely verbose
   neovim commands that get picked up by Zellij's command discovery and cause
   issues on resurrection.
2. When resurrecting a Zellij session, panes with commands are suspended.
   Normally you just press Enter to resume. But if your project uses direnv, the
   environment won't be loaded yet, you need to manually reload direnv first.

This tool:

- Strips verbose nvim commands down to just `nvim file1.txt file2.txt`
- Wraps commands in tabs starting with `dr` with `direnv exec .`

By wrapping commands with `direnv exec .`, you can just press Enter and the
command runs with the correct project environment automatically.

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

### Zellij command hook

Add to your `config.kdl`:

```kdl
post_command_discovery_hook "zellij-command-hook"
```

When a session is resurrected, Zellij sets the `RESURRECT_COMMAND` env var and
runs the hook. The tool outputs the simplified command.

### Scan existing layouts

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
2. Parses KDL using a proper KDL parser
3. For tabs starting with `dr`: wraps all commands with `direnv exec .`
4. For other tabs: simplifies verbose nvim commands to `nvim file1 file2`
5. Preserves all other attributes (cwd, focus, size, etc.)
6. Logs changes to `/tmp/nvim-resurrect.log`

### Example

Tab named `dr myproject`:

- `bacon` → `direnv exec . bacon`
- `/nix/store/.../nvim --cmd "lua..." file.rs` → `direnv exec . nvim file.rs`

Tab named `myproject` (no `dr` prefix):

- `/nix/store/.../nvim --cmd "lua..." file.rs` → `nvim file.rs`
- Other commands unchanged

## TODO

- direnv support in the actual post_command_discovery_hook
- direnv support enable/disable flag
- kdl dr tests also assert changes vec is correct
- support scan-layouts working on other file names
