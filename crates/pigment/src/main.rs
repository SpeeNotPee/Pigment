/// just stuff
mod ui;
use adw::prelude::*;
use gtk::glib;
const APP_ID: &str = "net.pigmentlab.Pigment";
fn main() -> glib::ExitCode {
    let app = adw::Application::builder().application_id(APP_ID).build();
    app.connect_activate(|app| ui::build_window(app).present());
    app.run()
}
