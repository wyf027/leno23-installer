use std::{
    ffi::OsString,
    fmt, fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use tar::EntryType;

const DEFAULT_REPO: &str = "wyf027/wyf027";
const DEFAULT_REF: &str = "HEAD";
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Parser)]
#[command(
    name = "leno23-install",
    version,
    about = "Download wyf027/wyf027 resources into a target directory.",
    after_help = "Examples:\n  leno23-install ~/Code/wyf027\n  leno23-install ./wyf027 --ref main --force\n  leno23-install ./snapshot --repo wyf027/wyf027 --keep-root"
)]
struct Cli {
    /// Directory where the repository contents should be installed.
    #[arg(value_name = "TARGET_DIR")]
    target_dir: PathBuf,

    /// GitHub repository in owner/name form.
    #[arg(long, default_value = DEFAULT_REPO, value_parser = parse_repo)]
    repo: Repo,

    /// Git ref to download. HEAD follows the repository default branch.
    #[arg(long = "ref", default_value = DEFAULT_REF, value_name = "REF")]
    reference: String,

    /// Allow installing into a non-empty directory and overwriting files.
    #[arg(long)]
    force: bool,

    /// Keep GitHub's archive root folder, for example wyf027-HEAD/.
    #[arg(long)]
    keep_root: bool,

    /// Print what would be downloaded without writing files.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Repo {
    owner: String,
    name: String,
}

impl Repo {
    fn archive_url(&self, reference: &str) -> String {
        format!(
            "https://codeload.github.com/{}/{}/tar.gz/{}",
            self.owner, self.name, reference
        )
    }
}

impl fmt::Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

impl FromStr for Repo {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        let (owner, name) = value
            .split_once('/')
            .ok_or_else(|| anyhow!("repo must be in owner/name form"))?;

        validate_github_name(owner, "owner")?;
        validate_github_name(name, "repo")?;

        Ok(Self {
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli)
}

fn run(cli: Cli) -> Result<()> {
    validate_ref(&cli.reference)?;

    let url = cli.repo.archive_url(&cli.reference);
    let target_dir = normalize_target(&cli.target_dir)?;

    if cli.dry_run {
        println!("Repository : {}", cli.repo);
        println!("Reference  : {}", cli.reference);
        println!("Archive URL: {url}");
        println!("Target dir : {}", target_dir.display());
        return Ok(());
    }

    prepare_target_dir(&target_dir, cli.force)?;

    println!("Installing {}@{} ...", cli.repo, cli.reference);
    println!("Target: {}", target_dir.display());

    let installed = download_and_unpack(&url, &target_dir, cli.keep_root, cli.force)
        .with_context(|| format!("failed to install archive from {url}"))?;

    println!(
        "Done. Installed {installed} archive entries into {}",
        target_dir.display()
    );

    Ok(())
}

fn parse_repo(value: &str) -> Result<Repo> {
    value.parse()
}

fn validate_github_name(value: &str, label: &str) -> Result<()> {
    if value.is_empty() {
        bail!("{label} cannot be empty");
    }

    if value.starts_with('.') || value.ends_with('.') {
        bail!("{label} cannot start or end with '.'");
    }

    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        bail!("{label} can only contain ASCII letters, numbers, '.', '_' and '-'");
    }

    Ok(())
}

fn validate_ref(reference: &str) -> Result<()> {
    if reference.trim().is_empty() {
        bail!("--ref cannot be empty");
    }

    if reference
        .bytes()
        .any(|byte| byte.is_ascii_control() || matches!(byte, b'?' | b'#'))
    {
        bail!("--ref contains unsupported characters");
    }

    Ok(())
}

fn normalize_target(target_dir: &Path) -> Result<PathBuf> {
    if target_dir.as_os_str().is_empty() {
        bail!("target directory cannot be empty");
    }

    if target_dir.is_absolute() {
        return Ok(target_dir.to_path_buf());
    }

    Ok(std::env::current_dir()
        .context("failed to read current directory")?
        .join(target_dir))
}

