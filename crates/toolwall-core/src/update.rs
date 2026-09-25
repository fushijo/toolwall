//! Finding out whether there is a newer toolwall, and installing it.
//!
//! Through `curl` and `git` rather than an HTTP crate. Both are already
//! required (the importer clones with git, the readout polls with curl), and
//! a TLS stack would roughly double a build that people run on their own
//! machines.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

const REPO: &str = "https://github.com/fushijo/toolwall";
const LATEST: &str = "https://api.github.com/repos/fushijo/toolwall/releases/latest";

/// The version this binary was built as.
pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Where `update` keeps its own checkout.
///
/// Its own, deliberately. People install from a tarball, from a clone in
/// Downloads, or from a clone they have since deleted, and an updater that
/// only works if you still have the directory you installed from is an
/// updater that works for about a week.
pub fn checkout() -> Result<PathBuf> {
    let base = directories::BaseDirs::new().context("cannot determine home directory")?;
    Ok(base.data_local_dir().join("toolwall/src"))
}

/// The newest released version, or None when the check could not be made.
///
/// A failed check is not an error worth stopping for: no network, a rate
/// limit, GitHub being down. The caller says "could not check" and moves on.
pub fn latest() -> Result<String> {
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "10", "-H", "Accept: application/vnd.github+json", LATEST])
        .output()
        .context("could not run curl")?;

    if !out.status.success() {
        bail!("could not reach GitHub");
    }

    let body = String::from_utf8_lossy(&out.stdout);
    let tag = body
        .split("\"tag_name\"")
        .nth(1)
        .and_then(|rest| rest.split('"').nth(1))
        .context("no tag_name in GitHub's answer")?;

    Ok(tag.trim_start_matches('v').to_string())
}

/// Is `latest` newer than `current`, comparing numbers rather than text?
///
/// "0.3.10" is newer than "0.3.9", which a string comparison gets backwards.
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parts = |v: &str| -> Vec<u32> {
        v.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect()
    };

    let (a, b) = (parts(latest), parts(current));
    for index in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(index).copied().unwrap_or(0), b.get(index).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

/// Fetch, build and install the newest release.
///
/// Each step's output is handed to `say` as it finishes, so a GUI can show
/// progress on something that takes a couple of minutes.
pub fn run(say: &mut dyn FnMut(&str)) -> Result<String> {
    for tool in ["git", "cargo"] {
        if !which(tool) {
            bail!("{tool} is not installed, so toolwall cannot update itself");
        }
    }

    let dir = checkout()?;

    if dir.join(".git").is_dir() {
        say("Fetching...");
        step(Command::new("git").arg("-C").arg(&dir).args(["fetch", "--tags", "--quiet"]))?;
        step(Command::new("git").arg("-C").arg(&dir).args(["reset", "--hard", "--quiet", "origin/HEAD"]))?;
    } else {
        say("Downloading...");
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        }
        let _ = std::fs::remove_dir_all(&dir);
        step(Command::new("git").args(["clone", "--quiet", REPO]).arg(&dir))?;
    }

    say("Building...");
    step(Command::new("cargo")
        .current_dir(&dir)
        .args(["install", "--quiet", "--path", "crates/toolwall-cli", "--force"]))?;
    step(Command::new("cargo")
        .current_dir(&dir)
        .args(["install", "--quiet", "--path", "crates/toolwall-gui", "--force"]))?;

    say("Installing the runtime...");
    step(Command::new("sh").current_dir(&dir).arg("install.sh").env("TOOLWALL_NONINTERACTIVE", "1"))?;

    let version = std::fs::read_to_string(dir.join("Cargo.toml"))
        .ok()
        .and_then(|body| {
            body.lines()
                .find(|line| line.trim_start().starts_with("version = "))
                .and_then(|line| line.split('"').nth(1))
                .map(str::to_string)
        })
        .unwrap_or_else(|| "the newest version".to_string());

    Ok(version)
}

fn step(command: &mut Command) -> Result<()> {
    let out = command.output().with_context(|| format!("could not run {:?}", command.get_program()))?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = stderr.lines().rev().take(4).collect();
        bail!("{:?} failed: {}", command.get_program(), tail.into_iter().rev().collect::<Vec<_>>().join(" "));
    }

    Ok(())
}

fn which(exe: &str) -> bool {
    std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path).any(|dir| Path::new(&dir).join(exe).is_file())
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_as_numbers_not_as_text() {
        // "0.3.10" sorts before "0.3.9" as text, which would tell everyone
        // on 0.3.9 that they were up to date forever.
        assert!(is_newer("0.3.10", "0.3.9"));
        assert!(is_newer("0.4.0", "0.3.99"));
        assert!(is_newer("1.0.0", "0.9.9"));

        assert!(!is_newer("0.3.6", "0.3.6"));
        assert!(!is_newer("0.3.5", "0.3.6"));
        assert!(!is_newer("0.3.6", "0.4.0"));
    }

    #[test]
    fn a_tag_with_fewer_parts_still_compares() {
        assert!(is_newer("0.4", "0.3.9"));
        assert!(!is_newer("0.3", "0.3.0"));
    }
}
