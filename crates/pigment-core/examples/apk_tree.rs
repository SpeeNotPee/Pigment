//! Index the real Sober `base.apk` on this machine and print summary stats.
//! Read-only. Usage: `cargo run -p pigment-core --example apk_tree`

use pigment_core::{ApkAssetTree, SoberPaths};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = SoberPaths::discover().ok_or("no $HOME")?;
    let apk = paths.base_apk();
    let tree = ApkAssetTree::read(&apk)?;
    println!("APK: {}", apk.display());
    println!("indexed asset files: {}", tree.len());

    let cursor = "content/textures/Cursors/KeyboardMouse/ArrowCursor.png";
    println!("contains documented cursor path ({cursor}): {}", tree.contains(cursor));

    println!("first 8 asset paths:");
    for p in tree.iter().take(8) {
        println!("  {p}");
    }
    Ok(())
}
