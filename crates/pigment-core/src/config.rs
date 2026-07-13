//! Reading and safely rewriting Sober's `config.json`.
//!
//! This is the most safety-critical module in Pigment. Sober has a known
//! crash-on-corrupt-config bug (vinegarhq/sober#1519): if this file is not valid
//! JSON, Roblox will not launch at all. Two further hazards make a naive
//! read/modify/write dangerous:
//!
//! 1. **It is JSONC, not JSON.** The live file opens with a `//` comment header.
//!    A bare `serde_json` parse fails on it outright.
//! 2. **Sober owns the schema and adds keys over time.** The live 1.7.1 file
//!    already carries `enable_mobile_home_screen`, which is absent from the
//!    published docs. Round-tripping through a typed struct would silently drop
//!    any key Pigment doesn't know about.
//!
//! [`Config`] therefore models the file as an editable JSON *document* (an
//! order-independent map of raw values) plus its comment preamble, rather than a
//! fixed struct. Writes are atomic (temp file + `rename`) and are re-parsed for
//! validity *before* the rename, so a bug in Pigment can never leave a
//! half-written or malformed config in place. Every overwrite first snapshots the
//! previous file so a launchable state can always be restored.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize as _;
use serde_json::Value;

/// Suffix appended to the config path for the pre-overwrite backup.
const BACKUP_SUFFIX: &str = ".pigment.bak";

/// Errors from loading, editing, or saving the Sober config.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("reading config at {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("writing config at {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("config is not valid JSONC: {0}")]
    Parse(String),

    /// The top-level JSON value was not an object (`{ ... }`).
    #[error("config root is not a JSON object")]
    NotAnObject,

    /// A serialized document failed to re-parse — refused to write it.
    /// This is the guard against ever handing Sober a corrupt file.
    #[error("internal: refusing to write config that does not re-parse: {0}")]
    WouldCorrupt(String),

    #[error("no backup found at {0}")]
    NoBackup(PathBuf),
}

/// An in-memory, editable view of Sober's `config.json`.
///
/// Preserves the comment header and every key present on disk, known or not.
/// Reserialized output matches Sober's own formatting (4-space indent,
/// alphabetically sorted keys) to keep on-disk diffs minimal.
#[derive(Debug, Clone)]
pub struct Config {
    /// The leading comment/blank lines before the `{`, verbatim (no trailing
    /// newline). Empty if the source had no header.
    preamble: String,
    /// The top-level object. Always a [`Value::Object`]; the type invariant is
    /// established at construction.
    root: Value,
}

impl Config {
    /// Parse a config from raw file text (JSONC).
    ///
    /// The comment header is captured for round-tripping; comments elsewhere are
    /// ignored by the parser (and not preserved). Fails if the JSON body is
    /// invalid or its root is not an object.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let preamble = extract_preamble(text);

        // jsonc-parser understands strings, so a value like "http://x" is never
        // mistaken for a comment — this is why we don't strip `//` by regex.
        let root = jsonc_parser::parse_to_serde_value::<Value>(text, &Default::default())
            .map_err(|e| ConfigError::Parse(e.to_string()))?;

