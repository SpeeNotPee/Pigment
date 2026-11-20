//profiles, lowk dont need to explain ts one prob
use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::{Config, ConfigError};
use crate::mods::{self, ComposeReport, ModSource};
use crate::paths::{PigmentPaths, SoberPaths};

/// a named configuration bundle.
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
    pub overlay: ComposeReport,
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
    #[error("{0:?} is not a valid profile name (letters, digits, space, - _ . only; max 64)")]
    InvalidName(String),
}

/// the sober keys pigment owns. snapshots only grab these + fflags, so a profile dont
/// freeze random future sober keys forever. matches the settings page 1:1 (theres a test)
pub const MANAGED_SETTINGS: &[&str] = &[
    "allow_gamepad_permission",
    "close_on_leave",
    "discord_rpc_enabled",
    "discord_rpc_show_join_button",
    "enable_gamemode",
    "enable_hidpi",
    "enable_mobile_home_screen",
    "graphics_optimization_mode",
    "server_location_indicator_enabled",
    "touch_mode",
    "use_console_experience",
    "use_libsecret",
    "use_opengl",
];

impl Profile {
    /// grab the current sober setup as a profile. managed keys + fflags only
    pub fn snapshot(name: impl Into<String>, config: &Config) -> Self {
        let mut p = Self::new(name);
        for key in MANAGED_SETTINGS {
            if let Some(v) = config.get(key) {
                p.settings.insert((*key).to_string(), v.clone());
            }
        }
        if let Some(flags) = config.fflags() {
            p.fflags = flags.clone();
        }
        p
    }
}

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

    fn profile_file(&self, name: &str) -> Result<PathBuf, ProfileError> {
        if !crate::util::is_valid_name(name) {
            return Err(ProfileError::InvalidName(name.to_string()));
        }
        Ok(self.paths.profiles_dir().join(format!("{name}.json")))
    }

    /// whether a profile by this name already exists (for the overwrite prompt)
    pub fn exists(&self, name: &str) -> bool {
        self.profile_file(name).map(|p| p.exists()).unwrap_or(false)
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
                if let Some(stem) = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .filter(|s| crate::util::is_valid_name(s))
                {
                    names.push(stem.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    /// Load a profile by name.
    pub fn load(&self, name: &str) -> Result<Profile, ProfileError> {
        let path = self.profile_file(name)?;
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
        let path = self.profile_file(&profile.name)?;
        let json =
            serde_json::to_vec_pretty(profile).map_err(|e| ProfileError::Parse(e.to_string()))?;
        crate::util::write_atomic(&path, &json).map_err(|source| ProfileError::Io { path, source })
    }

    /// Delete a profile. Deleting a missing profile is not an error.
    pub fn delete(&self, name: &str) -> Result<(), ProfileError> {
        let path = self.profile_file(name)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(ProfileError::Io { path, source }),
        }
    }

    /// The active profile name, if one is set and still exists.
    pub fn active(&self) -> Option<String> {
        let name = crate::state::State::load(&self.paths).active_profile?;
        // Only report it as active if the profile file still exists.
        self.exists(&name).then_some(name)
    }

    /// load, modify, save. used to rewrite state.json from scratch n nuke the saved handlers
    pub fn set_active(&self, name: Option<&str>) -> Result<(), ProfileError> {
        if let Some(n) = name {
            self.profile_file(n)?;
        }
        let mut state = crate::state::State::load(&self.paths);
        state.active_profile = name.map(str::to_string);
        state.save(&self.paths).map_err(|source| ProfileError::Io {
            path: self.paths.state_file(),
            source,
        })
    }

    /// Resolve a mod name to its source directory in the library, if it exists.
    /// names straight outta a profile json r untrusted, so `../../.ssh` = missing, not copied
    fn resolve_mod(&self, name: &str) -> Option<ModSource> {
        if !crate::util::is_valid_name(name) {
            return None;
        }
        let root = self.paths.mods_dir().join(name);
        root.is_dir().then(|| ModSource::new(name, root))
    }

    pub fn apply(
        &self,
        profile: &Profile,
        sober: &SoberPaths,
    ) -> Result<ApplyReport, ProfileError> {
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
        let overlay =
            mods::compose_overlay(sober.asset_overlay_dir(), &sources).map_err(|source| {
                ProfileError::Io {
                    path: sober.asset_overlay_dir(),
                    source,
                }
            })?;

        Ok(ApplyReport {
            overlay,
            missing_mods,
        })
    }
}

impl ProfileStore {
    /// apply whatever profile is active rn. Ok(None) = nothing active, launch w/ config as-is
    /// shared by pigment-launch and the gui join buttons so they cant drift apart
    pub fn apply_active(&self, sober: &SoberPaths) -> Result<Option<ApplyReport>, ProfileError> {
        let Some(name) = self.active() else {
            return Ok(None);
        };
        let profile = self.load(&name)?;
        self.apply(&profile, sober).map(Some)
    }

    /// THE way pages edit stuff. active profile = source of truth, so i edit it, save it,
    /// then apply it. Ok(None) means no active profile, caller writes config.json itself.
    /// b4 this the fflags/settings pages only wrote config.json n the next apply reverted em
    pub fn update_active(
        &self,
        sober: &SoberPaths,
        edit: impl FnOnce(&mut Profile),
    ) -> Result<Option<ApplyReport>, ProfileError> {
        let Some(name) = self.active() else {
            return Ok(None);
        };
        let mut profile = self.load(&name)?;
        edit(&mut profile);
        self.save(&profile)?;
        self.apply(&profile, sober).map(Some)
    }
}

/// what happened on a launch. profile trouble never blocks the launch, it just rides along
#[derive(Debug)]
pub struct LaunchOutcome {
    pub child: std::io::Result<std::process::Child>,
    /// set if the profile couldnt fully apply. show it, dont eat it
    pub profile_warning: Option<String>,
}

/// apply the active profile (best effort) then launch sober. every launch button goes thru
/// here: home, settings, join, n pigment-launch. used to be only some of em lol
pub fn launch_with_active_profile(sober: &crate::Sober, uri: Option<&str>) -> LaunchOutcome {
    let profile_warning = match ProfileStore::discover() {
        None => None,
        Some(store) => match store.apply_active(sober.paths()) {
            Ok(Some(rep)) if !rep.missing_mods.is_empty() => Some(format!(
                "active profile references missing mods: {}",
                rep.missing_mods.join(", ")
            )),
            Ok(_) => None,
            Err(e) => Some(format!("could not apply active profile: {e}")),
        },
    };
    LaunchOutcome {
        child: sober.launch(uri),
        profile_warning,
    }
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
            cfg.fflags()
                .and_then(|m| m.get("DFIntFoo"))
                .and_then(Value::as_i64),
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
    fn bad_profile_names_are_refused() {
        let (dir, store) = store();
        for bad in ["PvP/Ranked", "../x", "..", ".", ""] {
            assert!(
                matches!(
                    store.save(&Profile::new(bad)),
                    Err(ProfileError::InvalidName(_))
                ),
                "{bad}"
            );
            assert!(store.set_active(Some(bad)).is_err());
            assert!(store.delete(bad).is_err());
        }
        assert!(!dir.path().join("x.json").exists());
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn profile_mod_names_cant_escape_the_library() {
        let (dir, store) = store();
        let home = tempfile::tempdir().unwrap();
        let sober = fake_sober(home.path());
        //a juicy dir outside mods/ that the profile tries to drag into the overlay
        let secret = dir.path().join("secret");
        fs::create_dir_all(&secret).unwrap();
        fs::write(secret.join("id_ed25519"), b"nope").unwrap();

        let mut p = Profile::new("evil");
        p.mods = vec!["../secret".into(), "..".into()];
        let rep = store.apply(&p, &sober).unwrap();
        assert_eq!(rep.missing_mods.len(), 2);
        assert_eq!(rep.overlay.file_count(), 0);
        assert!(!sober.asset_overlay_dir().join("id_ed25519").exists());
    }

    #[test]
    fn flag_saved_thru_update_active_survives_a_mod_toggle() {
        //the B1 repro: enable mod -> add flag -> toggle mod -> flag must still be there
        let (_d, store) = store();
        let home = tempfile::tempdir().unwrap();
        let sober = fake_sober(home.path());
        store.save(&Profile::new("Default")).unwrap();
        store.set_active(Some("Default")).unwrap();

        store
            .update_active(&sober, |p| {
                p.fflags.insert("FFlagKeepMe".into(), Value::Bool(true));
            })
            .unwrap()
            .expect("profile is active");
        //toggle = edit mods + apply, same as the mods page does
        store
            .update_active(&sober, |p| p.mods.push("whatever".into()))
            .unwrap();
        //n a launch reapplies too
        store.apply_active(&sober).unwrap();

        let cfg = Config::load(sober.config_file()).unwrap();
        assert_eq!(
            cfg.fflags().unwrap().get("FFlagKeepMe"),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn update_active_without_profile_is_none() {
        let (_d, store) = store();
        let home = tempfile::tempdir().unwrap();
        let sober = fake_sober(home.path());
        assert!(store
            .update_active(&sober, |_| panic!("no profile"))
            .unwrap()
            .is_none());
    }

    #[test]
    fn snapshot_only_grabs_managed_keys() {
        let cfg =
            Config::parse(r#"{"use_opengl":true,"some_future_key":1,"fflags":{"A":1}}"#).unwrap();
        let p = Profile::snapshot("s", &cfg);
        assert_eq!(p.settings.len(), 1);
        assert!(p.settings.contains_key("use_opengl"));
        assert_eq!(p.fflags.len(), 1);
    }

    #[test]
    fn set_active_keeps_other_state() {
        let (_d, store) = store();
        let mut st = crate::state::State::load(store.paths());
        st.previous_handlers
            .insert("x-scheme-handler/roblox".into(), "other.desktop".into());
        st.save(store.paths()).unwrap();
        store.save(&Profile::new("a")).unwrap();
        store.set_active(Some("a")).unwrap();
        let st = crate::state::State::load(store.paths());
        assert_eq!(
            st.previous_handlers["x-scheme-handler/roblox"],
            "other.desktop"
        );
        assert_eq!(st.active_profile.as_deref(), Some("a"));
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
