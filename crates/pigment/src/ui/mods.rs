///Pigment: Yo bloxstrap leme copy your homework
///Bloxstrap: sure, just dont make it too obvious
///Pigment:
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::SystemTime;

use adw::prelude::*;
use pigment_core::{mods, presets, ApkAssetTree, ModLibrary, ProfileStore, Sober};

type CatalogResult = Result<presets::Catalog, String>;
/// (apk mtime when read, the index if it parsed)
type CachedTree = (Option<SystemTime>, Option<Arc<ApkAssetTree>>);

thread_local! {
    //the apk index, keyed by the apk mtime so a sober/roblox update invalidates it.
    //this page rebuilds every time its shown so i really dont wanna reread 100MB each time
    static TREE: RefCell<Option<CachedTree>> = const { RefCell::new(None) };
    //catalog gets fetched ONCE per run, the first time this page is opened. not at startup
    static CATALOG: RefCell<Option<CatalogResult>> = const { RefCell::new(None) };
}

/// shared page context, cloned into row callbacks.
struct Ctx {
    lib: ModLibrary,
    store: ProfileStore,
    sober: Sober,
    tree: RefCell<Option<Arc<ApkAssetTree>>>,
    list: gtk::ListBox,
    banner: adw::Banner,
    status: gtk::Label,
}

fn apk_mtime(sober: &Sober) -> Option<SystemTime> {
    std::fs::metadata(sober.paths().base_apk())
        .and_then(|m| m.modified())
        .ok()
}

/// cached tree if the apk didnt change, else read it on a worker n call `done` w/ it
fn with_tree(sober: &Sober, done: impl FnOnce(Option<Arc<ApkAssetTree>>) + 'static) {
    let mtime = apk_mtime(sober);
    let cached = TREE.with(|t| {
        t.borrow()
            .as_ref()
            .filter(|(m, _)| *m == mtime)
            .map(|(_, tree)| tree.clone())
    });
    if let Some(tree) = cached {
        return done(tree);
    }
    let apk = sober.paths().base_apk();
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(ApkAssetTree::read(apk).ok().map(Arc::new));
    });
    gtk::glib::spawn_future_local(async move {
        let tree = rx.recv().await.ok().flatten();
        TREE.with(|t| *t.borrow_mut() = Some((mtime, tree.clone())));
        done(tree);
    });
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
        tree: RefCell::new(None),
        list,
        banner,
        status,
    });

    page.append(&presets_group(&ctx));
    let catalog = catalog_group(&ctx);
    page.append(&catalog);
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
                install_folder(&ctx, path);
            });
        });
    }

    //list shows right away, file match counts fill in once the apk index is ready
    populate(&ctx);
    {
        let (ctx, catalog) = (ctx.clone(), catalog.clone());
        with_tree(&ctx.sober.clone(), move |tree| {
            *ctx.tree.borrow_mut() = tree;
            populate(&ctx);
            fill_catalog(&ctx, &catalog);
        });
    }
    super::scrolled(&page).upcast()
}

