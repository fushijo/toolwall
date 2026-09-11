//! Loading, validating and persisting the config document.
//!
//! # The hot-reload trigger
//!
//! waywall watches for changes to **`.lua` files** in its config directory. It
//! does not watch `toolwall.json`. Writing the JSON alone therefore changes
//! nothing until waywall is restarted.
//!
//! So every save does two things:
//!
//!   1. Writes `toolwall.json` atomically (temp file + rename), so waywall can
//!      never observe a half-written document.
//!   2. Rewrites `toolwall_reload.lua` with a bumped counter, which trips the
//!      watcher and causes waywall to rebuild its Lua VM and re-read the JSON.
//!
//! Order matters. The trigger is written last, after the rename has landed.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::schema::{Command, Document, SCHEMA_VERSION};

/// Leaderboard rules cap any dimension here, and waywall's own texture limit
/// happens to be the same number.
const MAX_RESOLUTION: u32 = 16384;

pub const CONFIG_FILE: &str = "toolwall.json";
pub const RELOAD_FILE: &str = "toolwall_reload.lua";
pub const LAST_GOOD_SUFFIX: &str = ".last-good";

/// `$XDG_CONFIG_HOME/waywall`, falling back to `~/.config/waywall`.
pub fn waywall_config_dir() -> Result<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join("waywall"));
        }
    }

    let home = directories::BaseDirs::new()
        .context("cannot determine home directory")?
        .home_dir()
        .to_path_buf();

    Ok(home.join(".config").join("waywall"))
}

pub fn default_config_path() -> Result<PathBuf> {
    Ok(waywall_config_dir()?.join(CONFIG_FILE))
}

pub struct Store {
    path: PathBuf,
}

impl Store {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn at_default_path() -> Result<Self> {
        Ok(Self::new(default_config_path()?))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn dir(&self) -> &Path {
        self.path.parent().unwrap_or_else(|| Path::new("."))
    }

    /// When the config was last written, by anyone.
    ///
    /// The editor holds the whole document in memory and writes all of it, so
    /// a change made outside it would be silently reverted by the next edit.
    /// This is how it notices.
    pub fn modified(&self) -> Option<std::time::SystemTime> {
        fs::metadata(&self.path).ok()?.modified().ok()
    }

    pub fn load(&self) -> Result<Document> {
        let raw = fs::read_to_string(&self.path)
            .with_context(|| format!("reading {}", self.path.display()))?;

        let doc: Document = serde_json::from_str(&raw)
            .with_context(|| format!("parsing {}", self.path.display()))?;

        validate(&doc)?;
        Ok(doc)
    }

    /// Write the document and trip waywall's hot reload.
    pub fn save(&self, doc: &Document) -> Result<()> {
        validate(doc).context("refusing to write an invalid document")?;

        let body = serde_json::to_string_pretty(doc)? + "\n";

        fs::create_dir_all(self.dir())?;

        // Atomic replace: waywall must never see a partial file.
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut fh = fs::File::create(&tmp)
                .with_context(|| format!("creating {}", tmp.display()))?;
            fh.write_all(body.as_bytes())?;
            fh.sync_all()?;
        }
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("replacing {}", self.path.display()))?;

        self.trigger_reload()
    }

    /// Touch a `.lua` file so waywall's watcher fires.
    ///
    /// The content must actually change; some watchers coalesce identical
    /// writes, so a monotonic counter is used rather than a bare `touch`.
    pub fn trigger_reload(&self) -> Result<()> {
        let path = self.dir().join(RELOAD_FILE);

        let counter = fs::read_to_string(&path)
            .ok()
            .and_then(|s| {
                s.lines()
                    .find_map(|line| line.trim().strip_prefix("return ")?.trim().parse::<u64>().ok())
            })
            .unwrap_or(0)
            .wrapping_add(1);

        let body = format!(
            "-- Written by toolwall. Do not edit.\n\
             -- Bumping this counter is what makes waywall re-read toolwall.json,\n\
             -- because waywall only watches .lua files.\n\
             return {counter}\n"
        );

        fs::write(&path, body)
            .with_context(|| format!("writing {}", path.display()))?;

        Ok(())
    }

    pub fn last_good_path(&self) -> PathBuf {
        let mut name = self.path.as_os_str().to_owned();
        name.push(LAST_GOOD_SUFFIX);
        PathBuf::from(name)
    }
}