fn prepare_target_dir(target_dir: &Path, force: bool) -> Result<()> {
    if target_dir.exists() {
        if fs::symlink_metadata(target_dir)
            .with_context(|| format!("failed to inspect {}", target_dir.display()))?
            .file_type()
            .is_symlink()
        {
            bail!(
                "target directory must not be a symlink: {}",
                target_dir.display()
            );
        }

        let metadata = fs::metadata(target_dir)
            .with_context(|| format!("failed to inspect {}", target_dir.display()))?;

        if !metadata.is_dir() {
            bail!(
                "target exists but is not a directory: {}",
                target_dir.display()
            );
        }

        if !force && fs::read_dir(target_dir)?.next().transpose()?.is_some() {
            bail!(
                "target directory is not empty: {}\nre-run with --force to overwrite matching files",
                target_dir.display()
            );
        }

        return Ok(());
    }

    fs::create_dir_all(target_dir)
        .with_context(|| format!("failed to create {}", target_dir.display()))
}

fn download_and_unpack(
    url: &str,
    target_dir: &Path,
    keep_root: bool,
    force: bool,
) -> Result<usize> {
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .get(url)
        .send()
        .with_context(|| format!("request failed: {url}"))?
        .error_for_status()
        .with_context(|| format!("GitHub returned an error for {url}"))?;

    let progress = download_progress(response.content_length())?;
    let reader = ProgressReader::new(response, progress.clone());
    let installed = unpack_tar_gz(reader, target_dir, keep_root, force);

    progress.finish_and_clear();

    installed
}

fn download_progress(total_bytes: Option<u64>) -> Result<ProgressBar> {
    let progress = match total_bytes {
        Some(total) => {
            let progress = ProgressBar::new(total);
            progress.set_style(ProgressStyle::with_template(
                "{spinner:.green} downloading [{bar:40.cyan/blue}] {bytes}/{total_bytes} {eta}",
            )?);
            progress
        }
        None => {
            let progress = ProgressBar::new_spinner();
            progress.set_style(ProgressStyle::with_template(
                "{spinner:.green} downloading {bytes}",
            )?);
            progress
        }
    };

    Ok(progress)
}

fn unpack_tar_gz<R: Read>(
    reader: R,
    target_dir: &Path,
    keep_root: bool,
    force: bool,
) -> Result<usize> {
    let decoder = GzDecoder::new(reader);
    let mut archive = tar::Archive::new(decoder);
    let mut installed = 0usize;

    for entry in archive
        .entries()
        .context("failed to read archive entries")?
    {
        let mut entry = entry.context("failed to read archive entry")?;
        let original_path = entry
            .path()
            .context("failed to read archive entry path")?
            .into_owned();

        let Some(relative_path) = archive_entry_target(&original_path, keep_root)? else {
            continue;
        };

        let destination = target_dir.join(&relative_path);
        ensure_safe_destination(target_dir, &relative_path, &destination, force)?;

        let entry_type = entry.header().entry_type();

        if entry_type == EntryType::Directory {
            fs::create_dir_all(&destination)
                .with_context(|| format!("failed to create {}", destination.display()))?;
            installed += 1;
            continue;
        }

        if !entry_type.is_file() {
            bail!(
                "archive entry type is not supported for {}",
                original_path.display()
            );
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        entry
            .unpack(&destination)
            .with_context(|| format!("failed to write {}", destination.display()))?;
        installed += 1;
    }

    Ok(installed)
}

fn archive_entry_target(path: &Path, keep_root: bool) -> Result<Option<PathBuf>> {
    let mut normal_parts = Vec::<OsString>::new();

    for component in path.components() {
        match component {
            Component::Normal(part) => normal_parts.push(part.to_os_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("unsafe archive path: {}", path.display());
            }
        }
    }

    let mut output = PathBuf::new();
    let parts = normal_parts.into_iter().skip(if keep_root { 0 } else { 1 });

    for part in parts {
        output.push(part);
    }

    if output.as_os_str().is_empty() {
        Ok(None)
    } else {
        Ok(Some(output))
    }
}

fn ensure_safe_destination(
    target_dir: &Path,
    relative_path: &Path,
    destination: &Path,
    force: bool,
) -> Result<()> {
    reject_symlink_ancestors(target_dir, relative_path)?;

    if let Ok(metadata) = fs::symlink_metadata(destination) {
        if metadata.file_type().is_symlink() {
            bail!("refusing to overwrite symlink at {}", destination.display());
        }

        if metadata.is_dir() {
            return Ok(());
        }

        if !force {
            bail!(
                "file already exists: {}\nre-run with --force to overwrite matching files",
                destination.display()
            );
        }
    }

    Ok(())
}

fn reject_symlink_ancestors(target_dir: &Path, relative_path: &Path) -> Result<()> {
    let mut current = target_dir.to_path_buf();

    for component in relative_path.components() {
        let Component::Normal(part) = component else {
            bail!(
                "unsafe archive path component in {}",
                relative_path.display()
            );
        };

        current.push(part);

        if current == target_dir.join(relative_path) {
            break;
        }

        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                bail!("refusing to write through symlink at {}", current.display());
            }
            Ok(metadata) if !metadata.is_dir() => {
                bail!(
                    "archive path conflicts with a non-directory at {}",
                    current.display()
                );
            }
            Ok(_) | Err(_) => {}
        }
    }

    Ok(())
}

