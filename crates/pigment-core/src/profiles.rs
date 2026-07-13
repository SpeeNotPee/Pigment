//! Profiles: named bundles of Sober settings, FastFlags, and enabled mods.
//!
//! Sober has exactly one config and one overlay, so profiles are applied
//! *sequentially* — switching profile re-applies its state onto Sober. This is
//! the deliberate alternative to simultaneous multi-instance (which Sober refuses
//! and which invites bans); a user keeps main/alt/testing setups and swaps
//! between them.
//!
//! [`ProfileStore::apply`] is where the pieces meet: it writes the profile's
//! settings and flags into Sober's config through the safe [`crate::Config`]
//! writer, and composes the profile's mods into the overlay via
//! [`crate::mods`]. Applying is best-effort about mods (a missing mod is
//! reported, not fatal) but strict about the config write.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::{Config, ConfigError};
use crate::mods::{self, ComposeReport, ModSource};
use crate::paths::{PigmentPaths, SoberPaths};

/// A named configuration bundle.
///
/// `settings` are Sober config keys to force (a subset — unlisted keys keep their
/// current on-disk value). `fflags` fully defines the profile's FastFlag set.
/// `mods` names entries in the mod library, in overlay precedence order (later
/// wins).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub settings: Map<String, Value>,
    #[serde(default)]
    pub fflags: Map<String, Value>,
    #[serde(default)]
    pub mods: Vec<String>,
}

impl Profile {
    /// A new, empty profile with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            settings: Map::new(),
            fflags: Map::new(),
            mods: Vec::new(),
        }
    }
}

/// The outcome of applying a profile.
#[derive(Debug, Clone, Default)]
pub struct ApplyReport {
    /// Files written into the overlay, and which mod won each.
    pub overlay: ComposeReport,
    /// Names in `profile.mods` with no matching directory in the mod library.
    pub missing_mods: Vec<String>,
}

/// Errors from profile storage and application.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("profile i/o at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("profile is not valid JSON: {0}")]
    Parse(String),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("no profile named {0:?}")]
    NotFound(String),
}

/// Reads and writes profiles under [`PigmentPaths`], and applies them onto Sober.
#[derive(Debug, Clone)]
pub struct ProfileStore {
    paths: PigmentPaths,
}

impl ProfileStore {
    pub fn new(paths: PigmentPaths) -> Self {
        Self { paths }
    }

    /// Construct from the current environment.
    pub fn discover() -> Option<Self> {
        Some(Self::new(PigmentPaths::discover()?))
    }

    /// The paths this store uses.
    pub fn paths(&self) -> &PigmentPaths {
        &self.paths
    }

    fn profile_file(&self, name: &str) -> PathBuf {
        self.paths.profiles_dir().join(format!("{name}.json"))
    }

    /// Names of all stored profiles, sorted.
    pub fn list(&self) -> Result<Vec<String>, ProfileError> {
        let dir = self.paths.profiles_dir();
        let mut names = Vec::new();
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(names),
            Err(source) => return Err(ProfileError::Io { path: dir, source }),
        };
        for entry in entries {
            let entry = entry.map_err(|source| ProfileError::Io {
                path: dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Load a profile by name.
    pub fn load(&self, name: &str) -> Result<Profile, ProfileError> {
        let path = self.profile_file(name);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(ProfileError::NotFound(name.to_string()))
            }
            Err(source) => return Err(ProfileError::Io { path, source }),
        };
        serde_json::from_str(&text).map_err(|e| ProfileError::Parse(e.to_string()))
    }

    /// Save a profile, creating or overwriting its file atomically.
    pub fn save(&self, profile: &Profile) -> Result<(), ProfileError> {
        let path = self.profile_file(&profile.name);
        let json = serde_json::to_vec_pretty(profile)
            .map_err(|e| ProfileError::Parse(e.to_string()))?;
        crate::util::write_atomic(&path, &json).map_err(|source| ProfileError::Io { path, source })
    }

    /// Delete a profile. Deleting a missing profile is not an error.
    pub fn delete(&self, name: &str) -> Result<(), ProfileError> {
        let path = self.profile_file(name);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(ProfileError::Io { path, source }),
        }
    }

    /// The active profile name, if one is set and still exists.
    pub fn active(&self) -> Option<String> {
        let text = fs::read_to_string(self.paths.state_file()).ok()?;
        let state: State = serde_json::from_str(&text).ok()?;
        let name = state.active_profile?;
        // Only report it as active if the profile file still exists.
        self.profile_file(&name).exists().then_some(name)
    }

    /// Set (or clear, with `None`) the active profile.
    pub fn set_active(&self, name: Option<&str>) -> Result<(), ProfileError> {
        let path = self.paths.state_file();
        let state = State {
            active_profile: name.map(str::to_string),
        };
        let json = serde_json::to_vec_pretty(&state)
            .map_err(|e| ProfileError::Parse(e.to_string()))?;
        crate::util::write_atomic(&path, &json).map_err(|source| ProfileError::Io { path, source })
    }

    /// Resolve a mod name to its source directory in the library, if it exists.
    fn resolve_mod(&self, name: &str) -> Option<ModSource> {
        let root = self.paths.mods_dir().join(name);
        root.is_dir().then(|| ModSource::new(name, root))
    }

