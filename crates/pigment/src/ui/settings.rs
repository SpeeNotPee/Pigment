///settings and more configurations, genuinely tired right now.
use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use pigment_core::{Config, ProfileStore, Sober};

/// switch rows + what they showed at build time, so apply only writes what u touched
type SwitchRows = Rc<RefCell<Vec<(&'static str, adw::SwitchRow, bool)>>>;
/// combo rows w/ their option list (may include an unknown value sober wrote) + start index
type ComboRows = Rc<RefCell<Vec<(&'static str, adw::ComboRow, Vec<String>, u32)>>>;

/// more auto generated cause I cannot be bothed to write all of this
const BOOL_SETTINGS: &[(&str, &str, &str)] = &[
    (
        "discord_rpc_enabled",
        "Discord Rich Presence",
        "Share your current game on Discord",
    ),
    (
        "discord_rpc_show_join_button",
        "Discord Join Button",
        "Let friends join from your Discord profile",
    ),
    (
        "enable_gamemode",
        "Feral GameMode",
        "Optimize system performance while playing",
    ),
    (
        "use_opengl",
        "Use OpenGL",
        "Fall back from Vulkan for compatibility",
    ),
    (
        "enable_hidpi",
        "HiDPI Scaling",
        "Scale the UI for high-resolution displays",
    ),
    (
        "server_location_indicator_enabled",
        "Server Location",
        "Show the game server's location on join",
    ),
    (
        "close_on_leave",
        "Close on Leave",
        "Quit Sober when you leave a game",
    ),
    (
        "allow_gamepad_permission",
        "Gamepad Access",
        "Allow controllers and gamepads",
    ),
    (
        "use_console_experience",
        "Console UI",
        "Use the console interface instead of desktop",
    ),
    (
        "enable_mobile_home_screen",
        "Mobile Home Screen",
        "Use the mobile home UI instead of the desktop one",
    ),
    (
        "use_libsecret",
        "Store Login in Keyring",
        "Keep the session cookie in libsecret instead of plaintext (experimental)",
    ),
];

/// Enum settings: (config key, title, options).
const ENUM_SETTINGS: &[(&str, &str, &[&str])] = &[
    (
        "graphics_optimization_mode",
        "Graphics Optimization",
        &["quality", "balanced", "performance"],
    ),
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

    // toggles
    let general = adw::PreferencesGroup::builder().title("General").build();
    for (key, title, subtitle) in BOOL_SETTINGS {
        let initial = config.get_bool(key).unwrap_or(false);
        let row = adw::SwitchRow::builder()
            .title(*title)
            .subtitle(*subtitle)
            .active(initial)
            .build();
        general.add(&row);
        switches.borrow_mut().push((key, row, initial));
    }
    page.add(&general);

    // Enum combos.
    // PreferencesGroup titles are Pango markup, so the & must be escaped.
    let graphics = adw::PreferencesGroup::builder()
        .title("Graphics &amp; Input")
        .build();
    for (key, title, options) in ENUM_SETTINGS {
        let mut opts: Vec<String> = options.iter().map(|o| o.to_string()).collect();
        let current = config.get(key).and_then(|v| v.as_str()).unwrap_or("");
        //a value i dont know (newer sober?) shows up as its own option instead of getting
        //silently reset to the first one on apply
        if !current.is_empty() && !opts.iter().any(|o| o == current) {
            opts.push(current.to_string());
        }
        let refs: Vec<&str> = opts.iter().map(String::as_str).collect();
        let model = gtk::StringList::new(&refs);
        let selected = opts.iter().position(|o| o == current).unwrap_or(0) as u32;
        let row = adw::ComboRow::builder()
            .title(*title)
            .model(&model)
            .selected(selected)
            .build();
        graphics.add(&row);
        combos.borrow_mut().push((key, row, opts, selected));
    }
    page.add(&graphics);

    // Aaply group.
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
            //only the controls u actually changed. untouched + absent keys stay as they r
            let mut changed: Vec<(&'static str, serde_json::Value)> = Vec::new();
            for (key, row, initial) in switches.borrow().iter() {
                if row.is_active() != *initial {
                    changed.push((key, row.is_active().into()));
                }
            }
            for (key, row, options, initial) in combos.borrow().iter() {
                if row.selected() != *initial {
                    if let Some(choice) = options.get(row.selected() as usize) {
                        changed.push((key, choice.clone().into()));
                    }
                }
            }
            if changed.is_empty() {
                status.set_text("Nothing changed.");
                return;
            }
            match save_settings(&config_path, &changed) {
                Ok(Some(profile)) => status.set_text(&format!(
                    "Saved to profile “{profile}”. Restart Roblox to apply."
                )),
                Ok(None) => status.set_text("Saved. Restart Roblox to apply."),
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

    page.add(&sober_tools_group(&sober));

    page.upcast()
}

/// active profile = source of truth (B1). i edit the profile n apply it; no profile = write
/// config.json directly. Ok(Some(name)) tells the ui which profile got it
fn save_settings(
    config_path: &std::path::Path,
    changed: &[(&'static str, serde_json::Value)],
) -> Result<Option<String>, String> {
    let sober = Sober::discover().ok_or("no $HOME")?;
    if let Some(store) = ProfileStore::discover() {
        let name = store.active();
        let res = store
            .update_active(sober.paths(), |p| {
                for (k, v) in changed {
                    p.settings.insert((*k).to_string(), v.clone());
                }
            })
            .map_err(|e| e.to_string())?;
        if res.is_some() {
            return Ok(name);
        }
    }
    //reload fresh so unknown keys and concurrent Sober edits are kept.
    let mut cfg = Config::load(config_path).map_err(|e| e.to_string())?;
    for (k, v) in changed {
        cfg.set(*k, v.clone());
    }
    cfg.save(config_path).map_err(|e| e.to_string())?;
    Ok(None)
}

/// sobers own settings window + the undo-everything button
fn sober_tools_group(sober: &Sober) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("Sober").build();
    let status = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();

    let open = adw::ActionRow::builder()
        .title("Open Sober's own settings")
        .subtitle("The settings window Sober ships with")
        .activatable(true)
        .build();
    open.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
    {
        let (sober, status) = (sober.clone(), status.clone());
        open.connect_activated(move |_| match sober.open_settings() {
            Ok(child) => pigment_core::reap(child),
            Err(e) => status.set_text(&format!("Could not open Sober settings: {e}")),
        });
    }
    group.add(&open);

    let restore = adw::ActionRow::builder()
        .title("Restore original Sober config")
        .subtitle("Put config.json back exactly as it was before Pigment first changed it")
        .build();
    let btn = gtk::Button::builder()
        .label("Restore…")
        .valign(gtk::Align::Center)
        .css_classes(["destructive-action"])
        .sensitive(Config::has_original(sober.config_file()))
        .build();
    {
        let (path, status) = (sober.config_file(), status.clone());
        btn.connect_clicked(move |btn| {
            let (path, status) = (path.clone(), status.clone());
            super::confirm(
                btn,
                "Restore original config?",
                "Every Sober setting and FastFlag goes back to how it was before Pigment first saved. If a profile is active, it will reapply on the next launch.",
                "Restore",
                move || match Config::restore_original(&path) {
                    Ok(()) => status.set_text("Original config restored. Restart Roblox to apply."),
                    Err(e) => status.set_text(&format!("Restore failed: {e}")),
                },
            );
        });
    }
    restore.add_suffix(&btn);
    group.add(&restore);

    let status_row = adw::PreferencesRow::builder()
        .activatable(false)
        .child(&status)
        .build();
    group.add(&status_row);
    group
}

/// A page shown when the config doesn't exist yet, with a button to create it by launching Sober once.
fn needs_sober_page(sober: &Sober) -> gtk::Widget {
    let status = adw::StatusPage::builder()
        .icon_name("dialog-information-symbolic")
        .title("No Sober configuration yet")
        .description("Launch Roblox through Sober once so it creates its config, then come back to this page.")
        .build();

    let button = gtk::Button::builder()
        .label("Launch Sober")
        .halign(gtk::Align::Center)
        .css_classes(["pill", "suggested-action"])
        .sensitive(sober.is_installed())
        .build();
    let sober = sober.clone();
    button.connect_clicked(move |_| {
        if let Err(e) = super::launch(&sober, None) {
            eprintln!("pigment: {e}");
        }
    });
    status.set_child(Some(&button));
    status.upcast()
}

/// error page
fn error_page(msg: &str) -> gtk::Widget {
    adw::StatusPage::builder()
        .icon_name("dialog-error-symbolic")
        .title("Settings unavailable")
        .description(msg)
        .build()
        .upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EXPERIMENTAL, I'M NOT ENTIRELY SURE HOW THIS WOULD PAN OUT
    const DOCUMENTED_KEYS: &[&str] = &[
        "allow_gamepad_permission",
        "close_on_leave",
        "discord_rpc_enabled",
        "discord_rpc_show_join_button",
        "enable_gamemode",
        "enable_hidpi",
        "enable_mobile_home_screen",
        "graphics_optimization_mode",
        "server_location_indicator_enabled",
        "touch_mode",
        "use_console_experience",
        "use_libsecret",
        "use_opengl",
    ];

    fn managed_keys() -> Vec<&'static str> {
        let mut keys: Vec<&str> = BOOL_SETTINGS.iter().map(|(k, ..)| *k).collect();
        keys.extend(ENUM_SETTINGS.iter().map(|(k, ..)| *k));
        keys.sort_unstable();
        keys
    }

    #[test]
    fn every_documented_sober_key_has_a_control() {
        assert_eq!(managed_keys(), DOCUMENTED_KEYS);
        //n core snapshots the exact same set, so a profile never freezes a key i cant show
        let mut core = pigment_core::MANAGED_SETTINGS.to_vec();
        core.sort_unstable();
        assert_eq!(core, DOCUMENTED_KEYS);
    }

    #[test]
    fn no_key_is_managed_twice() {
        let mut keys = managed_keys();
        let total = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), total, "duplicate key would apply twice");
    }

    #[test]
    fn enum_options_match_sobers_accepted_values() {
        // The docs spell touch_mode's third value `fake-off`, but Sober itself
        // writes and accepts `fake_off` — Daddy claude verified this shit so its good
        let touch = ENUM_SETTINGS
            .iter()
            .find(|(k, ..)| *k == "touch_mode")
            .expect("touch_mode row");
        assert_eq!(touch.2, ["off", "on", "fake_off"]);
    }
}
