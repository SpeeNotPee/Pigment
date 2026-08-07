use std::path::PathBuf;
use std::process::Command;

use crate::paths::{SoberPaths, SOBER_APP_ID};

const FLATPAK_BIN: &str = "flatpak";

const DEFAULT_REMOTE: &str = "flathub";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildInfo {
    pub version: Option<String>,
    pub commit: Option<String>,
    pub build: Option<String>,
    pub date: Option<String>,
    pub origin: Option<String>,
}

impl BuildInfo {
    pub fn label(&self) -> Option<String> {
        let version = self.version.as_deref()?;
        match self.build_date() {
            Some(d) => Some(format!("{version} (build {d})")),
            None => Some(version.to_string()),
        }
    }

    pub fn build_date(&self) -> Option<&str> {
        self.build
            .as_deref()
            .and_then(|b| b.split('_').next())
            .or(self.date.as_deref())
    }
}


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

    /// The full argv as a single shell ish string, for logging/debugging only.
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

    /// Build the command that launches Sober, optionally into a deep link URI.
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

    /// Build the command that opens Sober own settings dialog.
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

    pub fn launch(&self, uri: Option<&str>) -> std::io::Result<std::process::Child> {
        self.launch_spec(uri).to_command().spawn()
    }

    /// Whether the Sober Flatpak is installed, by asking `flatpak info`.
    pub fn is_installed(&self) -> bool {
        self.flatpak_output(&["info", SOBER_APP_ID]).is_some()
    }

    fn flatpak_output(&self, extra: &[&str]) -> Option<String> {
        let out = Command::new(&self.flatpak_bin).args(extra).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    pub fn installed_version(&self) -> Option<String> {
        self.installed_build()?.version
    }

    /// [`Sober::is_installed`] and [`Sober::installed_version`] separately.
    pub fn installed_build(&self) -> Option<BuildInfo> {
        Some(parse_build(&self.flatpak_output(&["info", SOBER_APP_ID])?))
    }

    /// Build details for the newest Sober published on its remote.
    pub fn latest_build(&self) -> Option<BuildInfo> {
        let origin = self
            .installed_build()
            .and_then(|b| b.origin)
            .unwrap_or_else(|| DEFAULT_REMOTE.to_string());
        Some(parse_build(&self.flatpak_output(&[
            "remote-info",
            &origin,
            SOBER_APP_ID,
        ])?))
    }

    /// The newer build available on the remote, or `None` if Sober is current
    pub fn update_available(&self) -> Option<BuildInfo> {
        let installed = self.installed_build()?.commit?;
        let latest = self.latest_build()?;
        (latest.commit.as_deref()? != installed).then_some(latest)
    }

    pub fn roblox_version(&self) -> Option<String> {
        let text = std::fs::read_to_string(self.paths.state_file()).ok()?;
        parse_roblox_version(&text)
    }

    pub fn has_config(&self) -> bool {
        self.paths.config_file().exists()
    }

    pub fn config_file(&self) -> PathBuf {
        self.paths.config_file()
    }
}


