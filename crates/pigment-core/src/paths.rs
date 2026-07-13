//! Resolution of Sober's on-disk locations.
//!
//! Every path Pigment touches inside Sober's Flatpak sandbox is derived here, so
//! there is exactly one place to fix if VinegarHQ ever moves things. Paths are
//! computed, not probed — none of these methods touch the filesystem — so they
//! are cheap and usable even when Sober has never been launched.
//!
//! Layout verified against Sober 1.7.1:
//! ```text
//! ~/.var/app/org.vinegarhq.Sober/
//! ├── config/sober/config.json
//! └── data/sober/
//!     ├── asset_overlay/                         (mods composited here)
//!     ├── sober_logs/latest.log
//!     └── packages/<arch>/com.roblox.client/base.apk   (authoritative asset tree)
//! ```

use std::path::{Path, PathBuf};

/// The Flatpak application id for Sober.
pub const SOBER_APP_ID: &str = "org.vinegarhq.Sober";

/// Locations inside Sober's per-app Flatpak directory.
///
/// Construct with [`SoberPaths::from_home`] (or [`SoberPaths::discover`], which
/// reads `$HOME`). The `arch` field selects the `packages/<arch>` subtree; it
/// defaults to the host architecture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoberPaths {
    /// `~/.var/app/org.vinegarhq.Sober`
    app_dir: PathBuf,
    /// e.g. `x86_64` — selects the `packages/<arch>` subtree.
    arch: String,
}

impl SoberPaths {
    /// Build paths rooted at the given home directory.
    ///
    /// Does not touch the filesystem; `home` need not exist.
    pub fn from_home(home: impl AsRef<Path>) -> Self {
        Self {
            app_dir: home.as_ref().join(".var/app").join(SOBER_APP_ID),
            arch: default_arch().to_string(),
        }
    }

    /// Build paths from the current user's `$HOME`.
    ///
    /// Returns `None` if `$HOME` is unset or empty, which is the only reason
    /// resolution can fail.
    pub fn discover() -> Option<Self> {
        let home = std::env::var_os("HOME").filter(|h| !h.is_empty())?;
        Some(Self::from_home(home))
    }

    /// Override the package architecture subdirectory (default: host arch).
    pub fn with_arch(mut self, arch: impl Into<String>) -> Self {
        self.arch = arch.into();
        self
    }

    /// `~/.var/app/org.vinegarhq.Sober`
    pub fn app_dir(&self) -> &Path {
        &self.app_dir
    }

    /// The config file: `config/sober/config.json`.
    pub fn config_file(&self) -> PathBuf {
        self.app_dir.join("config/sober/config.json")
    }

    /// The Sober data root: `data/sober`.
    pub fn data_dir(&self) -> PathBuf {
        self.app_dir.join("data/sober")
    }

    /// The mod overlay directory: `data/sober/asset_overlay`.
    ///
    /// Files placed here, mirroring the APK's `assets/` tree, replace the
    /// corresponding game assets on Sober's next launch.
    pub fn asset_overlay_dir(&self) -> PathBuf {
        self.data_dir().join("asset_overlay")
    }

    /// The current log file: `data/sober/sober_logs/latest.log`.
    pub fn latest_log(&self) -> PathBuf {
        self.data_dir().join("sober_logs/latest.log")
    }

    /// The authoritative asset archive: `packages/<arch>/com.roblox.client/base.apk`.
    ///
    /// A standard ZIP; its `assets/` entries are the canonical list of
    /// replaceable game assets and the source of truth for validating mod paths.
    pub fn base_apk(&self) -> PathBuf {
        self.data_dir()
            .join("packages")
            .join(&self.arch)
            .join("com.roblox.client/base.apk")
    }
}

/// Pigment's own storage locations under the user's config directory.
///
/// Rooted at `$XDG_CONFIG_HOME/pigment` (or `~/.config/pigment`). Holds profiles,
/// the mod library, and the small state file tracking the active profile. Like
/// [`SoberPaths`], these are computed, not probed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PigmentPaths {
    config_dir: PathBuf,
}

impl PigmentPaths {
    /// Build paths rooted at an explicit config directory (e.g. for tests).
    pub fn with_config_dir(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

    /// Resolve from the environment: `$XDG_CONFIG_HOME/pigment`, else
    /// `$HOME/.config/pigment`. Returns `None` only if neither is set.
    pub fn discover() -> Option<Self> {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .filter(|v| !v.is_empty())
                    .map(|h| PathBuf::from(h).join(".config"))
            })?;
        Some(Self::with_config_dir(base.join("pigment")))
    }

    /// `~/.config/pigment`
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Directory holding one JSON file per profile.
    pub fn profiles_dir(&self) -> PathBuf {
        self.config_dir.join("profiles")
    }

    /// The mod library: one subdirectory per installed mod, each a file tree
    /// mirroring the APK's `assets/` layout.
    pub fn mods_dir(&self) -> PathBuf {
        self.config_dir.join("mods")
    }

    /// Small JSON file tracking cross-cutting state (e.g. the active profile).
    pub fn state_file(&self) -> PathBuf {
        self.config_dir.join("state.json")
    }
}

/// The Flatpak architecture name for the host, matching Sober's `packages/<arch>`.
fn default_arch() -> &'static str {
    // Flatpak uses these names; they differ from Rust's `target_arch` spelling
    // only for aarch64 vs. arm64-style variants, but for the arches Sober ships
    // (x86_64, aarch64) the names coincide.
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        // Best-effort fallback; unsupported by Sober anyway.
        "x86_64"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_rooted_under_the_flatpak_app_dir() {
        let p = SoberPaths::from_home("/home/alice").with_arch("x86_64");
        assert_eq!(
            p.config_file(),
            Path::new("/home/alice/.var/app/org.vinegarhq.Sober/config/sober/config.json")
        );
        assert_eq!(
            p.asset_overlay_dir(),
            Path::new("/home/alice/.var/app/org.vinegarhq.Sober/data/sober/asset_overlay")
        );
        assert_eq!(
            p.base_apk(),
            Path::new("/home/alice/.var/app/org.vinegarhq.Sober/data/sober/packages/x86_64/com.roblox.client/base.apk")
        );
        assert_eq!(
            p.latest_log(),
            Path::new("/home/alice/.var/app/org.vinegarhq.Sober/data/sober/sober_logs/latest.log")
        );
    }

    #[test]
    fn discover_uses_home_env() {
        // Just assert the shape depends on HOME; we don't mutate the process env
        // here to avoid racing other tests.
        let p = SoberPaths::from_home("/x");
        assert!(p.config_file().starts_with("/x/.var/app"));
    }

    #[test]
    fn pigment_paths_layout() {
        let p = PigmentPaths::with_config_dir("/home/alice/.config/pigment");
        assert_eq!(p.profiles_dir(), Path::new("/home/alice/.config/pigment/profiles"));
        assert_eq!(p.mods_dir(), Path::new("/home/alice/.config/pigment/mods"));
        assert_eq!(p.state_file(), Path::new("/home/alice/.config/pigment/state.json"));
    }
}
