# devspace

Sandboxed development environments with UI passthrough.

## Features

- Containerized development environments using Docker or Podman
- GUI application support (X11 and Wayland)
- Optional init scripts for container setup
- Automatic workspace mounting
- Verbose mode for debugging

## Installation

Download the latest binary from the [releases page](https://github.com/gdotdesign/devspace/releases).

```bash
# Download and install
wget https://github.com/gdotdesign/devspace/releases/download/v1.0.0/devspace-linux-x86_64
chmod +x devspace-linux
sudo mv devspace-linux /usr/local/bin/devspace
```

Or install from source:

```bash
cargo install --path .
```

## Usage

Create a `.devspace.toml` in your project directory:

```toml
image = "alpine:latest"
name = "myproject"
shell = "sh"
gui = false
ports = ["8080:80", "3000:3000"]

init = """
apk add --no-cache git vim
"""
```

### Commands

- `devspace exec [-v] <command>` - Run a command in the container
- `devspace enter [-v]` - Create and enter the container
- `devspace stop [-v]` - Stop the running container
- `devspace status [-v]` - Show container status
- `devspace remove [-v]` - Remove the container
- `devspace version` - Show version information
- `devspace init` - Create a sample config file

## Configuration

- `privileged` - Run container with privileged mode (default: `false`)
- `name` - Container name (alphanumeric, hyphens, underscores only)
- `init` - Init script to run on container creation
- `gui` - Enable GUI passthrough (default: `false`)
- `shell` - Shell to use (default: `sh`)
- `image` - Container image to use
- `ports` - List of port mappings to expose (e.g., `["8080:80", "3000:3000"]`)
- `user_mapping` - Map host user into the container (default: `true`); set to `false` to disable all user mapping
- `chown_dirs` - List of container directories to `chown` to the mapped user on creation (default: `[]`); only applies when `user_mapping = true`

## Key Behaviors

- **User mapping** - Automatically maps the host user into the container so files created inside have correct ownership. Uses `--userns=keep-id` on Podman and `--user UID:GID` on Docker.

## Requirements

- Docker or Podman
- Linux
