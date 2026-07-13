//! `pigment` — the GTK4/libadwaita management GUI for the Pigment Roblox launcher.
//!
//! All behavior lives in `pigment-core`; this crate is presentation only. The
//! window is a sidebar navigation ([`adw::NavigationSplitView`]) over a stack of
//! pages, each backed by a core module.

mod ui;

use adw::prelude::*;
use gtk::glib;

/// Application id; also the desktop-file / protocol-handler base name.
const APP_ID: &str = "org.pigment.Pigment";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| ui::build_window(app).present());
    app.run()
}
