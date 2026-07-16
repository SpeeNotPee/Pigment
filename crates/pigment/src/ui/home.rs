//! Home page: Sober runtime status, the default-launcher toggle, and launch.

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use pigment_core::{protocol, Sober};

/// Build the Home page.
pub fn build() -> gtk::Widget {
    let sober = Sober::discover();

    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(24)
        .margin_top(36)
        .margin_bottom(36)
        .margin_start(12)
        .margin_end(12)
        .build();

    // Title block. Uses the installed Pigment app icon; falls back gracefully to
    // a generic icon when running uninstalled.
    let icon = gtk::Image::builder()
        .icon_name("net.pigmentlab.Pigment")
        .pixel_size(96)
        .build();
    let heading = gtk::Label::builder()
        .label("Pigment")
        .css_classes(["title-1"])
        .build();
    let subtitle = gtk::Label::builder()
        .label("Roblox on Linux, via Sober")
        .css_classes(["dim-label"])
        .build();
    container.append(&icon);
    container.append(&heading);
    container.append(&subtitle);

    // Runtime status group.
    let installed = sober.as_ref().map(|s| s.is_installed()).unwrap_or(false);
    let version = sober.as_ref().and_then(|s| s.installed_version());
    let has_config = sober.as_ref().map(|s| s.has_config()).unwrap_or(false);

    let group = adw::PreferencesGroup::builder().title("Sober Runtime").build();
    group.add(&status_row(
        "Installed",
        &match (&installed, &version) {
            (true, Some(v)) => format!("Yes — {v}"),
            (true, None) => "Yes".into(),
            (false, _) => "Not found".into(),
        },
        installed,
    ));
    group.add(&status_row(
        "Configuration",
        if has_config { "Present" } else { "Not yet created" },
        has_config,
    ));

    // Not-installed guidance.
    if !installed {
        let banner = adw::Banner::builder()
            .title("Sober is not installed. Install it, then relaunch Pigment.")
            .revealed(true)
            .build();
        container.append(&banner);

        let cmd = gtk::Label::builder()
            .label("flatpak install flathub org.vinegarhq.Sober")
            .selectable(true)
            .css_classes(["monospace", "dim-label"])
            .build();
        container.append(&cmd);
    }

    // Launch button.
    let launch = gtk::Button::builder()
        .label("Launch Roblox")
        .halign(gtk::Align::Center)
        .css_classes(["suggested-action", "pill"])
        .sensitive(installed)
        .build();
    if let Some(sober) = sober.clone() {
        launch.connect_clicked(move |btn| match sober.launch(None) {
            Ok(_) => super::note("launched Sober"),
            Err(e) => {
                btn.set_label("Launch failed — see terminal");
                eprintln!("pigment: failed to launch Sober: {e}");
            }
        });
    }

    container.append(&group);

    // Opt-in: become the system roblox:// handler. Only meaningful with Sober
    // present; the switch reflects and controls the real system association.
    if installed {
        container.append(&default_launcher_group());
    }

    container.append(&launch);

    let clamp = adw::Clamp::builder()
        .maximum_size(560)
        .child(&container)
        .build();
    super::scrolled(&clamp).upcast()
}

/// The opt-in "make Pigment the default launcher" control.
///
/// The switch reflects the live system handler and drives
/// [`protocol::register`] / [`protocol::restore_sober`]. On failure it reverts
/// itself (guarded against re-entrancy) and reports the reason inline. Nothing
/// happens until the user flips it — takeover is never automatic.
fn default_launcher_group() -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Default Launcher")
        .description("Let Pigment intercept Roblox links to apply your profile, then hand off to Sober. You can switch back anytime.")
        .build();

    let row = adw::SwitchRow::builder()
        .title("Make Pigment the default launcher")
        .subtitle("Handles roblox:// links")
        .active(protocol::pigment_is_handler())
        .build();

    // Suppresses the change signal while we revert programmatically.
    let guard = Rc::new(Cell::new(false));
    let guard2 = guard.clone();
    let row_weak = row.clone();
    row.connect_active_notify(move |row| {
        if guard2.get() {
            return;
        }
        let enable = row.is_active();
        let result = if enable {
            match super::launch_binary_path() {
                Some(exec) => protocol::register(&exec),
                None => {
                    row.set_subtitle("Could not locate the pigment-launch binary");
                    revert(&guard2, &row_weak, !enable);
                    return;
                }
            }
        } else {
            protocol::restore_sober()
        };

        match result {
            Ok(()) => row.set_subtitle(if enable {
                "Pigment now handles roblox:// links"
            } else {
                "Sober handles roblox:// links again"
            }),
            Err(e) => {
                row.set_subtitle(&format!("Could not change handler: {e}"));
                revert(&guard2, &row_weak, !enable);
            }
        }
    });

    group.add(&row);
    group
}

/// Flip a switch back to `to` without re-triggering its change handler.
fn revert(guard: &Rc<Cell<bool>>, row: &adw::SwitchRow, to: bool) {
    guard.set(true);
    row.set_active(to);
    guard.set(false);
}

/// A status row with a check/cross suffix icon reflecting a boolean.
fn status_row(title: &str, value: &str, ok: bool) -> adw::ActionRow {
    let row = adw::ActionRow::builder().title(title).subtitle(value).build();
    let icon = gtk::Image::from_icon_name(if ok {
        "emblem-ok-symbolic"
    } else {
        "dialog-warning-symbolic"
    });
    row.add_suffix(&icon);
    row
}
