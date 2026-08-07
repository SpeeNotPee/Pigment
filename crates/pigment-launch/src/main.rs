use std::process::ExitCode;

use pigment_core::{ProfileStore, Sober};

fn main() -> ExitCode {
    let uri = std::env::args().nth(1);

    let Some(sober) = Sober::discover() else {
        eprintln!("pigment-launch: cannot resolve $HOME; nothing to launch");
        return ExitCode::FAILURE;
    };

    // Best effort profile application. Failures here must not block the launch.
    apply_active_profile(&sober);

    // Hand off to Sober. This is the step that must succeed.
    match sober.launch(uri.as_deref()) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pigment-launch: failed to launch Sober: {e}");
            ExitCode::FAILURE
        }
    }
}

fn apply_active_profile(sober: &Sober) {
    let Some(store) = ProfileStore::discover() else {
        return;
    };
    // core does the actual work now so the gui join buttons cant drift from this path
    match store.apply_active(sober.paths()) {
        Ok(Some(report)) if !report.missing_mods.is_empty() => {
            eprintln!(
                "pigment-launch: active profile references missing mods: {}",
                report.missing_mods.join(", ")
            );
        }
        // None = no active profile, launch w/ config untouched
        Ok(_) => {}
        Err(e) => {
            eprintln!("pigment-launch: could not apply active profile: {e}; launching anyway");
        }
    }
}
