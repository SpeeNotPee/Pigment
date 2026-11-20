//! LIVE test: really flips ur roblox:// handler n puts it back. opt in only:
//!   PIGMENT_TOUCH_REAL_HANDLER=1 cargo run -p pigment-core --example handler_roundtrip
//! pigment state goes in a temp dir so ur real profiles/state r never touched
use std::path::PathBuf;

use pigment_core::protocol::{self, ROBLOX_SCHEMES};
use pigment_core::PigmentPaths;

fn handlers() -> Vec<Option<String>> {
    ROBLOX_SCHEMES
        .iter()
        .map(|s| protocol::current_handler(s))
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("PIGMENT_TOUCH_REAL_HANDLER").is_none() {
        eprintln!(
            "this changes your real roblox:// handler. set PIGMENT_TOUCH_REAL_HANDLER=1 to run it."
        );
        std::process::exit(2);
    }
    let tmp = tempfile::tempdir()?;
    let paths = PigmentPaths::with_config_dir(tmp.path());

    let before = handlers();
    println!("handlers before:     {before:?}");

    let launch_exec = PathBuf::from("/usr/bin/pigment-launch"); // placeholder
    protocol::register(&launch_exec, &paths)?;
    println!("after register:      {:?}", handlers());
    println!("pigment is handler:  {}", protocol::pigment_is_handler());

    //restores whatever it read at the start, not blindly sober
    protocol::restore_previous(&paths)?;
    let after = handlers();
    println!("after restore:       {after:?}");
    let leftover = protocol::launcher_desktop_path().is_some_and(|p| p.exists());
    assert!(!leftover, "generated desktop file was left behind");
    assert_eq!(
        before, after,
        "handler was not restored to its original value"
    );
    println!("\nOK - handler round-tripped and system left pristine.");
    Ok(())
}