    /// Apply a profile onto Sober: write its settings and flags into the config
    /// (safely, preserving unknown keys), and compose its mods into the overlay.
    ///
    /// The Sober config must already exist (Sober creates it on first launch);
    /// otherwise this returns a [`ConfigError`]. Mods that don't resolve are
    /// collected in [`ApplyReport::missing_mods`] rather than aborting.
    pub fn apply(
        &self,
        profile: &Profile,
        sober: &SoberPaths,
    ) -> Result<ApplyReport, ProfileError> {
        // 1. Config: load fresh (preserves keys we don't manage), overlay the
        //    profile's settings, replace the fflag set, save atomically.
        let mut config = Config::load(sober.config_file())?;
        for (k, v) in &profile.settings {
            config.set(k.clone(), v.clone());
        }
        config.set_fflags(profile.fflags.clone());
        config.save(sober.config_file())?;

        // 2. Mods: resolve names to sources, compose in order.
        let mut sources = Vec::new();
        let mut missing_mods = Vec::new();
        for name in &profile.mods {
            match self.resolve_mod(name) {
                Some(src) => sources.push(src),
                None => missing_mods.push(name.clone()),
            }
        }
        let overlay = mods::compose_overlay(sober.asset_overlay_dir(), &sources)
            .map_err(|source| ProfileError::Io {
                path: sober.asset_overlay_dir(),
                source,
            })?;

        Ok(ApplyReport {
            overlay,
            missing_mods,
        })
    }
}

/// The on-disk cross-cutting state file (`state.json`).
#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    #[serde(default)]
    active_profile: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn store() -> (tempfile::TempDir, ProfileStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(PigmentPaths::with_config_dir(dir.path()));
        (dir, store)
    }

    /// Write a minimal valid Sober config into a temp Sober tree; return its
    /// SoberPaths.
    fn fake_sober(home: &std::path::Path) -> SoberPaths {
        let sober = SoberPaths::from_home(home);
        let cfg = sober.config_file();
        fs::create_dir_all(cfg.parent().unwrap()).unwrap();
        let mut f = fs::File::create(&cfg).unwrap();
        f.write_all(b"{\n    \"use_opengl\": false,\n    \"fflags\": {}\n}\n")
            .unwrap();
        sober
    }

    #[test]
    fn save_load_list_delete_roundtrip() {
        let (_d, store) = store();
        let mut p = Profile::new("main");
        p.settings.insert("use_opengl".into(), Value::Bool(true));
        p.fflags.insert("DFIntTest".into(), Value::from(3));
        p.mods.push("darkcursor".into());

        store.save(&p).unwrap();
        assert_eq!(store.list().unwrap(), vec!["main".to_string()]);
        assert_eq!(store.load("main").unwrap(), p);

        store.delete("main").unwrap();
        assert!(store.list().unwrap().is_empty());
        assert!(matches!(store.load("main"), Err(ProfileError::NotFound(_))));
    }

    #[test]
    fn active_pointer_tracks_and_validates() {
        let (_d, store) = store();
        assert_eq!(store.active(), None);
        store.save(&Profile::new("alt")).unwrap();
        store.set_active(Some("alt")).unwrap();
        assert_eq!(store.active().as_deref(), Some("alt"));

        // Pointing at a deleted profile reads as no active profile.
        store.delete("alt").unwrap();
        assert_eq!(store.active(), None);
    }

    #[test]
    fn apply_writes_settings_and_flags_preserving_unknown_keys() {
        let (_d, store) = store();
        let home = tempfile::tempdir().unwrap();
        let sober = fake_sober(home.path());

        let mut p = Profile::new("main");
        p.settings.insert("use_opengl".into(), Value::Bool(true));
        p.fflags.insert("DFIntFoo".into(), Value::from(7));
        store.apply(&p, &sober).unwrap();

        let cfg = Config::load(sober.config_file()).unwrap();
        assert_eq!(cfg.get_bool("use_opengl"), Some(true));
        assert_eq!(
            cfg.fflags().and_then(|m| m.get("DFIntFoo")).and_then(Value::as_i64),
            Some(7)
        );
    }

    #[test]
    fn apply_composes_mods_and_reports_missing() {
        let (_d, store) = store();
        let home = tempfile::tempdir().unwrap();
        let sober = fake_sober(home.path());

        // Install one real mod into the library; reference a second that's absent.
        let mod_root = store.paths().mods_dir().join("darkcursor");
        let target = mod_root.join("content/textures/Cursors/KeyboardMouse/ArrowCursor.png");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"dark").unwrap();

        let mut p = Profile::new("main");
        p.mods = vec!["darkcursor".into(), "ghostmod".into()];
        let report = store.apply(&p, &sober).unwrap();

        assert_eq!(report.missing_mods, vec!["ghostmod".to_string()]);
        assert_eq!(report.overlay.file_count(), 1);
        let overlaid = sober
            .asset_overlay_dir()
            .join("content/textures/Cursors/KeyboardMouse/ArrowCursor.png");
        assert_eq!(fs::read(overlaid).unwrap(), b"dark".to_vec());
    }

    #[test]
    fn apply_without_sober_config_errors() {
        let (_d, store) = store();
        let home = tempfile::tempdir().unwrap();
        let sober = SoberPaths::from_home(home.path()); // no config written
        let err = store.apply(&Profile::new("x"), &sober).unwrap_err();
        assert!(matches!(err, ProfileError::Config(_)));
    }
}
