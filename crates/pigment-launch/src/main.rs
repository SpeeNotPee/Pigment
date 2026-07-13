//! `pigment-launch` — the latency-critical protocol-handler entry point.
//!
//! Registered as the handler for `roblox:`/`roblox-player:` URIs (opt-in, via the
//! GUI). When the browser's "Play" button fires, this binary:
//!
//! 1. applies the active Pigment profile onto Sober (settings, FastFlags, mods),
//! 2. hands the URI to Sober to launch the game.
//!
//! It links no GUI toolkit, so there is no window-system startup cost on this hot
//! path. Its cardinal rule is **never strand the player**: if applying the
//! profile fails for any reason, it logs the problem and launches Sober anyway.

use std::process::ExitCode;

use pigment_core::{ProfileStore, Sober};

fn main() -> ExitCode {
    let uri = std::env::args().nth(1);

    let Some(sober) = Sober::discover() else {
        eprintln!("pigment-launch: cannot resolve $HOME; nothing to launch");
        return ExitCode::FAILURE;
    };

    // Best-effort profile application. Failures here must not block the launch.
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

/// Apply the active profile if there is one. Purely best-effort: every failure is
/// logged and swallowed so the subsequent launch still happens.
fn apply_active_profile(sober: &Sober) {
    let Some(store) = ProfileStore::discover() else {
        return;
    };
    let Some(active) = store.active() else {
        // No active profile: launch Sober with its config untouched.
        return;
    };
    let profile = match store.load(&active) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("pigment-launch: could not load profile {active:?}: {e}");
            return;
        }
    };
    match store.apply(&profile, sober.paths()) {
        Ok(report) => {
            if !report.missing_mods.is_empty() {
                eprintln!(
                    "pigment-launch: profile {active:?} references missing mods: {}",
                    report.missing_mods.join(", ")
                );
            }
        }
        Err(e) => {
            eprintln!("pigment-launch: could not apply profile {active:?}: {e}; launching anyway");
        }
    }
}
