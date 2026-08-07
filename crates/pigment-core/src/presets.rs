//preset mods. some ship inside the binary, some get pulled from a lil json catalog
//so we dont have to commit copyrighted shit (looking at u, oof sound) into the repo

use std::io;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::library::ModLibrary;

/// one file inside a bundled preset. apk-relative path + the bytes baked into the binary
pub struct PresetFile {
    pub apk_path: &'static str,
    pub bytes: &'static [u8],
}

/// a preset we ship w/ pigment itself. all self-made art, nothing yoinked
pub struct BundledPreset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub files: &'static [PresetFile],
}

// the include_bytes paths r relative to THIS file, hence the ../../ crawl. ugly but true
macro_rules! cursor_preset {
    ($id:literal) => {
        &[
            PresetFile {
                apk_path: "content/textures/Cursors/KeyboardMouse/ArrowCursor.png",
                bytes: include_bytes!(concat!(
                    "../presets/",
                    $id,
                    "/content/textures/Cursors/KeyboardMouse/ArrowCursor.png"
                )),
            },
            PresetFile {
                apk_path: "content/textures/Cursors/KeyboardMouse/ArrowFarCursor.png",
                bytes: include_bytes!(concat!(
                    "../presets/",
                    $id,
                    "/content/textures/Cursors/KeyboardMouse/ArrowFarCursor.png"
                )),
            },
        ]
    };
}

/// every bundled preset, in display order
pub const BUNDLED: &[BundledPreset] = &[
    BundledPreset {
        id: "clean-cursor",
        name: "Clean Cursor",
        description: "A plain white dot cursor with a dark outline",
        files: cursor_preset!("clean-cursor"),
    },
    BundledPreset {
        id: "dot-cursor",
        name: "Dot Cursor",
        description: "A small black dot — minimal as it gets",
        files: cursor_preset!("dot-cursor"),
    },
    BundledPreset {
        id: "crosshair-cursor",
        name: "Crosshair Cursor",
        description: "A crosshair with an open center, handy for shooters",
        files: cursor_preset!("crosshair-cursor"),
    },
];

/// stick a bundled preset into the mod library. comes back as a bog standard mod
pub fn install_bundled(lib: &ModLibrary, preset: &BundledPreset) -> io::Result<String> {
    let files: Vec<(&str, &[u8])> = preset.files.iter().map(|f| (f.apk_path, f.bytes)).collect();
    lib.install_from_files(preset.id, &files)
}

/// builder sans is the default roblox ui font. swap every weight for the same file
/// n the whole ui changes. engine dont care that a ttf is wearing an otf name btw
pub const FONT_TARGETS: &[&str] = &[
    "content/fonts/BuilderSans-Regular.otf",
    "content/fonts/BuilderSans-Medium.otf",
    "content/fonts/BuilderSans-Bold.otf",
    "content/fonts/BuilderSans-ExtraBold.otf",
];

/// build a custom-font mod from a font file the user picked. name comes from the file stem
pub fn install_font(lib: &ModLibrary, font_path: &Path) -> io::Result<String> {
    let bytes = std::fs::read(font_path)?;
    // barely a font check but catches "oops i picked a png": sfnt/otf/ttc/woff magics
    let ok = matches!(
        bytes.get(..4),
        Some(b"\x00\x01\x00\x00") | Some(b"OTTO") | Some(b"true") | Some(b"ttcf")
    );
    if !ok {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "that file does not look like a TTF/OTF font",
        ));
    }
    let stem = font_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("custom");
    let name = format!("font-{stem}");
    let files: Vec<(&str, &[u8])> = FONT_TARGETS.iter().map(|t| (*t, bytes.as_slice())).collect();
    lib.install_from_files(&name, &files)
}

/// where the catalog json lives. override w/ PIGMENT_CATALOG_URL for testing
pub const DEFAULT_CATALOG_URL: &str =
    "https://raw.githubusercontent.com/SpeeNotPee/Pigment/main/catalog/presets.json";

