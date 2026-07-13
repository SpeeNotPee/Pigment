//! The primary header menu (feedback, docs) and the About window, which carries
//! Pigment's license and the required legal disclaimer / trademark notice.

use adw::prelude::*;
use gtk::gio;

const SITE: &str = "https://speenotpee.github.io/Pigment/";
const REPO: &str = "https://github.com/SpeeNotPee/Pigment";
const ISSUES: &str = "https://github.com/SpeeNotPee/Pigment/issues";
const ISSUES_NEW_BUG: &str =
    "https://github.com/SpeeNotPee/Pigment/issues/new?template=bug_report.yml";
const DOCS: &str = "https://speenotpee.github.io/Pigment/guide.html";
const TERMS: &str = "https://github.com/SpeeNotPee/Pigment/blob/main/TERMS.md";
const PRIVACY: &str = "https://github.com/SpeeNotPee/Pigment/blob/main/PRIVACY.md";

/// Shown in the About window's "Legal" tab. Kept in sync with TERMS.md.
const DISCLAIMER: &str = "Pigment is free, unofficial, community software, provided \u{201c}as is\u{201d} \
without warranty of any kind.\n\n\
It is not affiliated with, endorsed by, or sponsored by Roblox Corporation or VinegarHQ. \
\u{201c}Roblox\u{201d} is a trademark of Roblox Corporation; \u{201c}Sober\u{201d} and \u{201c}Vinegar\u{201d} \
are projects of VinegarHQ. All trademarks are the property of their respective owners.\n\n\
You are responsible for complying with Roblox\u{2019}s Terms of Use and Community Standards. Using \
unofficial clients is at your own risk, including any risk to your account. Pigment uses only \
Sober\u{2019}s documented, sanctioned mechanisms and does not modify the Roblox client or bypass its \
anti-cheat.";

/// The primary menu button for the header bar. Call [`install_menu`] once the
/// window exists to wire the actions it triggers.
pub(crate) fn menu_button() -> gtk::MenuButton {
    gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .tooltip_text("Main menu")
        .primary(true)
        .build()
}

/// Attach the menu model and the window actions the menu triggers.
pub(crate) fn install_menu(window: &adw::ApplicationWindow, button: &gtk::MenuButton) {
    let menu = gio::Menu::new();
    let feedback = gio::Menu::new();
    feedback.append(Some("Report a Bug\u{2026}"), Some("win.report-bug"));
    feedback.append(Some("Documentation"), Some("win.docs"));
    feedback.append(Some("Website"), Some("win.website"));
    menu.append_section(None, &feedback);
    let about_section = gio::Menu::new();
    about_section.append(Some("About Pigment"), Some("win.about"));
    menu.append_section(None, &about_section);
    button.set_menu_model(Some(&menu));

    window.add_action_entries([
        gio::ActionEntry::builder("report-bug")
            .activate(|win: &adw::ApplicationWindow, _, _| open_uri(win, ISSUES_NEW_BUG))
            .build(),
        gio::ActionEntry::builder("docs")
            .activate(|win: &adw::ApplicationWindow, _, _| open_uri(win, DOCS))
            .build(),
        gio::ActionEntry::builder("website")
            .activate(|win: &adw::ApplicationWindow, _, _| open_uri(win, SITE))
            .build(),
        gio::ActionEntry::builder("about")
            .activate(|win: &adw::ApplicationWindow, _, _| show_about(win))
            .build(),
    ]);
}

/// Open a URL in the user's default browser.
fn open_uri(window: &impl IsA<gtk::Window>, url: &'static str) {
    gtk::UriLauncher::new(url).launch(
        Some(window),
        gio::Cancellable::NONE,
        move |res| {
            if let Err(e) = res {
                eprintln!("pigment: could not open {url}: {e}");
            }
        },
    );
}

/// Present the About window, including the version, license, links to the full
/// Terms and Privacy documents, a "Report an Issue" action, and the legal
/// disclaimer / trademark notice.
pub(crate) fn show_about(parent: &adw::ApplicationWindow) {
    let about = adw::AboutWindow::builder()
        .transient_for(parent)
        .modal(true)
        .application_icon("org.pigment.Pigment")
        .application_name("Pigment")
        .version(env!("CARGO_PKG_VERSION"))
        .developer_name("Pigment contributors")
        .comments("A Roblox launcher and manager for Linux, built on the Sober runtime.")
        .website(SITE)
        .issue_url(ISSUES)
        .license_type(gtk::License::MitX11)
        .copyright("\u{00a9} 2026 Pigment contributors")
        .build();
    about.add_link("Source Code", REPO);
    about.add_link("Terms of Use", TERMS);
    about.add_link("Privacy Policy", PRIVACY);
    about.add_legal_section(
        "Disclaimer & Trademarks",
        None,
        gtk::License::Custom,
        Some(DISCLAIMER),
    );
    about.present();
}
