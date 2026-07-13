//! Read the real Sober config on this machine, round-trip it through
//! `pigment_core::Config`, and print the result to stdout. Read-only: never
//! writes to the live config. Used to verify preservation against reality.
//!
//! Usage: `cargo run -p pigment-core --example roundtrip_live`

use pigment_core::{Config, SoberPaths};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = SoberPaths::discover().ok_or("no $HOME")?;
    let cfg = Config::load(paths.config_file())?;
    print!("{}", cfg.to_pretty_string());
    Ok(())
}
