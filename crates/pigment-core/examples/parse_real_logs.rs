//! Parse every real Sober log on this machine and print the sessions found.
//! Read-only. Usage: `cargo run -p pigment-core --example parse_real_logs`

use pigment_core::{activity, SoberPaths};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = SoberPaths::discover().ok_or("no $HOME")?;
    let log_dir = paths.latest_log().parent().unwrap().to_path_buf();

    let mut total = 0;
    for entry in std::fs::read_dir(&log_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("log") {
            continue;
        }
        let sessions = activity::sessions_from_file(&path)?;
        if sessions.is_empty() {
            continue;
        }
        println!("{}: {} session(s)", path.file_name().unwrap().to_string_lossy(), sessions.len());
        for s in &sessions {
            let when = s
                .joined_at
                .map(|t| t.to_string())
                .unwrap_or_else(|| "?".into());
            let dur = s
                .duration()
                .map(|d| format!("{}m{}s", d.whole_minutes(), d.whole_seconds() % 60))
                .unwrap_or_else(|| if s.is_active() { "active".into() } else { "?".into() });
            println!(
                "  place {} universe {:?}  joined {}  [{}]",
                s.place_id, s.universe_id, when, dur
            );
            total += 1;
        }
    }
    println!("\nTotal sessions parsed: {total}");
    Ok(())
}
