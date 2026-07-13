//! Mods page: install, inspect, and enable asset-overlay mods.
//!
//! A mod is a file tree mirroring Roblox's APK `assets/` layout. Installed mods
//! live in the library ([`ModLibrary`]); *enabling* one adds it to the active
//! profile's mod list and recomposes Sober's overlay, so the same set is applied
//! on launch. If no profile is active, enabling the first mod auto-creates and
//! activates a "Default" profile snapshotted from the current setup.
//!
//! Each mod is validated against the real APK asset tree so paths that Roblox
//! doesn't ship (typos, or Windows-client mods that don't match Android paths)
//! are flagged as unlikely to take effect.

use std::rc::Rc;

use adw::prelude::*;
use pigment_core::{mods, ApkAssetTree, ModLibrary, ProfileStore, Sober};

/// Shared page context, cloned into row callbacks.
struct Ctx {
    lib: ModLibrary,
    store: ProfileStore,
    sober: Sober,
    tree: Option<ApkAssetTree>,
    list: gtk::ListBox,
    banner: adw::Banner,
    status: gtk::Label,
}

/// Build the Mods page.
pub fn build() -> gtk::Widget {
    let (Some(lib), Some(store), Some(sober)) = (
        ModLibrary::discover(),
        ProfileStore::discover(),
        Sober::discover(),
    ) else {
        return error_page("Could not determine your configuration directories.");
    };

    // The authoritative asset list, for validating mod paths. Best-effort.
    let tree = ApkAssetTree::read(sober.paths().base_apk()).ok();

    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();

    let intro = gtk::Label::builder()
        .label("Mods replace Roblox assets through Sober's overlay. Enabled mods are applied to your active profile and take effect after you restart Roblox.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    page.append(&intro);

    let banner = adw::Banner::builder().build();
    page.append(&banner);

    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    page.append(&list);

    let install = gtk::Button::builder()
        .label("Install mod from folder…")
        .halign(gtk::Align::Start)
        .css_classes(["suggested-action"])
        .build();
    page.append(&install);

    let status = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    page.append(&status);

    let ctx = Rc::new(Ctx {
        lib,
        store,
        sober,
        tree,
        list,
        banner,
        status,
    });

    // Install-from-folder via a native folder picker.
    {
        let ctx = ctx.clone();
        install.connect_clicked(move |btn| {
            let dialog = gtk::FileDialog::builder()
                .title("Choose a mod folder")
                .modal(true)
                .build();
            let window = btn.root().and_downcast::<gtk::Window>();
            let ctx = ctx.clone();
            dialog.select_folder(window.as_ref(), gtk::gio::Cancellable::NONE, move |res| {
                let Ok(folder) = res else { return };
                let Some(path) = folder.path() else { return };
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("mod")
                    .to_string();
                match ctx.lib.install_from_dir(&name, &path) {
                    Ok(installed) => {
                        set_ok(&ctx.status, &format!("Installed “{installed}”. Toggle it on to apply."));
                        populate(&ctx);
                    }
                    Err(e) => set_error(&ctx.status, &format!("Could not install mod: {e}")),
                }
            });
        });
    }

    populate(&ctx);
    super::scrolled(&page).upcast()
}

/// Rebuild the mod list and the conflict banner from current state.
fn populate(ctx: &Rc<Ctx>) {
    ctx.list.remove_all();

    let installed = ctx.lib.installed().unwrap_or_default();
    let enabled = active_enabled_mods(&ctx.store);

    // Conflict banner: files claimed by more than one *enabled* mod.
    update_conflict_banner(ctx, &installed, &enabled);

    if installed.is_empty() {
        ctx.list.append(
            &adw::ActionRow::builder()
                .title("No mods installed")
                .subtitle("Install a folder that mirrors Roblox's asset layout")
                .build(),
        );
        return;
    }

    for m in installed {
        let is_on = enabled.iter().any(|n| n == &m.name);
        let row = adw::SwitchRow::builder().title(&m.name).active(is_on).build();
        row.set_subtitle(&describe_mod(&m, ctx.tree.as_ref()));

        // Remove button (prefix so it doesn't crowd the switch).
        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Uninstall mod")
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .build();
        {
            let (ctx, name) = (ctx.clone(), m.name.clone());
            remove.connect_clicked(move |_| {
                // Disable first so it leaves the profile/overlay, then delete.
                let _ = set_enabled(&ctx, &name, false);
                match ctx.lib.remove(&name) {
                    Ok(()) => set_ok(&ctx.status, &format!("Uninstalled “{name}”.")),
                    Err(e) => set_error(&ctx.status, &format!("Could not uninstall: {e}")),
                }
                let ctx = ctx.clone();
                gtk::glib::idle_add_local_once(move || populate(&ctx));
            });
        }
        row.add_prefix(&remove);

        // Enable/disable toggle.
        {
            let (ctx, name) = (ctx.clone(), m.name.clone());
            row.connect_active_notify(move |row| {
                let enable = row.is_active();
                match set_enabled(&ctx, &name, enable) {
                    Ok(()) => {
                        let verb = if enable { "Enabled" } else { "Disabled" };
                        set_ok(&ctx.status, &format!("{verb} “{name}”. Restart Roblox to apply."));
                    }
                    Err(e) => set_error(&ctx.status, &format!("Could not update mod: {e}")),
                }
                // Rebuild after the signal settles (refreshes conflicts + reverts
                // the switch if the operation failed).
                let ctx = ctx.clone();
                gtk::glib::idle_add_local_once(move || populate(&ctx));
            });
        }

        ctx.list.append(&row);
    }
}

/// Enable or disable a mod by editing the active profile's mod list and
/// recomposing the overlay. Auto-creates a "Default" profile if none is active.
fn set_enabled(ctx: &Rc<Ctx>, name: &str, enable: bool) -> Result<(), String> {
    let active = match ctx.store.active() {
        Some(a) => a,
        None if enable => {
            // Snapshot the current setup so applying it doesn't wipe settings.
            let profile = super::snapshot_profile("Default", &ctx.sober)?;
            ctx.store.save(&profile).map_err(|e| e.to_string())?;
            ctx.store.set_active(Some("Default")).map_err(|e| e.to_string())?;
            "Default".to_string()
        }
        None => return Ok(()), // disabling with no profile: nothing to do
    };

    let mut profile = ctx.store.load(&active).map_err(|e| e.to_string())?;
    let present = profile.mods.iter().any(|n| n == name);
    if enable && !present {
        profile.mods.push(name.to_string());
    } else if !enable && present {
        profile.mods.retain(|n| n != name);
    } else {
        return Ok(()); // already in the desired state
    }
    ctx.store.save(&profile).map_err(|e| e.to_string())?;
    ctx.store
        .apply(&profile, ctx.sober.paths())
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// The mod names enabled in the active profile (empty if none active).
fn active_enabled_mods(store: &ProfileStore) -> Vec<String> {
    store
        .active()
        .and_then(|name| store.load(&name).ok())
        .map(|p| p.mods)
        .unwrap_or_default()
}

/// A one-line description of a mod: file count and APK-validation summary.
fn describe_mod(m: &pigment_core::ModSource, tree: Option<&ApkAssetTree>) -> String {
    let files = m.files().map(|f| f.len()).unwrap_or(0);
    let mut s = format!("{files} file{}", if files == 1 { "" } else { "s" });
    if let Some(tree) = tree {
        if let Ok(unknown) = m.unknown_paths(tree) {
            let valid = files.saturating_sub(unknown.len());
            s.push_str(&format!(" · {valid} match Roblox assets"));
            if !unknown.is_empty() {
                s.push_str(&format!(" · {} unrecognized (won't apply)", unknown.len()));
            }
        }
    }
    s
}

/// Show or hide the conflict banner based on enabled-mod file overlaps.
fn update_conflict_banner(ctx: &Rc<Ctx>, installed: &[pigment_core::ModSource], enabled: &[String]) {
    let enabled_sources: Vec<_> = installed
        .iter()
        .filter(|m| enabled.iter().any(|n| n == &m.name))
        .cloned()
        .collect();
    match mods::detect_conflicts(&enabled_sources) {
        Ok(conflicts) if !conflicts.is_empty() => {
            let n = conflicts.len();
            let last = conflicts
                .last()
                .and_then(|c| c.mods.last())
                .cloned()
                .unwrap_or_default();
            ctx.banner.set_title(&format!(
                "{n} file{} claimed by multiple mods — “{last}” wins each.",
                if n == 1 { "" } else { "s" }
            ));
            ctx.banner.set_revealed(true);
        }
        _ => ctx.banner.set_revealed(false),
    }
}

fn set_error(label: &gtk::Label, msg: &str) {
    label.set_css_classes(&["error"]);
    label.set_text(msg);
}

fn set_ok(label: &gtk::Label, msg: &str) {
    label.set_css_classes(&["success"]);
    label.set_text(msg);
}

fn error_page(msg: &str) -> gtk::Widget {
    adw::StatusPage::builder()
        .icon_name("dialog-error-symbolic")
        .title("Mods unavailable")
        .description(msg)
        .build()
        .upcast()
}
