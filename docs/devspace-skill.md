# Devspace Skill

## Description

Devspace is a tool for creating sandboxed development environments with UI passthrough using Docker or Podman containers. It provides a simple CLI for managing containerized dev environments with workspace mounting, GUI support, and init scripts.

## When to Use

Use this skill automatically when `.devspace.toml` is present in the current working directory.

## Commands

### Enter the container
```bash
devspace enter [-v]
```
Create and enter the container interactively. Use `-v` for verbose output.

### Run a command
```bash
devspace exec [-i] [-v] <command>...
```
Run a command in the container without entering.

**Important:** Commands are run through the configured shell (default: `sh`, configurable via `shell` in config). Use single quotes to prevent local shell expansion:

```bash
devspace exec 'echo $PATH'        # Container expands $PATH
devspace exec "echo $PATH"        # Local shell expands $PATH
devspace exec -i vim              # Interactive mode for TUI apps
```

Options:
- `-i, --interactive` - Run in interactive mode (attaches stdin for TUI apps like vim, top)
- `-v, --verbose` - Show runtime commands (docker/podman)

### Stop the container
```bash
devspace stop [-v]
```

### Show status
```bash
devspace status [-v]
```

### Remove the container
```bash
devspace remove [-v]
```

### Create config
```bash
devspace init
```
Create a sample `.devspace.toml` config file.

### Show version
```bash
devspace version
```

## Configuration

Configuration is stored in `.devspace.toml` in the project directory:

```toml
image = "alpine:latest"      # Container image
name = "myproject"           # Container name (alphanumeric, hyphens, underscores only)
shell = "sh"                 # Shell to use (default: sh)
gui = false                  # Enable GUI passthrough (default: false)
privileged = false           # Run in privileged mode (default: false)

init = """
apk add --no-cache git vim
"""
```

## Configuration Fields

- `image` - Container image to use (required)
- `name` - Container name (required, alphanumeric/hyphens/underscores only)
- `shell` - Shell to use for commands (default: `sh`)
- `gui` - Enable X11/Wayland GUI passthrough (default: `false`)
- `privileged` - Run container with privileged mode (default: `false`)
- `init` - Init script to run on container creation (optional)

## Requirements

- Docker or Podman
- Linux

## Key Behaviors

1. **Workspace mounting** - Current directory is mounted at `/workspace` in the container
2. **Shell invocation** - Commands run via `shell -i -c "command"` for proper environment setup
3. **Runtime detection** - Automatically detects between docker and podman
4. **Persistent containers** - Containers persist between runs until explicitly removed
