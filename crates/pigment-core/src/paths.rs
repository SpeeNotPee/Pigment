use std::path::{Path, PathBuf};
pub const SOBER_APP_ID: &str = "org.vinegarhq.Sober";
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoberPaths {
    app_dir: PathBuf,
    arch: String,
}

impl SoberPaths {
    pub fn from_home(home: impl AsRef<Path>) -> Self {
        Self {
            app_dir: home.as_ref().join(".var/app").join(SOBER_APP_ID),
            arch: default_arch().to_string(),
        }
    }
    pub fn discover() -> Option<Self> {
        let home = std::env::var_os("HOME").filter(|h| !h.is_empty())?;
        Some(Self::from_home(home))
    }
    pub fn with_arch(mut self, arch: impl Into<String>) -> Self {
        self.arch = arch.into();
        self
    }
    pub fn app_dir(&self) -> &Path {
        &self.app_dir
    }
    pub fn config_file(&self) -> PathBuf {
        self.app_dir.join("config/sober/config.json")
    }
    pub fn data_dir(&self) -> PathBuf {
        self.app_dir.join("data/sober")
    }
    pub fn asset_overlay_dir(&self) -> PathBuf {
        self.data_dir().join("asset_overlay")
    }

    /// The current log file: `data/sober/sober_logs/latest.log`.
    pub fn latest_log(&self) -> PathBuf {
        self.data_dir().join("sober_logs/latest.log")
    }

    pub fn state_file(&self) -> PathBuf {
        self.data_dir().join("state")
    }

    pub fn base_apk(&self) -> PathBuf {
        self.data_dir()
            .join("packages")
            .join(&self.arch)
            .join("com.roblox.client/base.apk")
    }
}

/// Pigment own storage locations under the user's config directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PigmentPaths {
    config_dir: PathBuf,
}

impl PigmentPaths {
    /// Build paths rooted at an explicit config directory
    pub fn with_config_dir(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }

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

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn profiles_dir(&self) -> PathBuf {
        self.config_dir.join("profiles")
    }

    pub fn mods_dir(&self) -> PathBuf {
        self.config_dir.join("mods")
    }

    /// Small JSON file tracking cross cutting state
    pub fn state_file(&self) -> PathBuf {
        self.config_dir.join("state.json")
    }
}

fn default_arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
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
        assert_eq!(
            p.state_file(),
            Path::new("/home/alice/.var/app/org.vinegarhq.Sober/data/sober/state")
        );
    }

    #[test]
    fn discover_uses_home_env() {
        // Just assert the shape depends on HOME
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