fn parse_field(info: &str, field: &str) -> Option<String> {
    let prefix = format!("{field}:");
    info.lines()
        .map(str::trim_start)
        .find_map(|line| line.strip_prefix(&prefix))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn parse_version(info: &str) -> Option<String> {
    parse_field(info, "Version")
}

/// Parse the fields Pigment tracks out of `flatpak info` / `remote-info` output.
fn parse_build(info: &str) -> BuildInfo {
    BuildInfo {
        version: parse_version(info),
        commit: parse_field(info, "Commit"),
        build: parse_field(info, "Subject").as_deref().and_then(parse_build_tag),
        // `Date: 2026-07-28 00:04:56 +0000` — keep the day, drop the clock.
        date: parse_field(info, "Date").map(|d| {
            d.split_whitespace()
                .next()
                .unwrap_or(d.as_str())
                .to_string()
        }),
        origin: parse_field(info, "Origin"),
    }
}

fn parse_build_tag(subject: &str) -> Option<String> {
    subject
        .split_whitespace()
        .find(|token| {
            let Some((date, hash)) = token.split_once('_') else {
                return false;
            };
            let d: Vec<&str> = date.split('-').collect();
            d.len() == 3
                && d.iter().all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
                && !hash.is_empty()
                && hash.bytes().all(|b| b.is_ascii_hexdigit())
        })
        .map(str::to_string)
}

/// deep link for joining a game. job id pins the exact server, w/o it roblox picks a fresh one
/// heads up: job ids die when the server shuts down, roblox will js error at u. not my problem
pub fn join_uri(place_id: u64, job_id: Option<&str>) -> String {
    match job_id {
        Some(job) if !job.is_empty() => {
            format!("roblox://experiences/start?placeId={place_id}&gameInstanceId={job}")
        }
        _ => format!("roblox://experiences/start?placeId={place_id}"),
    }
}

/// Read `v1.app_version` — the Roblox client version — from Sober's state file.
fn parse_roblox_version(state_json: &str) -> Option<String> {
    let root: serde_json::Value = serde_json::from_str(state_json).ok()?;
    root.get("v1")?
        .get("app_version")?
        .as_str()
        .filter(|v| !v.is_empty())
        .map(str::to_string)
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

    const INFO_SAMPLE: &str = "\n\
Sober - Play, chat & explore on Roblox\n\
\n\
            ID: org.vinegarhq.Sober\n\
           Ref: app/org.vinegarhq.Sober/x86_64/stable\n\
          Arch: x86_64\n\
        Branch: stable\n\
       Version: 1.7.1\n\
       License: LicenseRef-proprietary\n\
        Origin: flathub\n\
    Collection: org.flathub.Stable\n\
Installed Size: 18.0 MB\n\
\n\
        Commit: 3f0141ef9c95ff47a08a1437d5b328bd8a6cdf749de592b06579dd5f3fcf948b\n\
        Parent: 2b9e6dd4af698d1c3950f8d8eb2823168eb25b8c1d5de9e3ae5ff0767ae53667\n\
       Subject: Update Sober to 2026-07-21_feffe25 (1.7.1 refresh) (9e07049f9e9c)\n\
          Date: 2026-07-22 01:28:52 +0000\n";

    #[test]
    fn parses_version_from_real_flatpak_info() {
        assert_eq!(parse_version(INFO_SAMPLE).as_deref(), Some("1.7.1"));
    }

    #[test]
    fn version_absent_yields_none() {
        assert_eq!(parse_version("ID: org.vinegarhq.Sober\nArch: x86_64\n"), None);
    }

    #[test]
    fn parses_full_build_from_real_flatpak_info() {
        let b = parse_build(INFO_SAMPLE);
        assert_eq!(b.version.as_deref(), Some("1.7.1"));
        assert_eq!(b.origin.as_deref(), Some("flathub"));
        assert_eq!(
            b.commit.as_deref(),
            Some("3f0141ef9c95ff47a08a1437d5b328bd8a6cdf749de592b06579dd5f3fcf948b")
        );
        assert_eq!(b.build.as_deref(), Some("2026-07-21_feffe25"));
        // The clock time is dropped; only the day is kept.
        assert_eq!(b.date.as_deref(), Some("2026-07-22"));
        assert_eq!(b.label().as_deref(), Some("1.7.1 (build 2026-07-21)"));
    }

    #[test]
    fn build_label_falls_back_when_subject_has_no_tag() {
        // A refresh whose subject doesn't follow the `<date>_<hash>` convention
        // still reports the version, dated by the commit.
        let b = BuildInfo {
            version: Some("1.7.1".into()),
            date: Some("2026-07-28".into()),
            ..Default::default()
        };
        assert_eq!(b.label().as_deref(), Some("1.7.1 (build 2026-07-28)"));

        // With nothing but a version, the label is just the version.
        let bare = BuildInfo {
            version: Some("1.7.1".into()),
            ..Default::default()
        };
        assert_eq!(bare.label().as_deref(), Some("1.7.1"));
        assert_eq!(BuildInfo::default().label(), None);
    }

    #[test]
    fn build_tag_is_only_taken_from_a_dated_hash_token() {
        assert_eq!(
            parse_build_tag("Update Sober to 2026-07-28_a4ebce8 (1.7.1 refresh)").as_deref(),
            Some("2026-07-28_a4ebce8")
        );
        assert_eq!(parse_build_tag("Initial commit"), None);
        assert_eq!(parse_build_tag("Update Sober to 1.7.1"), None);
        // Not a hash, so not a build tag.
        assert_eq!(parse_build_tag("bump 2026-07-28_release"), None);
    }

    #[test]
    fn join_uri_with_and_without_a_server() {
        assert_eq!(
            join_uri(17625359962, None),
            "roblox://experiences/start?placeId=17625359962"
        );
        assert_eq!(
            join_uri(17625359962, Some("135d0895-503a-4884-885e-c963758024e3")),
            "roblox://experiences/start?placeId=17625359962&gameInstanceId=135d0895-503a-4884-885e-c963758024e3"
        );
        // empty job id = same as no job id, dont emit a dangling param
        assert_eq!(
            join_uri(5, Some("")),
            "roblox://experiences/start?placeId=5"
        );
    }

    #[test]
    fn parses_roblox_version_from_real_state_file() {
        // Trimmed from the live `data/sober/state`.
        let state = r#"{
    "v1": { "app_version": "2.729.839", "fullscreen": false },
    "v2": { "has_seen_onboarding": false }
}"#;
        assert_eq!(parse_roblox_version(state).as_deref(), Some("2.729.839"));
        // Absent, empty, and malformed states are all "unknown", never a panic.
        assert_eq!(parse_roblox_version(r#"{"v2": {}}"#), None);
        assert_eq!(parse_roblox_version(r#"{"v1": {"app_version": ""}}"#), None);
        assert_eq!(parse_roblox_version("not json"), None);
    }

    #[test]
    fn display_renders_full_argv() {
        let spec = sober().launch_spec(Some("roblox://x"));
        assert_eq!(spec.display(), "flatpak run org.vinegarhq.Sober roblox://x");
    }
}
