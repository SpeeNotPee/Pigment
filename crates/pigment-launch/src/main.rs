use std::process::ExitCode;

use pigment_core::{launch_with_active_profile, protocol, PigmentPaths, Sober};

fn main() -> ExitCode {
    //args_os bc args() straight up panics on a non utf8 arg, n a handler that panics = dead link
    let arg = std::env::args_os().nth(1);

    if arg.as_deref() == Some(std::ffi::OsStr::new("--unregister")) {
        return unregister();
    }
    let uri = arg.map(|a| a.to_string_lossy().into_owned());

    let Some(sober) = Sober::discover() else {
        eprintln!("pigment-launch: cannot resolve $HOME; nothing to launch");
        return ExitCode::FAILURE;
    };

    //profile is best effort inside the helper, only the sober hand-off can fail us
    let outcome = launch_with_active_profile(&sober, uri.as_deref());
    if let Some(w) = &outcome.profile_warning {
        eprintln!("pigment-launch: {w}; launching anyway");
    }
    match outcome.child {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("pigment-launch: failed to launch Sober: {e}");
            ExitCode::FAILURE
        }
    }
}

/// give roblox:// back to whoever had it n delete my generated desktop file.
/// `make uninstall` n the aur .install hook call this so links dont point at a dead binary
fn unregister() -> ExitCode {
    let Some(paths) = PigmentPaths::discover() else {
        eprintln!("pigment-launch: cannot resolve $HOME");
        return ExitCode::FAILURE;
    };
    match protocol::restore_previous(&paths) {
        Ok(()) => {
            println!("pigment-launch: roblox:// handler restored, launcher entry removed");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("pigment-launch: could not unregister: {e}");
            ExitCode::FAILURE
        }
    }
}
