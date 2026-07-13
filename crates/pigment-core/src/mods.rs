//! Mod management via Sober's `asset_overlay` directory.
//!
//! Sober exposes a *sanctioned* overlay: any file placed under
//! `data/sober/asset_overlay/`, mirroring the layout inside the Roblox APK's
//! `assets/` directory, replaces the corresponding game asset on next launch. No
//! binary patching, no anti-cheat contact.
//!
//! Two facts shape this module:
//!
//! * The **authoritative** list of replaceable assets is the APK itself
//!   (`base.apk`, a ZIP). [`ApkAssetTree`] reads it so mods can be validated
//!   against real paths — a mod file at a path Roblox doesn't ship simply won't
//!   take effect, and we want to warn about that.
//! * Because Sober runs the *Android* client, asset paths differ from the Windows
//!   client's. Windows Bloxstrap mods therefore do **not** drop in unchanged;
//!   validation against the APK tree is how we catch that.
//!
//! Pigment owns the overlay directory: [`compose_overlay`] rebuilds it from the
//! ordered set of enabled mods, so the overlay is always exactly the mods and
//! nothing stale.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Prefix inside the APK under which game assets live.
const APK_ASSETS_PREFIX: &str = "assets/";

/// The authoritative set of asset paths Roblox ships, read from `base.apk`.
///
/// Paths are stored relative to the APK's `assets/` directory (and thus relative
/// to the overlay root), using `/` separators — e.g.
/// `content/textures/Cursors/KeyboardMouse/ArrowCursor.png`.
#[derive(Debug, Clone, Default)]
pub struct ApkAssetTree {
    files: BTreeSet<String>,
}

impl ApkAssetTree {
    /// Read and index the asset entries of an APK (ZIP) file.
    pub fn read(apk_path: impl AsRef<Path>) -> io::Result<Self> {
        let file = fs::File::open(apk_path.as_ref())?;
        let mut zip = zip::ZipArchive::new(file)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let mut files = BTreeSet::new();
        for i in 0..zip.len() {
            let entry = zip
                .by_index(i)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let name = entry.name();
            if entry.is_dir() {
                continue;
            }
            if let Some(rel) = name.strip_prefix(APK_ASSETS_PREFIX) {
                if !rel.is_empty() {
                    files.insert(rel.to_string());
                }
            }
        }
        Ok(Self { files })
    }

    /// Number of indexed asset files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the tree indexed no assets.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Whether Roblox ships an asset at this overlay-relative path.
    pub fn contains(&self, rel_path: &str) -> bool {
        self.files.contains(rel_path)
    }

    /// Iterate all asset paths (sorted).
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(String::as_str)
    }
}

/// A mod: a named directory tree of replacement files.
///
/// Every file beneath `root` maps to the same relative path within the overlay
/// (and thus within the APK's `assets/`). For example, a file at
/// `<root>/content/textures/Cursors/KeyboardMouse/ArrowCursor.png` replaces that
/// exact game asset.
#[derive(Debug, Clone)]
pub struct ModSource {
    pub name: String,
    pub root: PathBuf,
}

impl ModSource {
    pub fn new(name: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            root: root.into(),
        }
    }

    /// The overlay-relative paths this mod provides, sorted, using `/`
    /// separators. Directories are not included, only files.
    pub fn files(&self) -> io::Result<Vec<String>> {
        let mut out = Vec::new();
        collect_files(&self.root, &self.root, &mut out)?;
        out.sort();
        Ok(out)
    }

    /// Paths in this mod that Roblox does **not** ship (per the APK tree) and so
    /// likely won't take effect — usually a wrong path or a Windows-client mod.
    pub fn unknown_paths(&self, tree: &ApkAssetTree) -> io::Result<Vec<String>> {
        Ok(self
            .files()?
            .into_iter()
            .filter(|p| !tree.contains(p))
            .collect())
    }
}

