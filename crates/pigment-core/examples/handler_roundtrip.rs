use std::path::PathBuf;

use pigment_core::protocol;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let before = protocol::current_handler();
    println!("current handler:     {before:?}");

    let launch_exec = PathBuf::from("/usr/bin/pigment-launch"); // placeholder
    protocol::register(&launch_exec)?;
    println!("after register:      {:?}", protocol::current_handler());
    println!("pigment is handler:  {}", protocol::pigment_is_handler());

    protocol::restore_sober()?;
    println!("after restore:       {:?}", protocol::current_handler());


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