/// Where a validation problem lives, so the GUI can show it next to the item
/// that caused it rather than as one opaque status line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Document,
    Mode(String),
    Mirror(String),
    Image(String),
    Keybind(String),
    Ninb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub scope: Scope,
    pub message: String,
}

/// The ids a mode or a keybind is allowed to name.
///
/// Mirrors and images share one namespace because a mode lists them together
/// and a keybind toggles either, so a mirror and an image with the same id
/// would be indistinguishable at the point of use.
fn overlay_ids<'a>(doc: &'a Document, out: &mut Vec<Problem>) -> HashSet<&'a str> {
    let mut ids = HashSet::new();
    let mut seen = HashSet::new();

    let named = doc
        .mirrors
        .iter()
        .map(|m| (Scope::Mirror(m.id.clone()), "mirror", m.id.as_str()))
        .chain(
            doc.images
                .iter()
                .map(|i| (Scope::Image(i.id.clone()), "image", i.id.as_str())),
        );

    for (scope, kind, id) in named {
        if id.trim().is_empty() {
            out.push(Problem { scope: scope.clone(), message: "id must not be empty".into() });
        }
        if !seen.insert((kind, id)) {
            out.push(Problem { scope, message: format!("duplicate {kind} id {id:?}") });
        }
        ids.insert(id);
    }

    ids
}

/// Modes: unique, within the leaderboard limit, and naming overlays that exist.
fn mode_ids<'a>(doc: &'a Document, overlays: &HashSet<&str>, out: &mut Vec<Problem>)
    -> HashSet<&'a str>
{
    let mut ids = HashSet::new();

    for mode in &doc.modes {
        let scope = || Scope::Mode(mode.id.clone());

        if mode.id.trim().is_empty() {
            out.push(Problem { scope: scope(), message: "id must not be empty".into() });
        }
        if !ids.insert(mode.id.as_str()) {
            out.push(Problem {
                scope: scope(),
                message: format!("duplicate mode id {:?}", mode.id),
            });
        }

        // Leaderboard rules: no dimension above 16384.
        if mode.resolution.width > MAX_RESOLUTION || mode.resolution.height > MAX_RESOLUTION {
            out.push(Problem {
                scope: scope(),
                message: format!(
                    "exceeds the {MAX_RESOLUTION}px leaderboard limit ({}x{})",
                    mode.resolution.width, mode.resolution.height
                ),
            });
        }

        for id in mode.mirrors.iter().chain(mode.images.iter()) {
            if !overlays.contains(id.as_str()) {
                out.push(Problem {
                    scope: scope(),
                    message: format!("references unknown overlay {id:?}"),
                });
            }
        }
    }

    ids
}

/// The document's own references, which name things defined elsewhere in it.
fn document_references(
    doc: &Document,
    overlays: &HashSet<&str>,
    modes: &HashSet<&str>,
    out: &mut Vec<Problem>,
) {
    if let Some(default) = &doc.default_mode {
        if !modes.contains(default.as_str()) {
            out.push(Problem {
                scope: Scope::Document,
                message: format!("default_mode {default:?} is not a defined mode"),
            });
        }
    }

    for id in &doc.base_overlays {
        if !overlays.contains(id.as_str()) {
            out.push(Problem {
                scope: Scope::Document,
                message: format!("base_overlays references unknown overlay {id:?}"),
            });
        }
    }

    if doc.ninb.jar.trim().is_empty()
        && doc.keybinds.iter().any(|b| b.command == Command::NinbToggle)
    {
        out.push(Problem {
            scope: Scope::Ninb,
            message: "a key opens Ninjabrain Bot, but no jar is set".into(),
        });
    }
}

