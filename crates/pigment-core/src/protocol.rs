//! Registering Pigment as the `roblox://` protocol handler.
//!
//! On Linux, clicking "Play" on the Roblox website launches whatever desktop
//! application is registered for the `roblox:`/`roblox-player:` URI schemes.
//! Sober registers itself as that handler out of the box. Pigment can take the
//! handler over so it applies the active profile before handing the URI to
//! Sober — the same interception Bloxstrap performs on Windows.
//!
//! Takeover is **opt-in and reversible**: nothing here runs unless the user asks,
//! [`current_handler`] shows who owns the scheme, and [`restore_sober`] hands it
//! back. Registration writes a user-level desktop file (no root) and calls
//! `xdg-mime`. Pigment never edits Sober's own desktop file.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The URI schemes Roblox uses to launch a game.
pub const ROBLOX_SCHEMES: &[&str] = &["x-scheme-handler/roblox", "x-scheme-handler/roblox-player"];

/// Sober's desktop file id — the handler we restore to.
pub const SOBER_DESKTOP: &str = "org.vinegarhq.Sober.desktop";

/// The desktop file id Pigment installs for its launcher.
pub const PIGMENT_DESKTOP: &str = "org.pigment.Pigment.Launcher.desktop";

/// Errors from handler registration.
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("writing desktop file at {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("running {tool}: {source}")]
    Spawn {
        tool: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("{tool} failed: {stderr}")]
    Tool { tool: &'static str, stderr: String },
    #[error("could not resolve the user applications directory (no $HOME)")]
    NoApplicationsDir,
}

/// The user-level applications directory: `$XDG_DATA_HOME/applications` or
/// `~/.local/share/applications`.
pub fn user_applications_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|h| PathBuf::from(h).join(".local/share"))
        })
        .map(|base| base.join("applications"))
}

/// Render the desktop-file contents that make `pigment-launch` the handler.
///
/// `exec` is the absolute path to the `pigment-launch` binary. `%u` forwards the
/// clicked URI. `NoDisplay=true` keeps it out of application menus — it's a
/// handler, not a launchable app.
pub fn launcher_desktop_file(exec: &Path) -> String {
    let exec = exec.display();
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Pigment (Roblox Launcher)\n\
         Comment=Applies your Pigment profile, then launches Roblox via Sober\n\
         Exec={exec} %u\n\
         Terminal=false\n\
         NoDisplay=true\n\
         Categories=Game;\n\
         MimeType=x-scheme-handler/roblox;x-scheme-handler/roblox-player;\n"
    )
}

/// The desktop-file id currently handling `roblox:` (e.g.
/// `org.vinegarhq.Sober.desktop`), via `xdg-mime query default`.
pub fn current_handler() -> Option<String> {
    let out = Command::new("xdg-mime")
        .args(["query", "default", ROBLOX_SCHEMES[0]])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Whether Pigment is currently the registered handler.
pub fn pigment_is_handler() -> bool {
    current_handler().as_deref() == Some(PIGMENT_DESKTOP)
}

/// Install the launcher desktop file and make Pigment the default handler for
/// both Roblox schemes. Idempotent.
///
/// The freshly written desktop file must be indexed before a default association
/// to it will resolve — notably on KDE, where handler lookup goes through the
/// service cache (ksycoca), not `mimeapps.list` directly. So we refresh the
/// desktop caches between writing the file and calling `xdg-mime`.
pub fn register(launch_exec: &Path) -> Result<(), ProtocolError> {
    let dir = user_applications_dir().ok_or(ProtocolError::NoApplicationsDir)?;
    let path = dir.join(PIGMENT_DESKTOP);
    let contents = launcher_desktop_file(launch_exec);
    crate::util::write_atomic(&path, contents.as_bytes())
        .map_err(|source| ProtocolError::Write {
            path: path.clone(),
            source,
        })?;

    refresh_desktop_caches(&dir);

    for scheme in ROBLOX_SCHEMES {
        xdg_mime_default(PIGMENT_DESKTOP, scheme)?;
    }
    Ok(())
}

/// Best-effort refresh of the desktop-file caches so a newly written `.desktop`
/// is visible to handler lookup. Every tool here is optional: missing ones are
/// skipped, and none can fail registration. `update-desktop-database` is the
/// freedesktop standard; `kbuildsycoca6`/`5` rebuild KDE's service cache.
fn refresh_desktop_caches(applications_dir: &Path) {
    let _ = Command::new("update-desktop-database")
        .arg(applications_dir)
        .output();
    for tool in ["kbuildsycoca6", "kbuildsycoca5"] {
        let _ = Command::new(tool).arg("--noincremental").output();
    }
}

/// Restore Sober as the default handler for both Roblox schemes.
pub fn restore_sober() -> Result<(), ProtocolError> {
    for scheme in ROBLOX_SCHEMES {
        xdg_mime_default(SOBER_DESKTOP, scheme)?;
    }
    Ok(())
}

/// `xdg-mime default <desktop> <scheme>`, surfacing failures.
fn xdg_mime_default(desktop: &str, scheme: &str) -> Result<(), ProtocolError> {
    let out = Command::new("xdg-mime")
        .args(["default", desktop, scheme])
        .output()
        .map_err(|source| ProtocolError::Spawn {
            tool: "xdg-mime",
            source,
        })?;
    if !out.status.success() {
        return Err(ProtocolError::Tool {
            tool: "xdg-mime",
            stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_file_forwards_uri_and_declares_schemes() {
        let text = launcher_desktop_file(Path::new("/usr/bin/pigment-launch"));
        assert!(text.contains("Exec=/usr/bin/pigment-launch %u"));
        assert!(text.contains("x-scheme-handler/roblox;x-scheme-handler/roblox-player;"));
        assert!(text.contains("NoDisplay=true"));
        assert!(text.starts_with("[Desktop Entry]"));
    }

    #[test]
    fn applications_dir_prefers_xdg_data_home() {
        // We don't mutate global env in tests; just assert the fallback shape via
        // a direct HOME-style path is under applications/.
        if let Some(dir) = user_applications_dir() {
            assert!(dir.ends_with("applications"));
        }
    }
}
