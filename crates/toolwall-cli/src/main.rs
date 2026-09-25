//! toolwall CLI.
//!
//! This exists before the GUI on purpose. It exercises the entire write path -
//! load, mutate, validate, atomic write, reload trigger - without any pixels.
//! If `toolwall set` does not apply live in a running session, the GUI was
//! never going to work either.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use toolwall_core::Store;

#[derive(Parser)]
#[command(name = "toolwall", version, about = "Configure waywall from the command line")]
struct Cli {
    /// Path to toolwall.json (defaults to the waywall config directory).
    #[arg(long, global = true)]
    config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Check the config without changing it.
    Validate,

    /// Print the whole document, or one dotted path.
    Get {
        /// e.g. modes.thin.resolution.width
        path: Option<String>,
    },

    /// Set one dotted path to a JSON value, then trigger a reload.
    Set {
        /// e.g. modes.thin.resolution.width
        path: String,
        /// JSON literal: 320, "thin", true, null
        value: String,
    },

    /// List the configured modes.
    Modes,

    /// Write a starter config to the config path.
    Init {
        #[arg(long)]
        force: bool,
        /// Size the starter config for this window, e.g. 2560x1440.
        #[arg(long, value_name = "WxH")]
        screen: Option<String>,
    },

    /// Tell toolwall how big waywall's window is, and move the overlays to suit.
    ///
    /// The number you want is the Display line in F3, not your monitor's
    /// resolution. On a scaled desktop they are different, and the window is
    /// the smaller of the two.
    Screen {
        width: u32,
        height: u32,
    },

    /// Trip waywall's hot reload without changing anything.
    Reload,

    /// Update toolwall to the newest release.
    Update {
        /// Say what is available and stop.
        #[arg(long)]
        check: bool,
        /// Reinstall even when there is nothing newer, for repairing a
        /// half-finished install.
        #[arg(long)]
        force: bool,
    },

    /// Write the custom keyboard layout out as an XKB symbols file.
    ///
    /// The document is where the layout lives; this is what puts it somewhere
    /// xkbcommon will find it. The editor does this on save, so this is for
    /// setting a machine up from a config someone shared.
    Layout {
        /// Print the file instead of writing it.
        #[arg(long)]
        print: bool,
    },
}

/// `2560x1440` or `2560 1440`, because people type both.
fn parse_screen(text: &str) -> Result<(u32, u32)> {
    let cleaned = text.trim().to_lowercase();
    let (w, h) = cleaned
        .split_once(['x', ' '])
        .ok_or_else(|| anyhow::anyhow!("expected something like 2560x1440, got {text:?}"))?;

    let width: u32 = w.trim().parse().map_err(|_| anyhow::anyhow!("bad width {w:?}"))?;
    let height: u32 = h.trim().parse().map_err(|_| anyhow::anyhow!("bad height {h:?}"))?;

    if width < 320 || height < 240 {
        anyhow::bail!("{width}x{height} is too small to be a window");
    }
    Ok((width, height))
}

#[cfg(test)]
mod tests {
    use super::parse_screen;

    #[test]
    fn a_screen_size_can_be_typed_either_way() {
        assert_eq!(parse_screen("2560x1440").unwrap(), (2560, 1440));
        assert_eq!(parse_screen("2560 1440").unwrap(), (2560, 1440));
        assert_eq!(parse_screen(" 2560X1440 ").unwrap(), (2560, 1440));
    }

