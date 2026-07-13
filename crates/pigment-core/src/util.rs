//! Small filesystem helpers shared across modules.

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

/// Atomically write `bytes` to `path`: write to a temp file in the same
/// directory, `fsync` it, then `rename` over the target. On a single filesystem
/// the rename is atomic, so a reader (or a crash) never sees a partial file.
/// Parent directories are created as needed.
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

/// Recursively copy the contents of `src` into `dst`, creating `dst` and any
/// intermediate directories. Only regular files and directories are copied.
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