/// "install from folder" w/ all the guardrails: refuse ~ and / instantly, ask if nothing in
/// it looks like a roblox asset, then copy on a worker so the window dont freeze
fn install_folder(ctx: &Rc<Ctx>, path: std::path::PathBuf) {
    if let Err(e) = ctx.lib.check_source(&path) {
        return set_error(&ctx.status, &format!("Can't install that folder: {e}"));
    }
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("mod")
        .to_string();
    let do_copy = {
        let (ctx, path, name) = (ctx.clone(), path.clone(), name.clone());
        move || {
            set_ok(&ctx.status, &format!("Installing “{name}”…"));
            let (tx, rx) = async_channel::bounded::<Result<String, String>>(1);
            let (lib, path, name) = (ctx.lib.clone(), path.clone(), name.clone());
            std::thread::spawn(move || {
                let _ = tx.send_blocking(
                    lib.install_from_dir(&name, &path)
                        .map_err(|e| e.to_string()),
                );
            });
            let ctx = ctx.clone();
            gtk::glib::spawn_future_local(async move {
                match rx
                    .recv()
                    .await
                    .unwrap_or_else(|_| Err("worker died".into()))
                {
                    Ok(installed) => {
                        set_ok(
                            &ctx.status,
                            &format!("Installed “{installed}”. Toggle it on to apply."),
                        );
                        populate(&ctx);
                    }
                    Err(e) => set_error(&ctx.status, &format!("Could not install mod: {e}")),
                }
            });
        }
    };
    //quick sanity peek: does ANY file in there match the apk? scan is capped so its fast
    let tree = ctx.tree.borrow().clone();
    let looks_like_a_mod = match &tree {
        Some(tree) => {
            let probe = pigment_core::ModSource::new(&name, &path);
            match probe.files() {
                Ok(files) if files.len() <= pigment_core::library::MAX_MOD_FILES => {
                    files.iter().any(|f| tree.contains(f))
                }
                //too big or unreadable: let install_from_dir say why
                _ => true,
            }
        }
        None => true,
    };
    if looks_like_a_mod {
        do_copy();
    } else {
        super::confirm(
            &ctx.list,
            "Install anyway?",
            "Nothing in that folder matches a Roblox asset path for this Roblox version, so it probably won't change anything. A mod folder should mirror the game's assets, e.g. content/textures/…",
            "Install",
            do_copy,
        );
    }
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
        let btn = install_button(if ctx.lib.contains(preset.id) {
            "Reinstall"
        } else {
            "Install"
        });
        {
            let ctx = ctx.clone();
            btn.connect_clicked(
                move |btn| match presets::install_bundled(&ctx.lib, preset) {
                    Ok(name) => {
                        set_ok(
                            &ctx.status,
                            &format!("Installed “{name}”. Toggle it on to apply."),
                        );
                        btn.set_label("Reinstall");
                        populate(&ctx);
                    }
                    Err(e) => set_error(&ctx.status, &format!("Could not install preset: {e}")),
                },
            );
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
                        set_ok(
                            &ctx.status,
                            &format!("Installed “{name}”. Toggle it on to apply."),
                        );
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
fn catalog_group(_ctx: &Rc<Ctx>) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Downloadable Presets")
        .description("Fetched from the Pigment catalog on GitHub when you open this page. Downloads are verified against pinned checksums before install.")
        .build();
    group.add(
        &adw::ActionRow::builder()
            .title("Loading catalog…")
            .subtitle(presets::catalog_url())
            .build(),
    );
    group
}

/// swap the placeholder for real rows. fetches the first time only, then reuses the result
fn fill_catalog(ctx: &Rc<Ctx>, group: &adw::PreferencesGroup) {
    let cached = CATALOG.with(|c| c.borrow().clone());
    let (ctx, group) = (ctx.clone(), group.clone());
    let render = move |res: CatalogResult| {
        //drop the placeholder: its the only row so far
        while let Some(row) = first_row(&group) {
            group.remove(&row);
        }
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
    };
    if let Some(res) = cached {
        return render(res);
    }
    let (tx, rx) = async_channel::bounded::<CatalogResult>(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(
            presets::fetch_catalog(&presets::catalog_url()).map_err(|e| e.to_string()),
        );
    });
    gtk::glib::spawn_future_local(async move {
        let Ok(res) = rx.recv().await else { return };
        //only cache a success, a flaky network shouldnt stick for the whole run
        if res.is_ok() {
            CATALOG.with(|c| *c.borrow_mut() = Some(res.clone()));
        }
        render(res);
    });
}

/// PreferencesGroup dont expose its rows, so dig the first ActionRow outta the listbox
fn first_row(group: &adw::PreferencesGroup) -> Option<gtk::Widget> {
    fn find(w: &gtk::Widget) -> Option<gtk::Widget> {
        if w.is::<adw::ActionRow>() {
            return Some(w.clone());
        }
        let mut child = w.first_child();
        while let Some(c) = child {
            if let Some(found) = find(&c) {
                return Some(found);
            }
            child = c.next_sibling();
        }
        None
    }
    find(group.upcast_ref())
}

