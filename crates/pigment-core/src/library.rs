//! The mod library: the collection of installed mods under Pigment's config
//! directory (`~/.config/pigment/mods/<name>/`).
//!
//! This layer only manages the *collection* — installing a mod's file tree,
//! listing what's installed, and removing one. Which mods are *enabled* (and
//! thus composed into Sober's overlay) is a property of the active profile
//! (`Profile::mods`); enabling happens through [`crate::ProfileStore`], and the
//! actual overlay composition and conflict detection live in [`crate::mods`].
//!
//! A mod is a directory tree mirroring the Roblox APK's `assets/` layout — e.g.
//! `content/textures/Cursors/KeyboardMouse/ArrowCursor.png` — so it can be
//! validated against [`crate::ApkAssetTree`].

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::mods::ModSource;
use crate::paths::PigmentPaths;

/// Manages installed mods under [`PigmentPaths::mods_dir`].
#[derive(Debug, Clone)]
pub struct ModLibrary {
    paths: PigmentPaths,
}

impl ModLibrary {
    pub fn new(paths: PigmentPaths) -> Self {
        Self { paths }
    }

    /// Construct from the current environment.
    pub fn discover() -> Option<Self> {
        Some(Self::new(PigmentPaths::discover()?))
    }

    /// The directory a named mod lives in (whether or not it exists).
    pub fn mod_dir(&self, name: &str) -> PathBuf {
        self.paths.mods_dir().join(name)
    }

    /// All installed mods, sorted by name.
    pub fn installed(&self) -> io::Result<Vec<ModSource>> {
        let dir = self.paths.mods_dir();
        let mut mods = Vec::new();
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(mods),
            Err(e) => return Err(e),
        };
        for entry in entries {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    mods.push(ModSource::new(name, entry.path()));
                }
            }
        }
        mods.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(mods)
    }

    /// Look up one installed mod by name.
    pub fn get(&self, name: &str) -> Option<ModSource> {
        let dir = self.mod_dir(name);
        dir.is_dir().then(|| ModSource::new(name, dir))
    }

    /// Whether a mod by this name is installed.
    pub fn contains(&self, name: &str) -> bool {
        self.mod_dir(name).is_dir()
    }

    /// Install a mod by copying a source directory tree into the library under
    /// `name`, replacing any existing mod of the same name. Returns the sanitized
    /// name actually used.
    pub fn install_from_dir(&self, name: &str, src: &Path) -> io::Result<String> {
        let name = sanitize_name(name);
        if name.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "mod name is empty after sanitizing",
            ));
        }
        let dest = self.mod_dir(&name);
        if dest.exists() {
            fs::remove_dir_all(&dest)?;
        }
        crate::util::copy_dir_all(src, &dest)?;
        Ok(name)
    }

    /// Remove an installed mod. Removing a missing mod is not an error.
    pub fn remove(&self, name: &str) -> io::Result<()> {
        let dir = self.mod_dir(name);
        match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// Reduce a proposed mod name to a safe single path component: keep it to its
/// base name and strip anything that isn't alphanumeric, dash, underscore, dot,
/// or space. Prevents path traversal and nested-directory surprises.
fn sanitize_name(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    base.chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn lib() -> (tempfile::TempDir, ModLibrary) {
        let dir = tempfile::tempdir().unwrap();
        let lib = ModLibrary::new(PigmentPaths::with_config_dir(dir.path()));
        (dir, lib)
    }

    /// Build a source mod tree in a temp dir; return its path.
    fn make_mod(files: &[(&str, &[u8])]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (rel, bytes) in files {
            let p = dir.path().join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::File::create(p).unwrap().write_all(bytes).unwrap();
        }
        dir
    }

    #[test]
    fn install_list_get_remove() {
        let (_d, lib) = lib();
        assert!(lib.installed().unwrap().is_empty());

        let src = make_mod(&[
            ("content/textures/Cursors/KeyboardMouse/ArrowCursor.png", b"x"),
            ("content/sounds/ouch.ogg", b"y"),
        ]);
        let name = lib.install_from_dir("darkcursor", src.path()).unwrap();
        assert_eq!(name, "darkcursor");

        let installed = lib.installed().unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].name, "darkcursor");
        assert!(lib.contains("darkcursor"));
        assert_eq!(lib.get("darkcursor").unwrap().files().unwrap().len(), 2);

        lib.remove("darkcursor").unwrap();
        assert!(!lib.contains("darkcursor"));
        assert!(lib.remove("darkcursor").is_ok()); // idempotent
    }

    #[test]
    fn reinstall_replaces_previous_tree() {
        let (_d, lib) = lib();
        let a = make_mod(&[("content/a.png", b"1"), ("content/b.png", b"2")]);
        lib.install_from_dir("m", a.path()).unwrap();
        let b = make_mod(&[("content/a.png", b"1")]); // fewer files
        lib.install_from_dir("m", b.path()).unwrap();
        assert_eq!(lib.get("m").unwrap().files().unwrap().len(), 1);
    }

    #[test]
    fn name_is_sanitized_against_traversal() {
        assert_eq!(sanitize_name("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_name("my mod!!"), "my mod__");
        assert_eq!(sanitize_name("ok-name_1.2"), "ok-name_1.2");
    }
}
