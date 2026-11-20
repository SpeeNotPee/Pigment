//mod library
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::mods::ModSource;
use crate::paths::PigmentPaths;
use crate::util::is_valid_name;

/// Manages installed mods
#[derive(Debug, Clone)]
pub struct ModLibrary {
    paths: PigmentPaths,
}

/// hard caps on what "install from folder" will copy. a real mod is a handful of
/// pngs/oggs, anything past this is prob someone picking the wrong folder
pub const MAX_MOD_FILES: usize = 5_000;
pub const MAX_MOD_BYTES: u64 = 1024 * 1024 * 1024;

fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

impl ModLibrary {
    pub fn new(paths: PigmentPaths) -> Self {
        Self { paths }
    }

    /// Construct from the current environment.
    pub fn discover() -> Option<Self> {
        Some(Self::new(PigmentPaths::discover()?))
    }

    /// dir for a mod, or None if the name is sketchy. callers never get to join raw input
    pub fn mod_dir(&self, name: &str) -> Option<PathBuf> {
        is_valid_name(name).then(|| self.paths.mods_dir().join(name))
    }

    /// all installed mods, sorted by name. staging dirs + junk names get skipped
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
                if let Some(name) = entry.file_name().to_str().filter(|n| is_valid_name(n)) {
                    mods.push(ModSource::new(name, entry.path()));
                }
            }
        }
        mods.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(mods)
    }

    /// installed mod search
    pub fn get(&self, name: &str) -> Option<ModSource> {
        let dir = self.mod_dir(name)?;
        dir.is_dir().then(|| ModSource::new(name, dir))
    }

    /// whether a mod by this name is installed.
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// whether a folder is safe to install from. i refuse anything that IS the mods dir,
    /// contains it (picking ~ or / would copy the library into itself forever), or sits inside it
    pub fn check_source(&self, src: &Path) -> io::Result<()> {
        let src = src.canonicalize()?;
        if !src.is_dir() {
            return Err(invalid("that is not a folder"));
        }
        let mods = self.paths.mods_dir();
        //mods dir might not exist yet, so canonicalize its closest existing parent
        let mods = mods.canonicalize().unwrap_or(mods);
        if mods.starts_with(&src) {
            return Err(invalid(
                "that folder contains Pigment's own mod library - pick the mod's folder itself",
            ));
        }
        if src.starts_with(&mods) {
            return Err(invalid(
                "that folder is already inside Pigment's mod library",
            ));
        }
        Ok(())
    }

    /// copy a mod folder in. guarded by [`Self::check_source`] + the size caps, and staged so
    /// a reinstall only replaces the old copy once the new one fully landed
    pub fn install_from_dir(&self, name: &str, src: &Path) -> io::Result<String> {
        self.check_source(src)?;
        let (files, _bytes) = scan_dir(src)?;
        if files == 0 {
            return Err(invalid("that folder has no files"));
        }
        let name = sanitize_name(name);
        let dest = self
            .mod_dir(&name)
            .ok_or_else(|| invalid(format!("{name:?} is not a usable mod name")))?;
        let staging = self.staging_dir(&name)?;
        if let Err(e) = crate::util::copy_dir_all(src, &staging) {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
        crate::util::swap_dir_into_place(&staging, &dest)?;
        Ok(name)
    }

    /// install a mod straight from in-memory bytes. presets use this so they dont
    /// have to stage a temp folder first. rel paths are apk-relative like on disk.
    /// all-or-nothing for real now: every path gets checked b4 i touch the disk
    pub fn install_from_files(&self, name: &str, files: &[(&str, &[u8])]) -> io::Result<String> {
        if !is_valid_name(name) {
            return Err(invalid(format!("{name:?} is not a usable mod name")));
        }
        if files.is_empty() {
            return Err(invalid("mod has no files"));
        }
        for (rel, _) in files {
            if !is_safe_rel_path(rel) {
                return Err(invalid(format!("refusing bad mod path {rel:?}")));
            }
        }
        let dest = self.mod_dir(name).expect("validated above");
        let staging = self.staging_dir(name)?;
        let write_all = || -> io::Result<()> {
            for (rel, bytes) in files {
                let target = staging.join(rel);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&target, bytes)?;
            }
            Ok(())
        };
        if let Err(e) = write_all() {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
        crate::util::swap_dir_into_place(&staging, &dest)?;
        Ok(name.to_string())
    }

    pub fn remove(&self, name: &str) -> io::Result<()> {
        let dir = self
            .mod_dir(name)
            .ok_or_else(|| invalid(format!("{name:?} is not a usable mod name")))?;
        match fs::remove_dir_all(&dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// fresh empty `.staging-<name>` next to the real mods. dot prefix = installed() ignores it
    fn staging_dir(&self, name: &str) -> io::Result<PathBuf> {
        let staging = self.paths.mods_dir().join(format!(".staging-{name}"));
        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir_all(&staging)?;
        Ok(staging)
    }
}

/// count files + bytes, bailing the second it blows a cap so picking / dont take 10 min
pub fn scan_dir(src: &Path) -> io::Result<(usize, u64)> {
    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file() {
                files += 1;
                bytes += entry.metadata()?.len();
                if files > MAX_MOD_FILES || bytes > MAX_MOD_BYTES {
                    return Err(invalid(format!(
                        "that folder is too big for a mod (over {MAX_MOD_FILES} files or {} MiB)",
                        MAX_MOD_BYTES / 1024 / 1024
                    )));
                }
            }
        }
    }
    Ok((files, bytes))
}

