//! Finding Minecraft's `options.txt`, so the setup does not have to ask you
//! to go and read a number out of a file.
//!
//! Only sensitivity is read. `fov` is in there too, but it is stored as a
//! slider position rather than degrees and the mapping has moved between
//! versions, so guessing it wrong would quietly produce a wrong tall
//! coefficient. That one stays a field you fill in.

use std::path::{Path, PathBuf};

/// One Minecraft install we found an options.txt for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    /// The instance directory's name, which is what the launcher shows.
    pub name: String,
    pub options: PathBuf,
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Directories that hold a launcher's instances, if they exist.
fn instance_roots(home: &Path) -> Vec<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));

    let mut roots = vec![
        data.join("PrismLauncher/instances"),
        data.join("multimc/instances"),
        data.join("MultiMC/instances"),
        // Flatpak keeps its own home, and a lot of people install it that way.
        home.join(".var/app/org.prismlauncher.PrismLauncher/data/PrismLauncher/instances"),
        home.join(".var/app/org.multimc.MultiMC/data/multimc/instances"),
        // Portable installs, where the launcher lives in its own folder.
        home.join("PrismLauncher/instances"),
        home.join("MultiMC/instances"),
    ];

    // XDG_DATA_HOME may well point at the same place as ~/.local/share.
    roots.dedup();
    roots
}

/// The options file inside one instance directory, whichever layout it uses.
fn options_in(instance: &Path) -> Option<PathBuf> {
    for inner in ["minecraft/options.txt", ".minecraft/options.txt"] {
        let path = instance.join(inner);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Every Minecraft install we can find an options.txt for, by name.
///
/// Deliberately shallow: only the two layouts a launcher actually uses, never
/// a recursive search. An instance also carries
/// `minecraft/speedrunigt/options.txt`, which is a different file with a
/// mouseSensitivity-shaped hole in it, and a find would happily return that.
pub fn instances() -> Vec<Instance> {
    let Some(home) = home() else { return Vec::new() };
    let mut found: Vec<Instance> = Vec::new();

    for root in instance_roots(&home) {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(options) = options_in(&path) else { continue };
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };

            found.push(Instance { name: name.to_string(), options });
        }
    }

    // A plain install, which is not an instance but is still an answer.
    let vanilla = home.join(".minecraft/options.txt");
    if vanilla.is_file() {
        found.push(Instance { name: ".minecraft".into(), options: vanilla });
    }

    // Two roots can be the same directory by different names.
    found.sort_by(|a, b| a.name.cmp(&b.name).then(a.options.cmp(&b.options)));
    found.dedup_by(|a, b| a.options == b.options);
    found
}

/// One `key:value` line out of an options.txt.
pub fn read_option(path: &Path, key: &str) -> Option<String> {
    let body = std::fs::read_to_string(path).ok()?;

    for line in body.lines() {
        let Some((name, value)) = line.split_once(':') else { continue };
        if name.trim() == key {
            return Some(value.trim().to_string());
        }
    }
    None
}

/// The mouse sensitivity slider, 0 to 1, as the calculator wants it.
pub fn read_sensitivity(path: &Path) -> Option<f64> {
    let raw = read_option(path, "mouseSensitivity")?;
    let value: f64 = raw.parse().ok()?;

    // Out of range means we read something that is not the slider.
    if !(0.0..=1.0).contains(&value) || value.is_nan() {
        return None;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("toolwall-mc-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_the_sensitivity_slider() {
        let dir = scratch("read");
        let path = dir.join("options.txt");
        std::fs::write(
            &path,
            "version:2586\nmouseSensitivity:0.02291165\nfov:0.625\nguiScale:2\n",
        )
        .unwrap();

        assert_eq!(read_sensitivity(&path), Some(0.02291165));
        assert_eq!(read_option(&path, "guiScale").as_deref(), Some("2"));
        assert_eq!(read_option(&path, "nothing"), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_value_that_is_not_the_slider_is_refused() {
        let dir = scratch("range");
        let path = dir.join("options.txt");

        // Better to ask than to feed a nonsense number into the calculator.
        std::fs::write(&path, "mouseSensitivity:7\n").unwrap();
        assert_eq!(read_sensitivity(&path), None);

        std::fs::write(&path, "mouseSensitivity:very fast\n").unwrap();
        assert_eq!(read_sensitivity(&path), None);

        std::fs::write(&path, "fov:0.5\n").unwrap();
        assert_eq!(read_sensitivity(&path), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_key_that_only_looks_similar_is_not_matched() {
        let dir = scratch("prefix");
        let path = dir.join("options.txt");
        std::fs::write(&path, "mouseSensitivityX:0.9\nmouseSensitivity:0.4\n").unwrap();

        assert_eq!(read_sensitivity(&path), Some(0.4));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn both_instance_layouts_are_found_and_speedrunigt_is_not() {
        let dir = scratch("layouts");

        let prism = dir.join("Ranked/minecraft");
        std::fs::create_dir_all(&prism).unwrap();
        std::fs::write(prism.join("options.txt"), "mouseSensitivity:0.3\n").unwrap();

        // The decoy. It has the same name and is not the same file.
        std::fs::create_dir_all(prism.join("speedrunigt")).unwrap();
        std::fs::write(prism.join("speedrunigt/options.txt"), "mouseSensitivity:0.9\n").unwrap();

        let multimc = dir.join("Practice/.minecraft");
        std::fs::create_dir_all(&multimc).unwrap();
        std::fs::write(multimc.join("options.txt"), "mouseSensitivity:0.5\n").unwrap();

        assert_eq!(options_in(&dir.join("Ranked")), Some(prism.join("options.txt")));
        assert_eq!(options_in(&dir.join("Practice")), Some(multimc.join("options.txt")));
        assert_eq!(options_in(&dir.join("Missing")), None);

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
