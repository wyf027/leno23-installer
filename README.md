# leno23-installer

Rust CLI for downloading the contents of
[`wyf027/leno23`](https://github.com/wyf027/leno23) into a target directory.

It downloads GitHub's tarball archive directly, so the target machine does not
need `git`.

## Local Usage

```bash
cargo run -- ~/Code/leno23
```

The command strips GitHub's archive root folder by default. For example,
`leno23-HEAD/README.md` is installed as `README.md` in the target
directory.

Useful options:

```bash
# Follow the repository default branch.
leno23-install ~/Code/leno23

# Install a specific branch, tag, or commit.
leno23-install ~/Code/leno23 --ref main

# Allow installing into a non-empty directory.
leno23-install ~/Code/leno23 --force

# Keep GitHub's generated top-level archive folder.
leno23-install ~/Code/leno23 --keep-root

# Preview the request without writing files.
leno23-install ~/Code/leno23 --dry-run
```

## Bun-Style Install Script via esm.sh

After publishing this Rust project to `wyf027/leno23-installer`, esm.sh can
serve the one-letter `i` script directly from GitHub:

```bash
curl -L raw.esm.sh/gh/wyf027/leno23-installer/i.sh|sh
```

That installs into the current directory. To choose a target directory:

```bash
curl -L raw.esm.sh/gh/wyf027/leno23-installer/i.sh|sh -s ~/Code/leno23
```

The script installs the CLI with `cargo install --git`, then runs
`leno23-install`.

If you publish the installer somewhere else, override the source repository:

```bash
curl -L raw.esm.sh/gh/you/your-installer/i.sh | \
  LENO23_INSTALLER_REPO_URL=https://github.com/you/your-installer \
  sh -s ~/Code/leno23
```

## Build

```bash
cargo build --release
./target/release/leno23-install ~/Code/leno23
```

## Safety Notes

- The target directory is created if it does not exist.
- Non-empty targets require `--force`.
- Archive entries containing `..` or absolute paths are rejected.
- Symlink and hardlink archive entries are rejected to avoid writing outside the
  target directory.