struct ProgressReader<R> {
    inner: R,
    progress: ProgressBar,
}

impl<R> ProgressReader<R> {
    fn new(inner: R, progress: ProgressBar) -> Self {
        Self { inner, progress }
    }
}

impl<R: Read> Read for ProgressReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;

        if read > 0 {
            self.progress.inc(read as u64);
        }

        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn strips_github_archive_root_by_default() {
        let path = Path::new("wyf027-HEAD/sites/web3-interview-docs/index.html");
        let target = archive_entry_target(path, false).unwrap().unwrap();

        assert_eq!(
            target,
            PathBuf::from("sites/web3-interview-docs/index.html")
        );
    }

    #[test]
    fn can_keep_github_archive_root() {
        let path = Path::new("wyf027-HEAD/README.md");
        let target = archive_entry_target(path, true).unwrap().unwrap();

        assert_eq!(target, PathBuf::from("wyf027-HEAD/README.md"));
    }

    #[test]
    fn skips_archive_root_directory_when_stripping() {
        let path = Path::new("wyf027-HEAD");

        assert_eq!(archive_entry_target(path, false).unwrap(), None);
    }

    #[test]
    fn rejects_parent_directory_paths() {
        let path = Path::new("wyf027-HEAD/../escape");

        assert!(archive_entry_target(path, false).is_err());
    }

    #[test]
    fn rejects_leading_parent_directory_paths_even_when_stripping() {
        let path = Path::new("../escape");

        assert!(archive_entry_target(path, false).is_err());
    }

    #[test]
    fn rejects_absolute_paths_even_when_stripping() {
        let path = Path::new("/tmp/escape");

        assert!(archive_entry_target(path, false).is_err());
    }

    #[test]
    fn parses_owner_and_repo() {
        let repo: Repo = "wyf027/wyf027".parse().unwrap();

        assert_eq!(
            repo,
            Repo {
                owner: "wyf027".to_string(),
                name: "wyf027".to_string(),
            }
        );
    }

    #[test]
    fn rejects_invalid_repo_names() {
        assert!("wyf027".parse::<Repo>().is_err());
        assert!("wyf027/leet code".parse::<Repo>().is_err());
        assert!("wyf027/".parse::<Repo>().is_err());
    }

    #[test]
    fn rejects_control_characters_in_refs() {
        assert!(validate_ref("main").is_ok());
        assert!(validate_ref("feature/docs").is_ok());
        assert!(validate_ref("main\n").is_err());
        assert!(validate_ref("topic#fragment").is_err());
    }

    #[test]
    fn reject_symlink_ancestor_accepts_missing_paths() {
        let unique = OsString::from(format!("leno23-installer-test-{}", std::process::id()));
        let target = std::env::temp_dir().join(unique);

        reject_symlink_ancestors(&target, Path::new("nested/file.txt")).unwrap();
    }
}
