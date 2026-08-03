use pigment_core::Sober;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sober = Sober::discover().ok_or("no $HOME")?;

    let Some(build) = sober.installed_build() else {
        println!("Sober is not installed.");
        return Ok(());
    };
    println!("version:  {}", build.label().unwrap_or_else(|| "?".into()));
    println!("build:    {}", build.build.as_deref().unwrap_or("?"));
    println!("commit:   {}", build.commit.as_deref().unwrap_or("?"));
    println!("origin:   {}", build.origin.as_deref().unwrap_or("?"));
    println!(
        "roblox:   {}",
        sober.roblox_version().unwrap_or_else(|| "?".into())
    );

    match sober.update_available() {
        Some(newer) => println!(
            "update:   yes -> {}",
            newer.build.or(newer.date).unwrap_or_else(|| "?".into())
        ),
        None => println!("update:   up to date (or remote unreachable)"),
    }
    Ok(())
}
