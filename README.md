# OCI2Git

A Rust application that converts container images (Docker, etc.) to Git repositories. Each container layer is represented as a Git commit, preserving the history and structure of the original image.

## Features

- Analyze Docker images and extract layer information
- Create a Git repository where each image layer is represented as a commit
- Support for empty layers (ENV, WORKDIR, etc.) as empty commits
- Complete metadata extraction to Markdown format
- Extensible architecture for supporting different container engines

## Installation

```bash
cargo install --path .
```

## Usage

```bash
oci2git [OPTIONS] <IMAGE>
```

Arguments:
  `<IMAGE>`  Image name to convert (e.g., 'ubuntu:latest')

Options:
  `-o, --output <OUTPUT>`  Output directory for Git repository [default: ./container_repo]
  `-e, --engine <ENGINE>`  Container engine to use (docker, nerdctl) [default: docker]
  `-h, --help`            Print help information
  `-V, --version`         Print version information

## Example

```bash
oci2git -o ./ubuntu-repo ubuntu:latest
```

This will create a Git repository in `./ubuntu-repo` containing:
- `Image.md` - Complete metadata about the image in Markdown format
- `rootfs/` - The filesystem content from the container

The Git history reflects the container's layer history:
- The first commit contains only the `Image.md` file with full metadata
- Each subsequent commit represents a layer from the original image
- Commits include the Dockerfile command as the commit message

## Repository Structure

```
repository/
├── .git/
├── Image.md     # Complete image metadata
└── rootfs/      # Filesystem content from the container
```

## Architecture

The application uses a trait-based approach to abstract container engines:

- `ContainerEngine` trait defines methods for working with container images
- Implementation for Docker with a stub for future nerdctl support
- Modular design with separate modules for container operations, Git operations, and conversion logic

## Requirements

- Rust 2021 edition
- Docker CLI (for Docker engine support)
- Git

## License

MIT