///Pigment: Yo bloxstrap leme copy your homework
///Bloxstrap: sure, just dont make it too obvious
///Pigment:
use std::rc::Rc;

use adw::prelude::*;
use pigment_core::{mods, presets, ApkAssetTree, ModLibrary, ProfileStore, Sober};

/// shared page context, cloned into row callbacks.
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

    // The authoritative asset list for validating mod paths
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

    let ctx = Rc::new(Ctx {
        lib,
        store,
        sober,
        tree,
        list,
        banner,
        status,
    });

    page.append(&presets_group(&ctx));
    page.append(&catalog_group(&ctx));
    page.append(&ctx.status);

    // Install from folder via a native folder picker.
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

/// lil suffix install button, same look everywhere
fn install_button(label: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(label)
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build()
}

/// the presets we ship in the binary + the pick-ur-own-font row
fn presets_group(ctx: &Rc<Ctx>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Preset Mods")
        .description("One-click mods that ship with Pigment. Installing adds them to your library above — flip the switch to enable.")
        .build();

    for preset in presets::BUNDLED {
        let row = adw::ActionRow::builder()
            .title(preset.name)
            .subtitle(preset.description)
            .build();
        let btn = install_button(if ctx.lib.contains(preset.id) { "Reinstall" } else { "Install" });
        {
            let ctx = ctx.clone();
            btn.connect_clicked(move |btn| {
                match presets::install_bundled(&ctx.lib, preset) {
                    Ok(name) => {
                        set_ok(&ctx.status, &format!("Installed “{name}”. Toggle it on to apply."));
                        btn.set_label("Reinstall");
                        populate(&ctx);
                    }
                    Err(e) => set_error(&ctx.status, &format!("Could not install preset: {e}")),
                }
            });
        }
        row.add_suffix(&btn);
        group.add(&row);
    }

    // custom font: user picks a ttf/otf, we clone it over every builder sans weight
    let row = adw::ActionRow::builder()
        .title("Custom UI Font")
        .subtitle("Replace the Roblox UI font (Builder Sans) with any TTF or OTF file")
        .build();
    let btn = install_button("Choose font…");
    {
        let ctx = ctx.clone();
        btn.connect_clicked(move |btn| {
            let filter = gtk::FileFilter::new();
            filter.set_name(Some("Fonts (TTF/OTF)"));
            filter.add_suffix("ttf");
            filter.add_suffix("otf");
            let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
            filters.append(&filter);
            let dialog = gtk::FileDialog::builder()
                .title("Choose a font")
                .modal(true)
                .filters(&filters)
                .build();
            let window = btn.root().and_downcast::<gtk::Window>();
            let ctx = ctx.clone();
            dialog.open(window.as_ref(), gtk::gio::Cancellable::NONE, move |res| {
                let Ok(file) = res else { return };
                let Some(path) = file.path() else { return };
                match presets::install_font(&ctx.lib, &path) {
                    Ok(name) => {
                        set_ok(&ctx.status, &format!("Installed “{name}”. Toggle it on to apply."));
                        populate(&ctx);
                    }
                    Err(e) => set_error(&ctx.status, &format!("Could not install font: {e}")),
                }
            });
        });
    }
    row.add_suffix(&btn);
    group.add(&row);

    group
}

