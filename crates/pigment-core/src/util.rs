//! small filesystem helpers shared across modules.

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().filter(|p| !p.as_os_str().is_empty());
    let dir = dir.unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(dir)?;

    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    tmp.write_all(bytes)?;
    tmp.as_file_mut().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

pub(crate) fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if ft.is_dir() {
            copy_dir_all(&from, &to)?;
        } else if ft.is_file() {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// the one name check for mods + profiles. every name that becomes a path goes thru here
/// so `..` or `a/b` cant ever get joined onto a dir n yeet stuff outside it
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name == name.trim()
        && !name.starts_with('.')
        && name.chars().any(char::is_alphanumeric)
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | ' '))
}

/// swap a fully built staging dir into place. old one only dies after the new one exists,
/// so a failed copy never leaves u w/ nothing
pub(crate) fn swap_dir_into_place(staging: &Path, dest: &Path) -> io::Result<()> {
    let old = dest.with_file_name(format!(
        ".{}.old",
        dest.file_name().and_then(|s| s.to_str()).unwrap_or("dir")
    ));
    if old.exists() {
        fs::remove_dir_all(&old)?;
    }
    let had_old = dest.exists();
    if had_old {
        fs::rename(dest, &old)?;
    }
    if let Err(e) = fs::rename(staging, dest) {
        //put the old one back, better stale than gone
        if had_old {
            let _ = fs::rename(&old, dest);
        }
        return Err(e);
    }
    if had_old {
        let _ = fs::remove_dir_all(&old);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_that_escape_are_rejected() {
        for bad in [
            "",
            ".",
            "..",
            " .. ",
            "a/..",
            "../x",
            "a/b",
            ".hidden",
            "  ",
            "x\0y",
            "___",
            &"a".repeat(65),
        ] {
            assert!(!is_valid_name(bad), "{bad:?} should be invalid");
        }
        for ok in [
            "main",
            "PvP Ranked",
            "dark-cursor_v1.2",
            "font-Cool Font",
            "Default",
        ] {
            assert!(is_valid_name(ok), "{ok:?} should be valid");
        }
    }
}
