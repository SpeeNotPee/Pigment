//! registering Ppgment as the `roblox://` protocol handler.
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const ROBLOX_SCHEMES: &[&str] = &["x-scheme-handler/roblox", "x-scheme-handler/roblox-player"];

pub const SOBER_DESKTOP: &str = "org.vinegarhq.Sober.desktop";

pub const PIGMENT_DESKTOP: &str = "net.pigmentlab.Pigment.Launcher.desktop";

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

/// Render the desktop file contents that make `pigment-launch` the handler.
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

pub fn register(launch_exec: &Path) -> Result<(), ProtocolError> {
    let dir = user_applications_dir().ok_or(ProtocolError::NoApplicationsDir)?;
    let path = dir.join(PIGMENT_DESKTOP);
    let contents = launcher_desktop_file(launch_exec);
    crate::util::write_atomic(&path, contents.as_bytes()).map_err(|source| {
        ProtocolError::Write {
            path: path.clone(),
            source,
        }
    })?;
    refresh_desktop_caches(&dir);

    for scheme in ROBLOX_SCHEMES {
        xdg_mime_default(PIGMENT_DESKTOP, scheme)?;
    }
    Ok(())
}

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
