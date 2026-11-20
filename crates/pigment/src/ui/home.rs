//homepage, nothing too fancy

use std::cell::Cell;
use std::rc::Rc;

use adw::prelude::*;
use pigment_core::{protocol, PigmentPaths, Sober};

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

    // Title block, using tPigment app icon, falls back to a generic icon when running uninstall
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

    // runtime status group.
    let build = sober.as_ref().and_then(|s| s.installed_build());
    let installed = build.is_some();
    let has_config = sober.as_ref().map(|s| s.has_config()).unwrap_or(false);

    let group = adw::PreferencesGroup::builder()
        .title("Sober Runtime")
        .build();
    group.add(&status_row(
        "Installed",
        &match build.as_ref().and_then(|b| b.label()) {
            Some(label) => format!("Yes — {label}"),
            None if installed => "Yes".into(),
            None => "Not found".into(),
        },
        installed,
    ));
    // some update checks
    if let Some(client) = sober.as_ref().and_then(|s| s.roblox_version()) {
        group.add(&status_row("Roblox client", &client, true));
    }
    group.add(&status_row(
        "Configuration",
        if has_config {
            "Present"
        } else {
            "Not yet created"
        },
        has_config,
    ));

    // if not installed
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

    // luanch button
    let launch = gtk::Button::builder()
        .label("Launch Roblox")
        .halign(gtk::Align::Center)
        .css_classes(["suggested-action", "pill"])
        .sensitive(installed)
        .build();
    //shows what went wrong right under the button. "see terminal" is useless from a desktop launch
    let launch_status = gtk::Label::builder()
        .wrap(true)
        .justify(gtk::Justification::Center)
        .css_classes(["dim-label"])
        .build();
    if let Some(sober) = sober.clone() {
        let launch_status = launch_status.clone();
        launch.connect_clicked(move |_| match super::launch(&sober, None) {
            Ok(None) => {
                launch_status.set_css_classes(&["dim-label"]);
                launch_status.set_text("Launching Roblox…");
            }
            Ok(Some(warn)) => {
                launch_status.set_css_classes(&["warning"]);
                launch_status.set_text(&format!("Launching, but {warn}."));
            }
            Err(e) => {
                launch_status.set_css_classes(&["error"]);
                launch_status.set_text(&e);
            }
        });
    }

    container.append(&group);

    // stale refresh warning, filled in from a background check.
    if let Some(sober) = sober.clone() {
        container.append(&update_banner(sober));
    }

    if installed {
        container.append(&default_launcher_group());
    }

    container.append(&launch);
    container.append(&launch_status);

    let clamp = adw::Clamp::builder()
        .maximum_size(560)
        .child(&container)
        .build();
    super::scrolled(&clamp).upcast()
}

//update banner - self explanatory
fn update_banner(sober: Sober) -> adw::Banner {
    let banner = adw::Banner::builder().revealed(false).build();

    let (tx, rx) = async_channel::bounded::<String>(1);
    std::thread::spawn(move || {
        if sober.update_available().is_some() {
            let _ =
                tx.send_blocking("Update Sober: flatpak update org.vinegarhq.Sober".to_string());
        }
    });

    let banner_weak = banner.clone();
    gtk::glib::spawn_future_local(async move {
        if let Ok(msg) = rx.recv().await {
            banner_weak.set_title(&msg);
            banner_weak.set_revealed(true);
        }
    });

    banner
}

/// option to make pigment the default launcher
fn default_launcher_group() -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Default Launcher")
        .description("Let pigment be default launcher. You can switch back anytime :)")
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
        let Some(paths) = PigmentPaths::discover() else {
            row.set_subtitle("Could not resolve your config directory");
            revert(&guard2, &row_weak, !enable);
            return;
        };
        let exec = if enable {
            match super::launch_binary_path() {
                Some(exec) => Some(exec),
                None => {
                    row.set_subtitle("Could not locate the pigment-launch binary");
                    revert(&guard2, &row_weak, !enable);
                    return;
                }
            }
        } else {
            None
        };

        //xdg-mime + kbuildsycoca take secs on kde, so off the main thread it goes
        row.set_sensitive(false);
        row.set_subtitle("Updating…");
        let (tx, rx) = async_channel::bounded::<Result<(), String>>(1);
        std::thread::spawn(move || {
            let res = match exec {
                Some(exec) => protocol::register(&exec, &paths),
                None => protocol::restore_previous(&paths),
            };
            let _ = tx.send_blocking(res.map_err(|e| e.to_string()));
        });
        let (row, guard, row_weak) = (row.clone(), guard2.clone(), row_weak.clone());
        gtk::glib::spawn_future_local(async move {
            let res = rx
                .recv()
                .await
                .unwrap_or_else(|_| Err("worker died".into()));
            row.set_sensitive(true);
            match res {
                Ok(()) => row.set_subtitle(if enable {
                    "Pigment now handles roblox:// links"
                } else {
                    "Your previous roblox:// handler is back"
                }),
                Err(e) => {
                    row.set_subtitle(&format!("Could not change handler: {e}"));
                    revert(&guard, &row_weak, !enable);
                }
            }
        });
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
    let row = adw::ActionRow::builder()
        .title(title)
        .subtitle(value)
        .build();
    //NOT emblem-ok-symbolic, adwaita 47+ yeeted it (arch + debian 13 show a broken img).
    //object-select is the same checkmark n exists on every adwaita we support
    let icon = gtk::Image::from_icon_name(if ok {
        "object-select-symbolic"
    } else {
        "dialog-warning-symbolic"
    });
    row.add_suffix(&icon);
    row
}
