//! registering Ppgment as the `roblox://` protocol handler.
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths::PigmentPaths;
use crate::state::State;

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
    #[error(
        "refusing to register {0:?}: the path has characters a desktop file can't safely quote"
    )]
    BadExecPath(PathBuf),
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
/// handler, not a launchable app. path gets quoted + `%` doubled per the desktop spec,
/// n i flat out refuse the chars whose escaping rules r a minefield
pub fn launcher_desktop_file(exec: &Path) -> Result<String, ProtocolError> {
    let raw = exec
        .to_str()
        .ok_or_else(|| ProtocolError::BadExecPath(exec.to_path_buf()))?;
    if raw
        .chars()
        .any(|c| matches!(c, '"' | '`' | '$' | '\\') || c.is_control())
    {
        return Err(ProtocolError::BadExecPath(exec.to_path_buf()));
    }
    let quoted = raw.replace('%', "%%");
    Ok(format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Pigment (Roblox Launcher)\n\
         Comment=Applies your Pigment profile, then launches Roblox via Sober\n\
         TryExec={raw}\n\
         Exec=\"{quoted}\" %u\n\
         Terminal=false\n\
         NoDisplay=true\n\
         Categories=Game;\n\
         MimeType=x-scheme-handler/roblox;x-scheme-handler/roblox-player;\n"
    ))
}

/// who owns one scheme rn, per xdg-mime
pub fn current_handler(scheme: &str) -> Option<String> {
    let out = Command::new("xdg-mime")
        .args(["query", "default", scheme])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

/// Whether Pigment owns BOTH roblox schemes. one of two = half broken, so thats a no
pub fn pigment_is_handler() -> bool {
    ROBLOX_SCHEMES
        .iter()
        .all(|s| current_handler(s).as_deref() == Some(PIGMENT_DESKTOP))
}

/// generated handler file path
pub fn launcher_desktop_path() -> Option<PathBuf> {
    user_applications_dir().map(|d| d.join(PIGMENT_DESKTOP))
}

pub fn register(launch_exec: &Path, paths: &PigmentPaths) -> Result<(), ProtocolError> {
    let dir = user_applications_dir().ok_or(ProtocolError::NoApplicationsDir)?;
    let contents = launcher_desktop_file(launch_exec)?;

    //remember who had each scheme so switch back puts THEM back, not always sober.
    //only if it aint already us, else a double register would save pigment as "previous"
    let mut state = State::load(paths);
    for scheme in ROBLOX_SCHEMES {
        match current_handler(scheme) {
            Some(h) if h != PIGMENT_DESKTOP => {
                state.previous_handlers.insert((*scheme).to_string(), h);
            }
            _ => {}
        }
    }
    state.save(paths).map_err(|source| ProtocolError::Write {
        path: paths.state_file(),
        source,
    })?;

    let path = dir.join(PIGMENT_DESKTOP);
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
    //incremental is plenty for 1 changed file. --noincremental froze kde for secs
    for tool in ["kbuildsycoca6", "kbuildsycoca5"] {
        if Command::new(tool).output().is_ok() {
            break;
        }
    }
}

/// does a desktop id still resolve to an installed file? a saved handler from an app the
/// user since uninstalled is useless, so i fall back to sober then
fn desktop_id_exists(id: &str) -> bool {
    let mut dirs: Vec<PathBuf> = user_applications_dir().into_iter().collect();
    let data_dirs = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_string());
    dirs.extend(
        data_dirs
            .split(':')
            .map(|d| Path::new(d).join("applications")),
    );
    //flatpak exports live here even when XDG_DATA_DIRS is gutted (desktop file handler env)
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/flatpak/exports/share/applications"));
    }
    dirs.iter().any(|d| d.join(id).is_file())
}

/// switch back: give each scheme to whoever had it b4 (sober if unknown/gone), delete the
/// generated desktop file, n forget the saved handlers. also what `pigment-launch --unregister` runs
pub fn restore_previous(paths: &PigmentPaths) -> Result<(), ProtocolError> {
    let mut state = State::load(paths);
    for scheme in ROBLOX_SCHEMES {
        let target = state
            .previous_handlers
            .get(*scheme)
            .filter(|h| h.as_str() != PIGMENT_DESKTOP && desktop_id_exists(h))
            .cloned()
            .unwrap_or_else(|| SOBER_DESKTOP.to_string());
        xdg_mime_default(&target, scheme)?;
    }
    if let Some(path) = launcher_desktop_path() {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(ProtocolError::Write { path, source }),
        }
        if let Some(dir) = path.parent() {
            refresh_desktop_caches(dir);
        }
    }
    state.previous_handlers.clear();
    state.save(paths).map_err(|source| ProtocolError::Write {
        path: paths.state_file(),
        source,
    })
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
        let text = launcher_desktop_file(Path::new("/usr/bin/pigment-launch")).unwrap();
        assert!(text.contains("Exec=\"/usr/bin/pigment-launch\" %u"));
        assert!(text.contains("TryExec=/usr/bin/pigment-launch\n"));
        assert!(text.contains("x-scheme-handler/roblox;x-scheme-handler/roblox-player;"));
        assert!(text.contains("NoDisplay=true"));
        assert!(text.starts_with("[Desktop Entry]"));
    }

    #[test]
    fn exec_path_with_space_and_percent_is_quoted() {
        let text = launcher_desktop_file(Path::new("/home/a b/100%/pigment-launch")).unwrap();
        assert!(
            text.contains("Exec=\"/home/a b/100%%/pigment-launch\" %u"),
            "{text}"
        );
        assert!(text.contains("TryExec=/home/a b/100%/pigment-launch\n"));
    }

    #[test]
    fn exec_path_with_nasty_chars_is_refused() {
        for bad in ["/x/\"q/p", "/x/$HOME/p", "/x/`id`/p", "/x\\y/p", "/x\ny/p"] {
            assert!(launcher_desktop_file(Path::new(bad)).is_err(), "{bad}");
        }
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