/// Keybinds: unique, bound to something, and not pointing at a deleted overlay.
fn keybinds(doc: &Document, overlays: &HashSet<&str>, out: &mut Vec<Problem>) {
    let mut inputs = HashSet::new();

    for bind in &doc.keybinds {
        let scope = || Scope::Keybind(bind.input.clone());

        // An overlay.toggle naming something that does not exist is a keybind
        // that silently does nothing, which is worth catching.
        if bind.command == Command::OverlayToggle {
            let named = bind
                .args
                .as_ref()
                .and_then(|a| a.get("overlay"))
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if named.is_empty() || !overlays.contains(named) {
                out.push(Problem {
                    scope: scope(),
                    message: "this key opens an overlay that no longer exists".into(),
                });
            }
        }

        if bind.input.trim().is_empty() {
            out.push(Problem { scope: scope(), message: "keybind has no input string".into() });
        }
        if !inputs.insert(bind.input.as_str()) {
            out.push(Problem {
                scope: scope(),
                message: format!("duplicate keybind for {:?}", bind.input),
            });
        }
    }
}

/// Every structural problem in the document, not just the first.
///
/// `validate` is this reduced to a pass/fail; the GUI uses the full list to
/// annotate individual modes, mirrors, images and keybinds inline.
pub fn problems(doc: &Document) -> Vec<Problem> {
    let mut out = Vec::new();

    if doc.version != SCHEMA_VERSION {
        out.push(Problem {
            scope: Scope::Document,
            message: format!("schema version {}, expected {}", doc.version, SCHEMA_VERSION),
        });
    }

    // Ids first: everything below is a question about whether a name resolves.
    let overlays = overlay_ids(doc, &mut out);
    let modes = mode_ids(doc, &overlays, &mut out);

    document_references(doc, &overlays, &modes, &mut out);
    keybinds(doc, &overlays, &mut out);

    out
}

/// Structural checks beyond what serde enforces.
pub fn validate(doc: &Document) -> Result<()> {
    if let Some(first) = problems(doc).first() {
        bail!("{}", first.message);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::*;

    fn doc_with_mode() -> Document {
        Document {
            modes: vec![Mode {
                id: "thin".into(),
                label: None,
                resolution: Resolution { width: 320, height: 1080 },
                sensitivity: None,
                toggle: true,
                mirrors: vec![],
                images: vec![],
            }],
            ..Default::default()
        }
    }

    #[test]
    fn rejects_dangling_overlay_reference() {
        let mut doc = doc_with_mode();
        doc.modes[0].mirrors.push("nope".into());
        assert!(validate(&doc).is_err());
    }

    #[test]
    fn rejects_oversized_resolution() {
        let mut doc = doc_with_mode();
        doc.modes[0].resolution.width = 20000;
        assert!(validate(&doc).is_err());
    }

    #[test]
    fn rejects_duplicate_keybinds() {
        let mut doc = doc_with_mode();
        for _ in 0..2 {
            doc.keybinds.push(Keybind {
                f3_safe: true,
                input: "Shift-T".into(),
                command: Command::ModeReset,
                args: None,
                label: None,
            });
        }
        assert!(validate(&doc).is_err());
    }

    #[test]
    fn accepts_a_valid_document() {
        assert!(validate(&doc_with_mode()).is_ok());
        assert!(problems(&doc_with_mode()).is_empty());
    }

    #[test]
    fn rejects_a_ninb_key_with_no_jar_configured() {
        let mut doc = doc_with_mode();
        doc.keybinds.push(Keybind {
            f3_safe: true,
            input: "grave".into(),
            command: Command::NinbToggle,
            args: None,
            label: None,
        });

        // The key would silently do nothing without a jar to launch.
        assert!(validate(&doc).is_err());

        doc.ninb.jar = "~/Ninjabrain-Bot.jar".into();
        assert!(validate(&doc).is_ok());
    }

    #[test]
    fn reports_every_problem_scoped_to_its_item() {
        let mut doc = doc_with_mode();
        doc.modes[0].resolution.width = 20000;
        doc.modes[0].mirrors.push("nope".into());
        doc.keybinds.push(Keybind {
            f3_safe: true,
            input: "Shift-T".into(),
            command: Command::ModeReset,
            args: None,
            label: None,
        });
        doc.keybinds.push(Keybind {
            f3_safe: true,
            input: "Shift-T".into(),
            command: Command::ModeReset,
            args: None,
            label: None,
        });

        let found = problems(&doc);

        // validate() only ever surfaces the first of these.
        assert_eq!(found.len(), 3, "{found:#?}");
        assert!(found.iter().all(|p| match &p.scope {
            Scope::Mode(id) => id == "thin",
            Scope::Keybind(input) => input == "Shift-T",
            _ => false,
        }));
    }
}