/// stuff we fetch from the catalog json on github. network lives on worker threads,
/// results ride async channels back like everywhere else in this app
fn catalog_group(ctx: &Rc<Ctx>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Downloadable Presets")
        .description("Fetched from the Pigment catalog. Downloads are verified against pinned checksums before install.")
        .build();

    let placeholder = adw::ActionRow::builder()
        .title("Loading catalog…")
        .subtitle(presets::catalog_url())
        .build();
    group.add(&placeholder);

    let (tx, rx) = async_channel::unbounded::<Result<presets::Catalog, String>>();
    std::thread::spawn(move || {
        let _ = tx.send_blocking(presets::fetch_catalog(&presets::catalog_url()).map_err(|e| e.to_string()));
    });
    {
        let (group, ctx) = (group.clone(), ctx.clone());
        gtk::glib::spawn_future_local(async move {
            let Ok(res) = rx.recv().await else { return };
            group.remove(&placeholder);
            match res {
                Ok(catalog) if catalog.entries.is_empty() => {
                    group.add(&adw::ActionRow::builder().title("Catalog is empty").build());
                }
                Ok(catalog) => {
                    for entry in catalog.entries {
                        group.add(&catalog_row(&ctx, entry));
                    }
                }
                Err(e) => {
                    group.add(
                        &adw::ActionRow::builder()
                            .title("Catalog unavailable")
                            .subtitle(&e)
                            .build(),
                    );
                }
            }
        });
    }

    group
}

/// one downloadable preset row. install downloads on a worker thread so the ui dont freeze
fn catalog_row(ctx: &Rc<Ctx>, entry: presets::CatalogEntry) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(&entry.name)
        .subtitle(&entry.description)
        .build();
    let btn = install_button(if ctx.lib.contains(&entry.id) { "Reinstall" } else { "Install" });
    {
        let ctx = ctx.clone();
        btn.connect_clicked(move |btn| {
            btn.set_sensitive(false);
            btn.set_label("Downloading…");
            let (tx, rx) = async_channel::unbounded::<Result<String, String>>();
            let (lib, entry) = (ctx.lib.clone(), entry.clone());
            std::thread::spawn(move || {
                let _ = tx.send_blocking(
                    presets::install_remote(&lib, &entry).map_err(|e| e.to_string()),
                );
            });
            let (ctx, btn) = (ctx.clone(), btn.clone());
            gtk::glib::spawn_future_local(async move {
                let Ok(res) = rx.recv().await else { return };
                btn.set_sensitive(true);
                match res {
                    Ok(name) => {
                        set_ok(&ctx.status, &format!("Installed “{name}”. Toggle it on to apply."));
                        btn.set_label("Reinstall");
                        populate(&ctx);
                    }
                    Err(e) => {
                        btn.set_label("Install");
                        set_error(&ctx.status, &format!("Could not install: {e}"));
                    }
                }
            });
        });
    }
    row.add_suffix(&btn);
    row
}

/// rebuild the mod list and the conflict banner from current state.
fn populate(ctx: &Rc<Ctx>) {
    ctx.list.remove_all();

    let installed = ctx.lib.installed().unwrap_or_default();
    let enabled = active_enabled_mods(&ctx.store);

    // Conflict banner: files claimed by more than one enabled mod.
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

        // Remove button
        let remove = gtk::Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text("Uninstall mod")
            .css_classes(["flat"])
            .valign(gtk::Align::Center)
            .build();
        {
            let (ctx, name) = (ctx.clone(), m.name.clone());
            remove.connect_clicked(move |_| {
                // Disable first so it leaves the profile, then delete.
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

        // toggle.
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
                // Rebuild after the signal settles
                let ctx = ctx.clone();
                gtk::glib::idle_add_local_once(move || populate(&ctx));
            });
        }

        ctx.list.append(&row);
    }
}

/// Enable or disable a mod by editing the active profile's mod list and recomposing the overlay. Auto- reates a default profile if none is active.
fn set_enabled(ctx: &Rc<Ctx>, name: &str, enable: bool) -> Result<(), String> {
    let active = match ctx.store.active() {
        Some(a) => a,
        None if enable => {
            // Snapshot the current setup so applying it doesn wipe settings.
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

/// the mod names enabled in the active profile (empty if none active)
fn active_enabled_mods(store: &ProfileStore) -> Vec<String> {
    store
        .active()
        .and_then(|name| store.load(&name).ok())
        .map(|p| p.mods)
        .unwrap_or_default()
}

/// summary of the mod: file count and APK validation summary.
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

/// show or hide the conflict banner based on enabled mod file overlaps.
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
