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
    },

    /// Trip waywall's hot reload without changing anything.
    Reload,
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

        Cmd::Init { force } => {
            if store.path().exists() && !force {
                anyhow::bail!("{} already exists (use --force)", store.path().display());
            }
            store.save(&toolwall_core::Document::default())?;
            println!("wrote {}", store.path().display());
        }

        Cmd::Reload => {
            store.trigger_reload()?;
            println!("reload triggered");
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