/// A file provided by more than one enabled mod.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    /// The overlay-relative path in contention.
    pub path: String,
    /// Names of all mods providing it, in enable order (last one wins).
    pub mods: Vec<String>,
}

/// Detect files claimed by more than one mod, in the given precedence order.
///
/// Order matters: in [`compose_overlay`] later mods overwrite earlier ones, so
/// the last entry in a conflict's `mods` list is the effective winner.
pub fn detect_conflicts(mods: &[ModSource]) -> io::Result<Vec<Conflict>> {
    let mut providers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for m in mods {
        for f in m.files()? {
            providers.entry(f).or_default().push(m.name.clone());
        }
    }
    Ok(providers
        .into_iter()
        .filter(|(_, ms)| ms.len() > 1)
        .map(|(path, mods)| Conflict { path, mods })
        .collect())
}

/// The result of composing the overlay.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComposeReport {
    /// Effective winner for each written path: `path -> mod name`.
    pub winners: BTreeMap<String, String>,
}

impl ComposeReport {
    /// Number of files written into the overlay.
    pub fn file_count(&self) -> usize {
        self.winners.len()
    }
}

/// Rebuild `overlay_dir` from the ordered set of enabled mods.
///
/// The overlay is cleared first, then each mod's files are copied in order, so
/// later mods win conflicts and no file from a previously-enabled mod lingers.
/// Pigment treats the overlay as wholly owned by this function.
pub fn compose_overlay(
    overlay_dir: impl AsRef<Path>,
    ordered_mods: &[ModSource],
) -> io::Result<ComposeReport> {
    let overlay_dir = overlay_dir.as_ref();

    // Clear prior contents (the overlay is derived state), then recreate.
    if overlay_dir.exists() {
        fs::remove_dir_all(overlay_dir)?;
    }
    fs::create_dir_all(overlay_dir)?;

    let mut winners = BTreeMap::new();
    for m in ordered_mods {
        for rel in m.files()? {
            let dest = overlay_dir.join(&rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(m.root.join(&rel), &dest)?;
            winners.insert(rel, m.name.clone());
        }
    }
    Ok(ComposeReport { winners })
}

/// Remove all overlay contents, reverting to stock assets on next launch.
pub fn clear_overlay(overlay_dir: impl AsRef<Path>) -> io::Result<()> {
    let overlay_dir = overlay_dir.as_ref();
    if overlay_dir.exists() {
        fs::remove_dir_all(overlay_dir)?;
    }
    fs::create_dir_all(overlay_dir)?;
    Ok(())
}

/// Recursively collect files under `dir`, as paths relative to `base` with `/`
/// separators.
fn collect_files(base: &Path, dir: &Path, out: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            collect_files(base, &path, out)?;
        } else if ft.is_file() {
            if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel_to_slash(rel));
            }
        }
    }
    Ok(())
}

