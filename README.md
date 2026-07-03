# leno23-installer

Rust CLI for downloading the contents of
[`wyf027/wyf027`](https://github.com/wyf027/wyf027) into a target directory.

It downloads GitHub's tarball archive directly, so the target machine does not
need `git`.

## Local Usage

```bash
cargo run -- ~/Code/wyf027
```

The command strips GitHub's archive root folder by default. For example,
`wyf027-HEAD/README.md` is installed as `README.md` in the target
directory.

Useful options:

```bash
# Follow the repository default branch.
leno23-install ~/Code/wyf027

# Install a specific branch, tag, or commit.
leno23-install ~/Code/wyf027 --ref main

# Allow installing into a non-empty directory.
leno23-install ~/Code/wyf027 --force

# Keep GitHub's generated top-level archive folder.
leno23-install ~/Code/wyf027 --keep-root

# Preview the request without writing files.
leno23-install ~/Code/wyf027 --dry-run
```

## NPX Install

Install into the current directory:

```bash
npx wyf-skills
```

Choose a target directory:

```bash
npx wyf-skills ~/Code/wyf027
```

The command installs the CLI with `cargo install --git`, then runs
`leno23-install`.

## Bun-Style Install Script

Install into the current directory:

```bash
curl -fsSL https://github.com/wyf027/i/raw/main/i | bash
```

Choose a target directory:

```bash
curl -fsSL https://github.com/wyf027/i/raw/main/i | bash -s ~/Code/wyf027
```

The script also installs the CLI with `cargo install --git`, then runs
`leno23-install`.

If you publish the installer somewhere else, override the source repository:

```bash
curl -fsSL https://github.com/you/i/raw/main/i | \
  LENO23_INSTALLER_REPO_URL=https://github.com/you/your-installer \
  bash -s ~/Code/wyf027
```

## Build

```bash
cargo build --release
./target/release/leno23-install ~/Code/wyf027
```

## Safety Notes

- The target directory is created if it does not exist.
- Non-empty targets require `--force`.
- Archive entries containing `..` or absolute paths are rejected.
- Symlink and hardlink archive entries are rejected to avoid writing outside the
  target directory.