/// the catalog url, respecting the env override
pub fn catalog_url() -> String {
    std::env::var("PIGMENT_CATALOG_URL").unwrap_or_else(|_| DEFAULT_CATALOG_URL.to_string())
}

#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    #[error("network error: {0}")]
    Http(String),
    #[error("could not parse catalog: {0}")]
    Parse(String),
    #[error("{path}: downloaded file hash mismatch (expected {expected}, got {actual}) — refusing to install")]
    BadHash {
        path: String,
        expected: String,
        actual: String,
    },
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// one downloadable file in a catalog entry. sha256 is mandatory, no hash no install
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogFile {
    pub apk_path: String,
    pub url: String,
    pub sha256: String,
}

/// one installable preset in the remote catalog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub files: Vec<CatalogFile>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Catalog {
    pub entries: Vec<CatalogEntry>,
}

fn agent() -> ureq::Agent {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build();
    config.into()
}

/// pull n parse the catalog. network so this belongs on a worker thread in the gui
pub fn fetch_catalog(url: &str) -> Result<Catalog, PresetError> {
    let body = agent()
        .get(url)
        .call()
        .map_err(|e| PresetError::Http(e.to_string()))?
        .body_mut()
        .read_to_string()
        .map_err(|e| PresetError::Http(e.to_string()))?;
    parse_catalog(&body)
}

/// split out from fetch so tests dont need a webserver
pub fn parse_catalog(json: &str) -> Result<Catalog, PresetError> {
    serde_json::from_str(json).map_err(|e| PresetError::Parse(e.to_string()))
}

/// download every file of a catalog entry, verify hashes, then install as a normal mod.
/// all-or-nothing: one bad file n nothing lands in the library
pub fn install_remote(lib: &ModLibrary, entry: &CatalogEntry) -> Result<String, PresetError> {
    let agent = agent();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for f in &entry.files {
        let bytes = agent
            .get(&f.url)
            .call()
            .map_err(|e| PresetError::Http(format!("{}: {e}", f.url)))?
            .body_mut()
            .read_to_vec()
            .map_err(|e| PresetError::Http(format!("{}: {e}", f.url)))?;
        verify_sha256(&f.apk_path, &bytes, &f.sha256)?;
        files.push((f.apk_path.clone(), bytes));
    }
    let borrowed: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_slice()))
        .collect();
    Ok(lib.install_from_files(&entry.id, &borrowed)?)
}

