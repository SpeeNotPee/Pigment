use pigment_core::{Config, SoberPaths};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = SoberPaths::discover().ok_or("no $HOME")?;
    let cfg = Config::load(paths.config_file())?;
    print!("{}", cfg.to_pretty_string());
    Ok(())
}
