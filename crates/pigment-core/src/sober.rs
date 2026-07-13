//! Discovering and launching the Sober Flatpak.
//!
//! Pigment never bundles or reimplements Sober; it shells out to `flatpak`.
//! Command construction ([`LaunchSpec`], [`Sober::launch_spec`]) is kept pure and
//! separate from execution so it can be unit-tested without a Flatpak install,
//! and so `pigment-launch` can build the exact argv it will exec on the hot path.

use std::path::PathBuf;
use std::process::Command;

use crate::paths::{SoberPaths, SOBER_APP_ID};

/// The Flatpak CLI binary. Absolute path avoids `$PATH` surprises when launched
/// from a desktop-file handler with a minimal environment.
const FLATPAK_BIN: &str = "flatpak";

/// A fully-resolved launch command: the program plus its arguments.
///
/// Pure data — building one runs nothing. Convert to a [`Command`] with
/// [`LaunchSpec::to_command`] when ready to execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub program: String,
    pub args: Vec<String>,
}

impl LaunchSpec {
    /// Materialize a runnable [`Command`].
    pub fn to_command(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        cmd
    }

    /// The full argv as a single shell-ish string, for logging/debugging only.
    pub fn display(&self) -> String {
        let mut s = self.program.clone();
        for a in &self.args {
            s.push(' ');
            s.push_str(a);
        }
        s
    }
}

/// A handle to the Sober runtime on this machine.
#[derive(Debug, Clone)]
pub struct Sober {
    paths: SoberPaths,
    /// The `flatpak` binary to invoke (overridable in tests).
    flatpak_bin: String,
}

impl Sober {
    /// Create a handle from resolved Sober paths.
    pub fn new(paths: SoberPaths) -> Self {
        Self {
            paths,
            flatpak_bin: FLATPAK_BIN.to_string(),
        }
    }

    /// Create a handle from the current user's environment.
    pub fn discover() -> Option<Self> {
        Some(Self::new(SoberPaths::discover()?))
    }

    /// The resolved Sober paths.
    pub fn paths(&self) -> &SoberPaths {
        &self.paths
    }

    /// Build the command that launches Sober, optionally into a deep-link URI.
    ///
    /// `flatpak run org.vinegarhq.Sober [uri]`. Sober takes the `roblox:`/
    /// `roblox-player:` URI as its first positional argument (this is what the
    /// registered protocol handler forwards). With no URI, Sober opens its home
    /// screen.
    pub fn launch_spec(&self, uri: Option<&str>) -> LaunchSpec {
        let mut args = vec!["run".to_string(), SOBER_APP_ID.to_string()];
        if let Some(uri) = uri {
            args.push(uri.to_string());
        }
        LaunchSpec {
            program: self.flatpak_bin.clone(),
            args,
        }
    }

    /// Build the command that opens Sober's own settings dialog.
    pub fn settings_spec(&self) -> LaunchSpec {
        LaunchSpec {
            program: self.flatpak_bin.clone(),
            args: vec![
                "run".to_string(),
                "--command=sober".to_string(),
                SOBER_APP_ID.to_string(),
                "config".to_string(),
            ],
        }
    }

    /// Spawn Sober, optionally into a deep-link URI. Returns immediately with the
    /// child handle; does not wait for Roblox to exit.
    pub fn launch(&self, uri: Option<&str>) -> std::io::Result<std::process::Child> {
        self.launch_spec(uri).to_command().spawn()
    }

    /// Whether the Sober Flatpak is installed, by asking `flatpak info`.
    ///
    /// Returns `false` if `flatpak` itself is missing.
    pub fn is_installed(&self) -> bool {
        Command::new(&self.flatpak_bin)
            .args(["info", SOBER_APP_ID])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// The installed Sober version (e.g. `"1.7.1"`), or `None` if not installed
    /// or unparseable.
    pub fn installed_version(&self) -> Option<String> {
        let out = Command::new(&self.flatpak_bin)
            .args(["info", SOBER_APP_ID])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        parse_version(&String::from_utf8_lossy(&out.stdout))
    }

    /// Whether Sober has been launched at least once, inferred from the config
    /// existing. Callers use this to decide whether editing the config is safe
    /// (Sober regenerates it on first launch).
    pub fn has_config(&self) -> bool {
        self.paths.config_file().exists()
    }

    /// The config file path (convenience re-export).
    pub fn config_file(&self) -> PathBuf {
        self.paths.config_file()
    }
}

/// Extract the `Version:` field from `flatpak info` output.
fn parse_version(info: &str) -> Option<String> {
    info.lines()
        .map(str::trim_start)
        .find_map(|line| line.strip_prefix("Version:"))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sober() -> Sober {
        Sober::new(SoberPaths::from_home("/home/alice"))
    }

    #[test]
    fn launch_spec_without_uri_opens_app() {
        let spec = sober().launch_spec(None);
        assert_eq!(spec.program, "flatpak");
        assert_eq!(spec.args, vec!["run", "org.vinegarhq.Sober"]);
    }

    #[test]
    fn launch_spec_forwards_uri_as_positional_arg() {
        let spec = sober().launch_spec(Some("roblox://placeId=123"));
        assert_eq!(
            spec.args,
            vec!["run", "org.vinegarhq.Sober", "roblox://placeId=123"]
        );
    }

    #[test]
    fn settings_spec_targets_sober_config_subcommand() {
        let spec = sober().settings_spec();
        assert_eq!(
            spec.args,
            vec!["run", "--command=sober", "org.vinegarhq.Sober", "config"]
        );
    }

    #[test]
    fn parses_version_from_real_flatpak_info() {
        // Captured verbatim from `flatpak info org.vinegarhq.Sober` on 1.7.1.
        let sample = "\n\
Sober - Play, chat & explore on Roblox\n\
\n\
            ID: org.vinegarhq.Sober\n\
           Ref: app/org.vinegarhq.Sober/x86_64/stable\n\
          Arch: x86_64\n\
        Branch: stable\n\
       Version: 1.7.1\n\
       License: LicenseRef-proprietary\n\
Installed Size: 17.8 MB\n";
        assert_eq!(parse_version(sample).as_deref(), Some("1.7.1"));
    }

    #[test]
    fn version_absent_yields_none() {
        assert_eq!(parse_version("ID: org.vinegarhq.Sober\nArch: x86_64\n"), None);
    }

    #[test]
    fn display_renders_full_argv() {
        let spec = sober().launch_spec(Some("roblox://x"));
        assert_eq!(spec.display(), "flatpak run org.vinegarhq.Sober roblox://x");
    }
}