/// the "did the internet lie to us" check
fn verify_sha256(path: &str, bytes: &[u8], expected: &str) -> Result<(), PresetError> {
    let actual = hex(&Sha256::digest(bytes));
    if actual.eq_ignore_ascii_case(expected.trim()) {
        Ok(())
    } else {
        Err(PresetError::BadHash {
            path: path.to_string(),
            expected: expected.to_string(),
            actual,
        })
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::PigmentPaths;

    fn lib() -> (tempfile::TempDir, ModLibrary) {
        let dir = tempfile::tempdir().unwrap();
        let lib = ModLibrary::new(PigmentPaths::with_config_dir(dir.path()));
        (dir, lib)
    }

    #[test]
    fn bundled_presets_install_as_normal_mods() {
        let (_d, lib) = lib();
        for preset in BUNDLED {
            let name = install_bundled(&lib, preset).unwrap();
            assert_eq!(name, preset.id);
            let files = lib.get(&name).unwrap().files().unwrap();
            assert_eq!(files.len(), preset.files.len());
            // every baked path must be a real cursor path, no typos allowed
            for f in preset.files {
                assert!(files.contains(&f.apk_path.to_string()));
                assert!(!f.bytes.is_empty());
                // png magic, cuz an empty/broken include would still compile
                assert_eq!(&f.bytes[..8], b"\x89PNG\r\n\x1a\n");
            }
        }
    }

    #[test]
    fn font_install_covers_every_builder_sans_weight() {
        let (_d, lib) = lib();
        let dir = tempfile::tempdir().unwrap();
        let font = dir.path().join("Cool Font.ttf");
        // minimal sfnt magic so the sniff passes
        std::fs::write(&font, b"\x00\x01\x00\x00restoffontidc").unwrap();

        let name = install_font(&lib, &font).unwrap();
        assert_eq!(name, "font-Cool Font");
        let files = lib.get(&name).unwrap().files().unwrap();
        assert_eq!(files.len(), FONT_TARGETS.len());
        for t in FONT_TARGETS {
            assert!(files.contains(&t.to_string()));
        }
    }

    #[test]
    fn font_install_rejects_not_a_font() {
        let (_d, lib) = lib();
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("totally-a-font.ttf");
        std::fs::write(&fake, b"\x89PNG\r\n\x1a\nlol").unwrap();
        assert!(install_font(&lib, &fake).is_err());
    }

    #[test]
    fn catalog_parses_the_real_manifest_shape() {
        let json = r#"{
            "entries": [{
                "id": "old-death-sound",
                "name": "Old Death Sound",
                "description": "the oof",
                "files": [{
                    "apk_path": "content/sounds/oof.ogg",
                    "url": "https://example.com/ouch.ogg",
                    "sha256": "da23c3bc65272fcf50d56cc14d74037e85aee3f4ae1639dc7717b232bf37812a"
                }]
            }]
        }"#;
        let cat = parse_catalog(json).unwrap();
        assert_eq!(cat.entries.len(), 1);
        assert_eq!(cat.entries[0].files[0].apk_path, "content/sounds/oof.ogg");
        assert!(parse_catalog("not json").is_err());
    }

    #[test]
    fn sha256_gate_actually_gates() {
        assert!(verify_sha256(
            "x",
            b"hello",
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        )
        .is_ok());
        // case insensitive cuz ppl paste hashes in caps all the time
        assert!(verify_sha256(
            "x",
            b"hello",
            "2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824"
        )
        .is_ok());
        let err = verify_sha256("x", b"hello", "deadbeef").unwrap_err();
        assert!(matches!(err, PresetError::BadHash { .. }));
    }

    // hits the actual internet so its opt-in: cargo test -p pigment-core -- --ignored
    #[test]
    #[ignore = "network: downloads the real oof from github"]
    fn install_remote_downloads_verifies_and_installs() {
        let (_d, lib) = lib();
        let entry = CatalogEntry {
            id: "old-death-sound".into(),
            name: "Old Death Sound".into(),
            description: "the oof".into(),
            files: vec![CatalogFile {
                apk_path: "content/sounds/oof.ogg".into(),
                url: "https://raw.githubusercontent.com/OctaNebula/return-oof-sound/main/Resources/ouch.ogg".into(),
                sha256: "da23c3bc65272fcf50d56cc14d74037e85aee3f4ae1639dc7717b232bf37812a".into(),
            }],
        };
        let name = install_remote(&lib, &entry).unwrap();
        assert_eq!(name, "old-death-sound");
        let files = lib.get(&name).unwrap().files().unwrap();
        assert_eq!(files, vec!["content/sounds/oof.ogg".to_string()]);

        // n the tamper case: same url, wrong pin -> hard no, nothing installed
        let mut bad = entry.clone();
        bad.id = "evil-sound".into();
        bad.files[0].sha256 = "0".repeat(64);
        assert!(matches!(
            install_remote(&lib, &bad),
            Err(PresetError::BadHash { .. })
        ));
        assert!(!lib.contains("evil-sound"));
    }

    #[test]
    fn install_from_files_refuses_traversal() {
        let (_d, lib) = lib();
        let err = lib
            .install_from_files("evil", &[("../../escape.png", b"x".as_slice())])
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
        assert!(!lib.contains("evil"));
    }
}
