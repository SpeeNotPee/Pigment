/// the good shit, fastflags
use std::cell::Cell;
use std::rc::Rc;
use std::time::SystemTime;

use adw::prelude::*;
use pigment_core::{Config, ProfileStore, Sober};
use serde_json::{Map, Value};

thread_local! {
    //set when the editor has text that isnt saved. the page switcher checks it so peeking at
    //another page dont rebuild this one n eat ur edits
    static DIRTY: Cell<bool> = const { Cell::new(false) };
}

/// whether theres unsaved fastflag edits rn
pub fn has_unsaved_edits() -> bool {
    DIRTY.with(Cell::get)
}

fn set_dirty(v: bool) {
    DIRTY.with(|d| d.set(v));
}

fn mtime(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// write flags thru the active profile if there is one, else straight to config.json.
/// Ok(Some(name)) = went into that profile
fn save_flags(sober: &Sober, flags: Map<String, Value>) -> Result<Option<String>, String> {
    if let Some(store) = ProfileStore::discover() {
        let name = store.active();
        let flags2 = flags.clone();
        if store
            .update_active(sober.paths(), move |p| p.fflags = flags2)
            .map_err(|e| e.to_string())?
            .is_some()
        {
            return Ok(name);
        }
    }
    let path = sober.config_file();
    let mut cfg = Config::load(&path).map_err(|e| format!("Could not read config: {e}"))?;
    cfg.set_fflags(flags);
    cfg.save(&path).map_err(|e| e.to_string())?;
    Ok(None)
}

/// build the fastflags page.
pub fn build() -> gtk::Widget {
    let Some(sober) = Sober::discover() else {
        return error_page("Could not determine your home directory.");
    };
    let config_path = sober.config_file();

    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(_) => return error_page("Launch Roblox through Sober once so it creates its config, then come back to this page."),
    };

    //active profile = source of truth, so show ITS flags. config.json only if none active
    let store = ProfileStore::discover();
    let active = store
        .as_ref()
        .and_then(|s| s.active().and_then(|n| s.load(&n).ok()));
    let source_note = match &active {
        Some(p) => format!("Editing the FastFlags of your active profile “{}”.", p.name),
        None => {
            "No profile is active, so these are written straight to Sober's config.".to_string()
        }
    };
    let current = match &active {
        Some(p) => Value::Object(p.fflags.clone()),
        None => config
            .fflags()
            .cloned()
            .map(Value::Object)
            .unwrap_or_else(|| Value::Object(Default::default())),
    };
    let initial = serde_json::to_string_pretty(&current).unwrap_or_else(|_| "{}".into());
    //so i can tell if sober/a profile rewrote config.json while this page sat open
    let loaded_at = Rc::new(Cell::new(mtime(&config_path)));

    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(12)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();

    let intro = gtk::Label::builder()
        .label("Edit FastFlags as JSON. This is the same format as Bloxstrap — paste a Windows preset to import it. Only whitelisted flags take effect under Roblox's anti-cheat.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    root.append(&intro);
    root.append(
        &gtk::Label::builder()
            .label(&source_note)
            .wrap(true)
            .xalign(0.0)
            .css_classes(["caption", "dim-label"])
            .build(),
    );

    // The editor.
    let buffer = gtk::TextBuffer::builder().text(&initial).build();
    set_dirty(false);
    buffer.connect_changed(|_| set_dirty(true));
    let view = gtk::TextView::builder()
        .buffer(&buffer)
        .monospace(true)
        .top_margin(10)
        .bottom_margin(10)
        .left_margin(12)
        .right_margin(12)
        .build();
    let editor = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .min_content_height(300)
        .css_classes(["card"])
        .child(&view)
        .build();
    root.append(&editor);

    // status + apply row.
    let status = gtk::Label::builder()
        .xalign(0.0)
        .hexpand(true)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    // fastflag clear
    let clear = gtk::Button::builder()
        .label("Clear All")
        .tooltip_text("Remove all FastFlags")
        .css_classes(["destructive-action"])
        .build();
    {
        let (buffer, status, sober, loaded_at, config_path) = (
            buffer.clone(),
            status.clone(),
            sober.clone(),
            loaded_at.clone(),
            config_path.clone(),
        );
        clear.connect_clicked(move |btn| {
            let (buffer, status, sober, loaded_at, config_path) = (
                buffer.clone(),
                status.clone(),
                sober.clone(),
                loaded_at.clone(),
                config_path.clone(),
            );
            super::confirm(
                btn,
                "Clear all FastFlags?",
                "Every FastFlag gets removed. This can't be undone from Pigment.",
                "Clear All",
                move || match save_flags(&sober, Map::new()) {
                    Ok(_) => {
                        buffer.set_text("{}");
                        set_dirty(false);
                        loaded_at.set(mtime(&config_path));
                        set_ok(&status, "Cleared all FastFlags. Restart Roblox to apply.");
                    }
                    Err(e) => set_error(&status, &format!("Clear failed: {e}")),
                },
            );
        });
    }

    let apply = gtk::Button::builder()
        .label("Apply FastFlags")
        .css_classes(["suggested-action"])
        .build();
    {
        let (buffer, status, sober, loaded_at, config_path) = (
            buffer.clone(),
            status.clone(),
            sober.clone(),
            loaded_at.clone(),
            config_path.clone(),
        );
        apply.connect_clicked(move |_| {
            let text = buffer_text(&buffer);
            // Validate: must parse as a JSON object.
            let parsed: Value = match serde_json::from_str(&text) {
                Ok(v) => v,
                Err(e) => {
                    set_error(&status, &format!("Invalid JSON: {e}"));
                    return;
                }
            };
            let Some(map) = parsed.as_object() else {
                set_error(&status, "FastFlags must be a JSON object, e.g. { \"FFlagFoo\": true }");
                return;
            };
            //someone else wrote config.json since i loaded. warn once, 2nd click goes thru
            let now = mtime(&config_path);
            if now != loaded_at.get() {
                loaded_at.set(now);
                set_error(&status, "Sober's config changed since this page loaded. Click Apply again to overwrite its FastFlags, or open another page and come back to reload.");
                return;
            }
            match save_flags(&sober, map.clone()) {
                Ok(where_) => {
                    set_dirty(false);
                    loaded_at.set(mtime(&config_path));
                    let dest = where_.map(|n| format!(" to profile “{n}”")).unwrap_or_default();
                    set_ok(&status, &format!("Saved {} flag(s){dest}. Restart Roblox to apply.", map.len()));
                }
                Err(e) => set_error(&status, &format!("Save failed: {e}")),
            }
        });
    }

    let bar = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .build();
    bar.append(&status);
    bar.append(&clear);
    bar.append(&apply);
    root.append(&bar);

    root.upcast()
}

/// extract the full text of a buffer.
fn buffer_text(buffer: &gtk::TextBuffer) -> String {
    let (start, end) = buffer.bounds();
    buffer.text(&start, &end, false).to_string()
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
        .icon_name("dialog-information-symbolic")
        .title("FastFlags unavailable")
        .description(msg)
        .build()
        .upcast()
}
