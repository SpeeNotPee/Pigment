//! Live, reversible test of protocol-handler registration.
//!
//! Snapshots the current `roblox://` handler, registers Pigment, verifies the
//! switch, then restores the original handler and removes the desktop file it
//! wrote — leaving the system exactly as found.
//!
//! Usage: `cargo run -p pigment-core --example handler_roundtrip`

use std::path::PathBuf;

use pigment_core::protocol;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let before = protocol::current_handler();
    println!("current handler:     {before:?}");

    let launch_exec = PathBuf::from("/usr/bin/pigment-launch"); // placeholder; removed below
    protocol::register(&launch_exec)?;
    println!("after register:      {:?}", protocol::current_handler());
    println!("pigment is handler:  {}", protocol::pigment_is_handler());

    // Restore whatever was there before (Sober in practice).
    protocol::restore_sober()?;
    println!("after restore:       {:?}", protocol::current_handler());

    // Clean up the desktop file we wrote so no trace remains.
    if let Some(dir) = protocol::user_applications_dir() {
        let f = dir.join(protocol::PIGMENT_DESKTOP);
        if f.exists() {
            std::fs::remove_file(&f)?;
            println!("removed test desktop file: {}", f.display());
        }
    }

    let after = protocol::current_handler();
    println!("final handler:       {after:?}");
    assert_eq!(before, after, "handler was not restored to its original value");
    println!("\nOK — handler round-tripped and system left pristine.");
    Ok(())
}