        if !root.is_object() {
            return Err(ConfigError::NotAnObject);
        }
        Ok(Self { preamble, root })
    }

    /// Load and parse the config at `path`.
    ///
    /// A missing file is a [`ConfigError::Read`]; Pigment treats "Sober has never
    /// been launched" as a caller-handled condition rather than fabricating a
    /// default (we do not authoritatively know Sober's full defaults, and Sober
    /// regenerates the file itself on first launch).
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text)
    }

    /// Borrow the top-level object map.
    fn object(&self) -> &serde_json::Map<String, Value> {
        // Invariant established in `parse`.
        self.root.as_object().expect("root is always an object")
    }

    /// Mutably borrow the top-level object map.
    fn object_mut(&mut self) -> &mut serde_json::Map<String, Value> {
        self.root.as_object_mut().expect("root is always an object")
    }

    /// Get a raw value by key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.object().get(key)
    }

    /// Set a key to any JSON value, replacing an existing one.
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<Value>) {
        self.object_mut().insert(key.into(), value.into());
    }

    /// Remove a key, returning its previous value if present.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.object_mut().remove(key)
    }

    /// Read a boolean setting, if present and boolean-typed.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Value::as_bool)
    }

    /// Set a boolean setting.
    pub fn set_bool(&mut self, key: impl Into<String>, value: bool) {
        self.set(key, value);
    }

    /// Replace the entire `fflags` map.
    ///
    /// Sober stores FastFlags as a plain string→value object, the same shape
    /// Bloxstrap uses, so presets copy across directly.
    pub fn set_fflags(&mut self, flags: serde_json::Map<String, Value>) {
        self.set("fflags", Value::Object(flags));
    }

    /// Borrow the `fflags` map, if present and object-typed.
    pub fn fflags(&self) -> Option<&serde_json::Map<String, Value>> {
        self.get("fflags").and_then(Value::as_object)
    }

    /// All top-level keys currently in the document, in sorted order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.object().keys().map(String::as_str)
    }

    /// Serialize to Sober's on-disk format: preserved comment header, then the
    /// object pretty-printed with 4-space indent and sorted keys, then a
    /// trailing newline.
    pub fn to_pretty_string(&self) -> String {
        let mut buf = Vec::new();
        // 4-space indent matches Sober; serde_json's default is 2.
        let formatter = serde_json::ser::PrettyFormatter::with_indent(b"    ");
        let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
        // Value's Map is BTreeMap-backed, so keys serialize alphabetically —
        // matching Sober's own output and keeping diffs minimal.
        self.root
            .serialize(&mut ser)
            .expect("serializing a Value to a Vec cannot fail");
        let body = String::from_utf8(buf).expect("serde_json emits valid UTF-8");

        let mut out = String::new();
        if !self.preamble.is_empty() {
            out.push_str(&self.preamble);
            out.push('\n');
        }
        out.push_str(&body);
        out.push('\n');
        out
    }

    /// Atomically write the config to `path`, backing up any existing file first.
    ///
    /// Sequence, chosen so no failure can leave Sober with an unlaunchable file:
    /// 1. Serialize, then **re-parse** the result. If it isn't valid JSON, abort
    ///    before touching disk ([`ConfigError::WouldCorrupt`]).
    /// 2. Snapshot the existing file to `<path>.pigment.bak`.
    /// 3. Write to a temp file in the *same directory*, `fsync` it, then `rename`
    ///    it over the target — an atomic replace on a single filesystem.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let path = path.as_ref();
        let rendered = self.to_pretty_string();

        // (1) Never write something we can't read back.
        if let Err(e) = serde_json::from_str::<Value>(strip_preamble(&rendered)) {
            return Err(ConfigError::WouldCorrupt(e.to_string()));
        }

        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(dir).map_err(|source| ConfigError::Write {
            path: dir.to_path_buf(),
            source,
        })?;

        // (2) Snapshot the current on-disk config so we can always restore a
        // launchable state, even if the user later edits by hand.
        if path.exists() {
            let backup = backup_path(path);
            fs::copy(path, &backup).map_err(|source| ConfigError::Write {
                path: backup,
                source,
            })?;
        }

        // (3) Atomic write: temp file in the same dir, fsync, rename.
        crate::util::write_atomic(path, rendered.as_bytes()).map_err(|source| {
            ConfigError::Write {
                path: path.to_path_buf(),
                source,
            }
        })
    }

    /// Restore the most recent backup written by [`Config::save`] over `path`.
    pub fn restore_backup(path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let path = path.as_ref();
        let backup = backup_path(path);
        if !backup.exists() {
            return Err(ConfigError::NoBackup(backup));
        }
        fs::copy(&backup, path).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    }
}

/// Path of the backup file for a given config path.
fn backup_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(BACKUP_SUFFIX);
    PathBuf::from(s)
}

/// Capture leading whole-line `//` comments and blank lines as the preamble,
/// stopping at the first content line (which begins the JSON body). Returned
/// without a trailing newline.
fn extract_preamble(text: &str) -> String {
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.is_empty() {
            lines.push(line);
        } else {
            break;
        }
    }
    // Drop trailing blank lines from the captured block so we control spacing.
    while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines.join("\n")
}