/// relative, no `..`, no root, no empty bits. the gate for anything landing in a mod dir
pub(crate) fn is_safe_rel_path(rel: &str) -> bool {
    let p = Path::new(rel);
    !rel.is_empty() && !p.is_absolute() && p.components().all(|c| matches!(c, Component::Normal(_)))
}

/// turn a user-picked folder name into something valid, or empty if theres nothing left
fn sanitize_name(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '_'
            }
        })
        .collect();
    //leading dots would make it hidden (n `..` is the whole S1 bug), so strip em
    let cleaned = cleaned.trim().trim_start_matches('.').trim();
    cleaned
        .chars()
        .take(64)
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
            (
                "content/textures/Cursors/KeyboardMouse/ArrowCursor.png",
                b"x",
            ),
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
        assert_eq!(sanitize_name(".."), "");
        assert_eq!(sanitize_name("."), "");
        assert_eq!(sanitize_name(".hidden"), "hidden");
    }

    #[test]
    fn dot_names_never_touch_the_disk() {
        let (dir, lib) = lib();
        //canary mod + a profile file that S1 used to nuke
        let src = make_mod(&[("content/a.png", b"1")]);
        lib.install_from_dir("keep", src.path()).unwrap();
        fs::write(dir.path().join("state.json"), b"{}").unwrap();

        for bad in [".", "..", " .. ", "a/.."] {
            assert!(lib
                .install_from_files(bad, &[("content/x.png", b"x".as_slice())])
                .is_err());
            assert!(lib.remove(bad).is_err());
            assert!(!lib.contains(bad));
            assert!(lib.mod_dir(bad).is_none());
        }
        assert!(lib.contains("keep"), "mods got wiped");
        assert!(
            dir.path().join("state.json").exists(),
            "pigment config got wiped"
        );
    }

    #[test]
    fn refuses_sources_that_contain_or_live_in_the_library() {
        let (dir, lib) = lib();
        let src = make_mod(&[("content/a.png", b"1")]);
        lib.install_from_dir("m", src.path()).unwrap();
        //the config dir itself (stand-in for picking ~)
        assert!(lib.install_from_dir("home", dir.path()).is_err());
        //a mod already in the library
        let inside = lib.mod_dir("m").unwrap();
        assert!(lib.install_from_dir("again", &inside).is_err());
        assert!(lib.contains("m"), "source got deleted b4 copy");
    }

    #[test]
    fn failed_reinstall_keeps_the_old_copy() {
        let (_d, lib) = lib();
        let src = make_mod(&[("content/a.png", b"1")]);
        lib.install_from_dir("m", src.path()).unwrap();
        let err = lib.install_from_files(
            "m",
            &[
                ("content/ok.png", b"x".as_slice()),
                ("../evil", b"x".as_slice()),
            ],
        );
        assert!(err.is_err());
        assert_eq!(
            lib.get("m").unwrap().files().unwrap(),
            vec!["content/a.png".to_string()]
        );
    }

    #[test]
    fn staging_dirs_are_hidden_from_the_list() {
        let (dir, lib) = lib();
        fs::create_dir_all(dir.path().join("mods/.staging-x")).unwrap();
        assert!(lib.installed().unwrap().is_empty());
    }
}
