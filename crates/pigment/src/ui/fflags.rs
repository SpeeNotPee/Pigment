//! FastFlags page: a validating JSON editor over Sober's `fflags`.
//!
//! FastFlags are a plain string→value map, the exact shape Bloxstrap uses — so a
//! raw JSON editor is both the most powerful interface and a free Bloxstrap
//! import path (paste a `ClientAppSettings.json` body and Apply). Apply parses
//! and validates the text, requires a JSON object, then writes it through the
//! safe config writer, which re-checks validity before touching disk.

use adw::prelude::*;
use pigment_core::{Config, Sober};
use serde_json::Value;

/// Build the FastFlags page.
pub fn build() -> gtk::Widget {
    let Some(sober) = Sober::discover() else {
        return error_page("Could not determine your home directory.");
    };
    let config_path = sober.config_file();

    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(_) => return error_page("Launch Roblox through Sober once so it creates its config, then reopen this page."),
    };

    // Pretty-print the current fflags map as the editor's starting text.
    let current = config
        .fflags()
        .cloned()
        .map(Value::Object)
        .unwrap_or_else(|| Value::Object(Default::default()));
    let initial = serde_json::to_string_pretty(&current).unwrap_or_else(|_| "{}".into());

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

    // The editor.
    let buffer = gtk::TextBuffer::builder().text(&initial).build();
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

    // Status + apply row.
    let status = gtk::Label::builder()
        .xalign(0.0)
        .hexpand(true)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    // Clear/reset: wipe all FastFlags back to an empty set.
    let clear = gtk::Button::builder()
        .label("Clear All")
        .tooltip_text("Remove all FastFlags")
        .css_classes(["destructive-action"])
        .build();
    {
        let buffer = buffer.clone();
        let status = status.clone();
        let config_path = config_path.clone();
        clear.connect_clicked(move |_| {
            // Reload fresh so non-fflag keys survive, then empty the fflag map.
            let mut cfg = match Config::load(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    set_error(&status, &format!("Could not read config: {e}"));
                    return;
                }
            };
            cfg.set_fflags(serde_json::Map::new());
            match cfg.save(&config_path) {
                Ok(()) => {
                    buffer.set_text("{}");
                    set_ok(&status, "Cleared all FastFlags. Restart Roblox to apply.");
                }
                Err(e) => set_error(&status, &format!("Clear failed: {e}")),
            }
        });
    }

    let apply = gtk::Button::builder()
        .label("Apply FastFlags")
        .css_classes(["suggested-action"])
        .build();
    {
        let buffer = buffer.clone();
        let status = status.clone();
        let config_path = config_path.clone();
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
            // Reload fresh so non-fflag keys and concurrent edits survive.
            let mut cfg = match Config::load(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    set_error(&status, &format!("Could not read config: {e}"));
                    return;
                }
            };
            cfg.set_fflags(map.clone());
            match cfg.save(&config_path) {
                Ok(()) => set_ok(&status, &format!("Saved {} flag(s). Restart Roblox to apply.", map.len())),
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

/// Extract the full text of a buffer.
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