/// Strip a leading comment/blank preamble so the remainder is plain JSON, for
/// the pre-write validity re-parse. Assumes comments only appear before the body
/// (true for what we serialize; the body itself contains no comments).
fn strip_preamble(text: &str) -> &str {
    match text.find('{') {
        Some(i) => &text[i..],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A faithful sample of the live Sober 1.7.1 config, including the header,
    /// an undocumented key, and a populated fflags map.
    const LIVE_SAMPLE: &str = r#"// !!! STOP !!!
// This file is not meant to be edited by hand unless you know what you're doing.
// -------------------------------------------
{
    "allow_gamepad_permission": false,
    "close_on_leave": false,
    "discord_rpc_enabled": true,
    "enable_mobile_home_screen": false,
    "fflags": {
        "FFlagExample": true
    },
    "touch_mode": "fake_off",
    "use_opengl": false
}
"#;

    #[test]
    fn parses_jsonc_with_comment_header() {
        let cfg = Config::parse(LIVE_SAMPLE).expect("should parse JSONC");
        assert_eq!(cfg.get_bool("discord_rpc_enabled"), Some(true));
        assert_eq!(cfg.get_bool("use_opengl"), Some(false));
        assert!(cfg.preamble.starts_with("// !!! STOP !!!"));
    }

    #[test]
    fn preserves_comment_header_across_round_trip() {
        let cfg = Config::parse(LIVE_SAMPLE).unwrap();
        let out = cfg.to_pretty_string();
        assert!(out.starts_with("// !!! STOP !!!"), "header lost:\n{out}");
        assert!(out.contains("// ----"), "header body lost");
    }

    #[test]
    fn preserves_unknown_future_keys() {
        // A key Pigment has never heard of must survive a round-trip untouched.
        let src = r#"{ "a_future_sober_key": 42, "use_opengl": true }"#;
        let mut cfg = Config::parse(src).unwrap();
        cfg.set_bool("use_opengl", false); // edit an unrelated key
        let out = cfg.to_pretty_string();
        let reparsed = Config::parse(&out).unwrap();
        assert_eq!(
            reparsed.get("a_future_sober_key").and_then(Value::as_i64),
            Some(42),
            "unknown key was dropped:\n{out}"
        );
        assert_eq!(reparsed.get_bool("use_opengl"), Some(false));
    }

    #[test]
    fn output_is_sorted_and_four_space_indented() {
        let cfg = Config::parse(r#"{ "zeta": 1, "alpha": 2 }"#).unwrap();
        let out = cfg.to_pretty_string();
        let alpha = out.find("alpha").unwrap();
        let zeta = out.find("zeta").unwrap();
        assert!(alpha < zeta, "keys not alphabetized:\n{out}");
        assert!(out.contains("\n    \"alpha\""), "not 4-space indented:\n{out}");
    }

    #[test]
    fn serialized_output_always_reparses() {
        // The core sober#1519 guarantee: whatever we render, Sober can parse.
        let cfg = Config::parse(LIVE_SAMPLE).unwrap();
        let out = cfg.to_pretty_string();
        serde_json::from_str::<Value>(strip_preamble(&out))
            .expect("rendered config must be valid JSON");
    }

    #[test]
    fn save_then_load_round_trips_and_backs_up() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        // First save: no prior file, so no backup created.
        let cfg = Config::parse(LIVE_SAMPLE).unwrap();
        cfg.save(&path).unwrap();
        assert!(path.exists());
        assert!(!backup_path(&path).exists(), "no backup expected on first write");

        // Mutate and save again: previous file is backed up.
        let mut cfg2 = Config::load(&path).unwrap();
        cfg2.set_bool("use_opengl", true);
        cfg2.save(&path).unwrap();

        let reloaded = Config::load(&path).unwrap();
        assert_eq!(reloaded.get_bool("use_opengl"), Some(true));

        // Backup holds the pre-edit value.
        assert!(backup_path(&path).exists());
        let backup = Config::load(backup_path(&path)).unwrap();
        assert_eq!(backup.get_bool("use_opengl"), Some(false));
    }

    #[test]
    fn restore_backup_reverts_to_previous() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");

        Config::parse(LIVE_SAMPLE).unwrap().save(&path).unwrap();
        let mut edited = Config::load(&path).unwrap();
        edited.set_bool("use_opengl", true);
        edited.save(&path).unwrap();

        Config::restore_backup(&path).unwrap();
        let restored = Config::load(&path).unwrap();
        assert_eq!(
            restored.get_bool("use_opengl"),
            Some(false),
            "restore should revert to pre-edit state"
        );
    }

    #[test]
    fn string_value_with_double_slash_is_not_a_comment() {
        // A URL-like value must not be mangled by comment handling.
        let src = r#"{ "some_url": "https://roblox.com/x" }"#;
        let cfg = Config::parse(src).unwrap();
        assert_eq!(
            cfg.get("some_url").and_then(Value::as_str),
            Some("https://roblox.com/x")
        );
        let out = cfg.to_pretty_string();
        assert!(out.contains("https://roblox.com/x"), "URL corrupted:\n{out}");
    }

    #[test]
    fn rejects_non_object_root() {
        assert!(matches!(
            Config::parse("[1, 2, 3]"),
            Err(ConfigError::NotAnObject)
        ));
    }
}
