//! Settings page: a typed UI over Sober's real `config.json` keys.
//!
//! On Apply we reload the config from disk and set only the keys this page
//! manages, so any key Pigment doesn't know about — and any concurrent edit by
//! Sober's own settings dialog — survives. The write itself is atomic, validated,
//! and backed up by `pigment_core::Config`.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use pigment_core::{Config, Sober};

/// Collected boolean switch rows, read back on Apply.
type SwitchRows = Rc<RefCell<Vec<(&'static str, adw::SwitchRow)>>>;
/// Collected enum combo rows with their option lists, read back on Apply.
type ComboRows = Rc<RefCell<Vec<(&'static str, adw::ComboRow, &'static [&'static str])>>>;

/// Boolean settings: (config key, title, subtitle).
const BOOL_SETTINGS: &[(&str, &str, &str)] = &[
    ("discord_rpc_enabled", "Discord Rich Presence", "Share your current game on Discord"),
    ("discord_rpc_show_join_button", "Discord Join Button", "Let friends join from your Discord profile"),
    ("enable_gamemode", "Feral GameMode", "Optimize system performance while playing"),
    ("use_opengl", "Use OpenGL", "Fall back from Vulkan for compatibility"),
    ("enable_hidpi", "HiDPI Scaling", "Scale the UI for high-resolution displays"),
    ("server_location_indicator_enabled", "Server Location", "Show the game server's location on join"),
    ("close_on_leave", "Close on Leave", "Quit Sober when you leave a game"),
    ("allow_gamepad_permission", "Gamepad Access", "Allow controllers and gamepads"),
    ("use_console_experience", "Console UI", "Use the console interface instead of desktop"),
];

/// Enum settings: (config key, title, options).
const ENUM_SETTINGS: &[(&str, &str, &[&str])] = &[
    ("graphics_optimization_mode", "Graphics Optimization", &["quality", "balanced", "performance"]),
    ("touch_mode", "Touch Mode", &["off", "on", "fake_off"]),
];

/// Build the Settings page.
pub fn build() -> gtk::Widget {
    let Some(sober) = Sober::discover() else {
        return error_page("Could not determine your home directory.");
    };
    let config_path = sober.config_file();

    let config = match Config::load(&config_path) {
        Ok(c) => c,
        Err(_) => return needs_sober_page(&sober),
    };

    let page = adw::PreferencesPage::new();

    // Collected widget handles, read back on Apply.
    let switches: SwitchRows = Rc::new(RefCell::new(Vec::new()));
    let combos: ComboRows = Rc::new(RefCell::new(Vec::new()));

    // Boolean toggles.
    let general = adw::PreferencesGroup::builder().title("General").build();
    for (key, title, subtitle) in BOOL_SETTINGS {
        let row = adw::SwitchRow::builder()
            .title(*title)
            .subtitle(*subtitle)
            .active(config.get_bool(key).unwrap_or(false))
            .build();
        general.add(&row);
        switches.borrow_mut().push((key, row));
    }
    page.add(&general);

    // Enum combos.
    // PreferencesGroup titles are Pango markup, so the ampersand must be escaped.
    let graphics = adw::PreferencesGroup::builder()
        .title("Graphics &amp; Input")
        .build();
    for (key, title, options) in ENUM_SETTINGS {
        let model = gtk::StringList::new(options);
        let current = config.get(key).and_then(|v| v.as_str()).unwrap_or("");
        let selected = options.iter().position(|o| *o == current).unwrap_or(0) as u32;
        let row = adw::ComboRow::builder()
            .title(*title)
            .model(&model)
            .selected(selected)
            .build();
        graphics.add(&row);
        combos.borrow_mut().push((key, row, options));
    }
    page.add(&graphics);

    // Apply group.
    let apply_group = adw::PreferencesGroup::new();
    let status = gtk::Label::builder().css_classes(["dim-label"]).build();
    let apply = gtk::Button::builder()
        .label("Apply Changes")
        .halign(gtk::Align::End)
        .css_classes(["suggested-action"])
        .build();
    {
        let switches = switches.clone();
        let combos = combos.clone();
        let status = status.clone();
        let config_path = config_path.clone();
        apply.connect_clicked(move |_| {
            // Reload fresh so unknown keys and concurrent Sober edits are kept.
            let mut cfg = match Config::load(&config_path) {
                Ok(c) => c,
                Err(e) => {
                    status.set_text(&format!("Could not read config: {e}"));
                    return;
                }
            };
            for (key, row) in switches.borrow().iter() {
                cfg.set_bool(*key, row.is_active());
            }
            for (key, row, options) in combos.borrow().iter() {
                let idx = row.selected() as usize;
                if let Some(choice) = options.get(idx) {
                    cfg.set(*key, choice.to_string());
                }
            }
            match cfg.save(&config_path) {
                Ok(_) => status.set_text("Saved. Restart Roblox to apply."),
                Err(e) => status.set_text(&format!("Save failed: {e}")),
            }
        });
    }
    let apply_box = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .halign(gtk::Align::End)
        .build();
    apply_box.append(&status);
    apply_box.append(&apply);
    apply_group.add(&apply_box);
    page.add(&apply_group);

    page.upcast()
}

/// A page shown when the config doesn't exist yet, with a button to create it by
/// launching Sober once.
fn needs_sober_page(sober: &Sober) -> gtk::Widget {
    let status = adw::StatusPage::builder()
        .icon_name("dialog-information-symbolic")
        .title("No Sober configuration yet")
        .description("Launch Roblox through Sober once so it creates its config, then reopen Settings.")
        .build();

    let button = gtk::Button::builder()
        .label("Launch Sober")
        .halign(gtk::Align::Center)
        .css_classes(["pill", "suggested-action"])
        .sensitive(sober.is_installed())
        .build();
    let sober = sober.clone();
    button.connect_clicked(move |_| {
        let _ = sober.launch(None);
    });
    status.set_child(Some(&button));
    status.upcast()
}

/// A generic error page.
fn error_page(msg: &str) -> gtk::Widget {
    adw::StatusPage::builder()
        .icon_name("dialog-error-symbolic")
        .title("Settings unavailable")
        .description(msg)
        .build()
        .upcast()
}