/// one downloadable preset row. install downloads on a worker thread so the ui dont freeze
fn catalog_row(ctx: &Rc<Ctx>, entry: presets::CatalogEntry) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(&entry.name)
        .subtitle(&entry.description)
        .build();
    //roblox 2.745 (sober 1.8) dropped a ton of assets from the apk. if this preset targets
    //files this roblox ver dont ship, say so up front instead of a silent no-op
    if let Some(tree) = ctx.tree.borrow().as_ref() {
        let missing = presets::missing_targets(&entry, tree);
        if !missing.is_empty() {
            row.set_subtitle(&format!(
                "{} — ⚠ this Roblox version doesn't ship {}, so it may have no effect",
                entry.description,
                missing.join(", ")
            ));
        }
    }
    let btn = install_button(if ctx.lib.contains(&entry.id) {
        "Reinstall"
    } else {
        "Install"
    });
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
                        set_ok(
                            &ctx.status,
                            &format!("Installed “{name}”. Toggle it on to apply."),
                        );
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
        let row = adw::SwitchRow::builder()
            .title(&m.name)
            .active(is_on)
            .build();
        row.set_subtitle(&describe_mod(&m, ctx.tree.borrow().as_deref()));

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
                    Ok(notes) => {
                        let verb = if enable { "Enabled" } else { "Disabled" };
                        let mut msg = format!("{verb} “{name}”. Restart Roblox to apply.");
                        for n in notes {
                            msg.push(' ');
                            msg.push_str(&n);
                        }
                        set_ok(&ctx.status, &msg);
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

/// Enable or disable a mod by editing the active profile's mod list and recomposing the
/// overlay. auto creates "Default" if nothing is active, n now actually TELLS u it did.
/// returns extra notes for the status line
fn set_enabled(ctx: &Rc<Ctx>, name: &str, enable: bool) -> Result<Vec<String>, String> {
    let mut notes = Vec::new();
    if ctx.store.active().is_none() {
        if !enable {
            return Ok(notes); // disabling with no profile: nothing to do
        }
        //snapshot the current setup so applying it dont wipe settings
        let profile = super::snapshot_profile("Default", &ctx.sober)?;
        if !ctx.store.exists("Default") {
            ctx.store.save(&profile).map_err(|e| e.to_string())?;
        }
        ctx.store
            .set_active(Some("Default"))
            .map_err(|e| e.to_string())?;
        notes.push("Mods live in profiles, so Pigment created and activated a “Default” profile from your current setup.".into());
    }

    let name = name.to_string();
    let report = ctx
        .store
        .update_active(ctx.sober.paths(), |p| {
            let present = p.mods.contains(&name);
            if enable && !present {
                p.mods.push(name.clone());
            } else if !enable && present {
                p.mods.retain(|n| *n != name);
            }
        })
        .map_err(|e| e.to_string())?;
    if let Some(rep) = report {
        if let Some(backup) = rep.overlay.backed_up {
            notes.push(format!(
                "Your existing asset_overlay files were moved to {} so Pigment could manage the overlay.",
                backup.display()
            ));
        }
        if !rep.overlay.blocked_by_unmanaged.is_empty() {
            notes.push(format!(
                "{} file(s) were skipped because a hand-placed overlay file is already there.",
                rep.overlay.blocked_by_unmanaged.len()
            ));
        }
    }
    Ok(notes)
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
fn update_conflict_banner(
    ctx: &Rc<Ctx>,
    installed: &[pigment_core::ModSource],
    enabled: &[String],
) {
    let enabled_sources: Vec<_> = installed
        .iter()
        .filter(|m| enabled.iter().any(|n| n == &m.name))
        .cloned()
        .collect();
    //hand placed overlay files count as a provider too, they win over every mod
    let unmanaged =
        mods::unmanaged_overlay_files(ctx.sober.paths().asset_overlay_dir()).unwrap_or_default();
    let blocked = enabled_sources
        .iter()
        .flat_map(|m| m.files().unwrap_or_default())
        .filter(|f| unmanaged.contains(f))
        .count();
    match mods::detect_conflicts(&enabled_sources) {
        _ if blocked > 0 => {
            ctx.banner.set_title(&format!(
                "{blocked} mod file{} blocked by files you placed in Sober's asset_overlay by hand.",
                if blocked == 1 { " is" } else { "s are" }
            ));
            ctx.banner.set_revealed(true);
        }
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