    #[test]
    fn nonsense_is_refused_rather_than_guessed_at() {
        assert!(parse_screen("2560").is_err(), "one number is not a size");
        assert!(parse_screen("wide x tall").is_err());
        assert!(parse_screen("").is_err());

        // Small enough to be a typo for something, and a config sized for it
        // would be unusable.
        assert!(parse_screen("16x9").is_err(), "an aspect ratio is not a size");
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let store = match cli.config {
        Some(path) => Store::new(path),
        None => Store::at_default_path()?,
    };

    match cli.command {
        Cmd::Validate => {
            store.load()?;
            println!("ok: {}", store.path().display());
        }

        Cmd::Get { path } => {
            let doc = store.load()?;
            let value = serde_json::to_value(&doc)?;
            match path {
                None => println!("{}", serde_json::to_string_pretty(&value)?),
                Some(p) => {
                    let found = pointer_get(&value, &p)
                        .with_context(|| format!("no such path: {p}"))?;
                    println!("{}", serde_json::to_string_pretty(found)?);
                }
            }
        }

        Cmd::Set { path, value } => {
            let doc = store.load()?;
            let mut json = serde_json::to_value(&doc)?;

            let parsed: serde_json::Value = serde_json::from_str(&value)
                .with_context(|| format!("value is not valid JSON: {value}"))?;

            pointer_set(&mut json, &path, parsed)
                .with_context(|| format!("no such path: {path}"))?;

            // Round-trip through the typed model so a bad edit is caught here
            // rather than inside a running session.
            let updated: toolwall_core::Document = serde_json::from_value(json)
                .context("edit produced an invalid document")?;

            store.save(&updated)?;
            println!("set {path} = {value}");
        }

        Cmd::Modes => {
            let doc = store.load()?;
            for mode in &doc.modes {
                let label = mode.label.as_deref().unwrap_or(&mode.id);
                println!(
                    "{:<12} {:>5}x{:<5}  {}",
                    mode.id, mode.resolution.width, mode.resolution.height, label
                );
            }
        }

        Cmd::Init { force, screen } => {
            if store.path().exists() && !force {
                anyhow::bail!("{} already exists (use --force)", store.path().display());
            }

            // The preset, not an empty document: a config with nothing in it
            // is not a starting point, it is a blank page.
            let mut doc = toolwall_core::preset::preset();

            match screen {
                Some(text) => {
                    let (w, h) = parse_screen(&text)?;
                    toolwall_core::preset::fit_to_screen(&mut doc, w, h);
                    println!("sized for {w}x{h}");
                }
                None => {
                    println!("sized for 1920x1080, which is the default");
                    if let Some(hint) = toolwall_core::screen::hint() {
                        println!("{hint}");
                    }
                    println!("Run `toolwall screen <width> <height>` if that is wrong.");
                }
            }

            store.save(&doc)?;
            println!("wrote {}", store.path().display());
        }

        Cmd::Screen { width, height } => {
            let mut doc = store.load()?;
            let was = doc.gui.screen;

            toolwall_core::preset::fit_to_screen(&mut doc, width, height);
            store.save(&doc)?;

            println!("was {}x{}, now {width}x{height}", was.w, was.h);
            println!(
                "{} overlay(s) anchored, so this stays right if the window changes again",
                doc.mirrors.len() + doc.images.len()
            );
        }

        Cmd::Layout { print } => {
            let doc = store.load()?;
            let layout = doc
                .input
                .custom_layout
                .as_ref()
                .context("this config has no custom layout; build one in the Layout tab")?;

            if print {
                print!("{}", toolwall_core::xkb::symbols_file(layout));
            } else {
                let path = toolwall_core::xkb::write_symbols(layout)?;
                println!("wrote {}", path.display());
                println!(
                    "set input.layout to {:?} for waywall to load it",
                    toolwall_core::xkb::sanitise_name(&layout.name)
                );
            }
        }

        Cmd::Reload => {
            store.trigger_reload()?;
            println!("reload triggered");
        }

        Cmd::Update { check, force } => {
            let current = toolwall_core::update::current();

            let latest = match toolwall_core::update::latest() {
                Ok(latest) => latest,
                Err(err) if !force => {
                    println!("on {current}, could not check for a newer one: {err:#}");
                    return Ok(());
                }
                Err(_) => "unknown".to_string(),
            };

            if !force && !toolwall_core::update::is_newer(&latest, current) {
                println!("on {current}, which is the newest");
                return Ok(());
            }

            if force {
                println!("reinstalling {latest} over {current}");
            } else {
                println!("{latest} is out, you are on {current}");
            }

            if check {
                return Ok(());
            }

            let installed = toolwall_core::update::run(&mut |step| println!("{step}"))?;
            println!("updated to {installed}");
            println!("restart waywall to pick up the new runtime");
        }
    }

    Ok(())
}

/// Resolve a dotted path, treating arrays-of-objects as maps keyed by `id`.
///
/// `modes.thin.resolution.width` is friendlier than `modes.0.resolution.width`
/// and survives reordering.
fn pointer_get<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = match current {
            serde_json::Value::Object(map) => map.get(segment)?,
            serde_json::Value::Array(items) => items.iter().find(|item| {
                item.get("id").and_then(|id| id.as_str()) == Some(segment)
            })?,
            _ => return None,
        };
    }
    Some(current)
}

fn pointer_set(value: &mut serde_json::Value, path: &str, new: serde_json::Value) -> Option<()> {
    let (parent_path, leaf) = match path.rsplit_once('.') {
        Some((parent, leaf)) => (parent, leaf),
        None => ("", path),
    };

    let parent = if parent_path.is_empty() {
        value
    } else {
        pointer_get_mut(value, parent_path)?
    };

    match parent {
        serde_json::Value::Object(map) => {
            map.insert(leaf.to_string(), new);
            Some(())
        }
        _ => None,
    }
}

fn pointer_get_mut<'a>(
    value: &'a mut serde_json::Value,
    path: &str,
) -> Option<&'a mut serde_json::Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = match current {
            serde_json::Value::Object(map) => map.get_mut(segment)?,
            serde_json::Value::Array(items) => items.iter_mut().find(|item| {
                item.get("id").and_then(|id| id.as_str()) == Some(segment)
            })?,
            _ => return None,
        };
    }
    Some(current)
}