/// Render a relative path with `/` separators regardless of platform.
fn rel_to_slash(rel: &Path) -> String {
    rel.components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Write `contents` to `dir/rel`, creating parents.
    fn write_file(dir: &Path, rel: &str, contents: &[u8]) {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::File::create(p).unwrap().write_all(contents).unwrap();
    }

    /// Build a minimal APK-shaped ZIP with the given asset paths (relative to
    /// `assets/`), plus a non-asset entry that must be ignored.
    fn make_apk(path: &Path, asset_paths: &[&str]) {
        let file = fs::File::create(path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        // A non-asset entry (e.g. classes.dex) that must not appear in the tree.
        zip.start_file("classes.dex", opts).unwrap();
        zip.write_all(b"not an asset").unwrap();
        for p in asset_paths {
            zip.start_file(format!("assets/{p}"), opts).unwrap();
            zip.write_all(b"asset bytes").unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn apk_tree_indexes_only_assets_relative_to_assets_dir() {
        let dir = tempfile::tempdir().unwrap();
        let apk = dir.path().join("base.apk");
        make_apk(
            &apk,
            &[
                "content/textures/Cursors/KeyboardMouse/ArrowCursor.png",
                "content/sounds/action_footsteps_plastic.mp3",
            ],
        );

        let tree = ApkAssetTree::read(&apk).unwrap();
        assert_eq!(tree.len(), 2);
        assert!(tree.contains("content/textures/Cursors/KeyboardMouse/ArrowCursor.png"));
        assert!(!tree.contains("classes.dex"));
        assert!(!tree.contains("assets/content/sounds/action_footsteps_plastic.mp3"));
    }

    #[test]
    fn mod_lists_its_files_relative_to_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("darkcursor");
        write_file(&root, "content/textures/Cursors/KeyboardMouse/ArrowCursor.png", b"x");
        write_file(&root, "content/sounds/ouch.ogg", b"y");

        let m = ModSource::new("darkcursor", &root);
        let files = m.files().unwrap();
        assert_eq!(
            files,
            vec![
                "content/sounds/ouch.ogg".to_string(),
                "content/textures/Cursors/KeyboardMouse/ArrowCursor.png".to_string(),
            ]
        );
    }

    #[test]
    fn unknown_paths_flags_files_absent_from_apk() {
        let dir = tempfile::tempdir().unwrap();
        let apk = dir.path().join("base.apk");
        make_apk(&apk, &["content/textures/real.png"]);
        let tree = ApkAssetTree::read(&apk).unwrap();

        let root = dir.path().join("mod");
        write_file(&root, "content/textures/real.png", b"a"); // valid target
        write_file(&root, "content/textures/typo.png", b"b"); // not in APK

        let m = ModSource::new("mod", &root);
        assert_eq!(
            m.unknown_paths(&tree).unwrap(),
            vec!["content/textures/typo.png".to_string()]
        );
    }

    #[test]
    fn conflicts_report_all_providers_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        write_file(&a, "content/x.png", b"a");
        write_file(&b, "content/x.png", b"b");
        write_file(&b, "content/y.png", b"b");

        let mods = vec![ModSource::new("a", &a), ModSource::new("b", &b)];
        let conflicts = detect_conflicts(&mods).unwrap();
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].path, "content/x.png");
        assert_eq!(conflicts[0].mods, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn compose_overlay_lets_later_mods_win_and_writes_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        write_file(&a, "content/x.png", b"from-a");
        write_file(&b, "content/x.png", b"from-b");
        write_file(&a, "content/only_a.png", b"a-only");
        let overlay = dir.path().join("asset_overlay");

        let mods = vec![ModSource::new("a", &a), ModSource::new("b", &b)];
        let report = compose_overlay(&overlay, &mods).unwrap();

        assert_eq!(report.file_count(), 2);
        assert_eq!(report.winners["content/x.png"], "b"); // later mod wins
        assert_eq!(report.winners["content/only_a.png"], "a");
        // Winning bytes actually landed on disk.
        assert_eq!(
            fs::read(overlay.join("content/x.png")).unwrap(),
            b"from-b".to_vec()
        );
    }

    #[test]
    fn recompose_removes_files_from_now_disabled_mods() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        write_file(&a, "content/x.png", b"a");
        let overlay = dir.path().join("asset_overlay");

        // Enable mod A, then recompose with no mods: overlay must be empty.
        compose_overlay(&overlay, &[ModSource::new("a", &a)]).unwrap();
        assert!(overlay.join("content/x.png").exists());

        let report = compose_overlay(&overlay, &[]).unwrap();
        assert_eq!(report.file_count(), 0);
        assert!(!overlay.join("content/x.png").exists(), "stale file lingered");
    }

    #[test]
    fn clear_overlay_empties_it() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        write_file(&a, "content/x.png", b"a");
        let overlay = dir.path().join("asset_overlay");
        compose_overlay(&overlay, &[ModSource::new("a", &a)]).unwrap();

        clear_overlay(&overlay).unwrap();
        assert!(overlay.exists());
        assert_eq!(fs::read_dir(&overlay).unwrap().count(), 0);
    }
}
