//! Window assembly and shared UI helpers.

mod about;
mod activity;
mod fflags;
mod home;
mod mods;
mod profiles;
mod settings;

use adw::prelude::*;
use gtk::glib;

/// A navigable page: a sidebar entry and the widget it reveals.
struct Page {
    id: &'static str,
    title: &'static str,
    icon: &'static str,
    build: fn() -> gtk::Widget,
}

/// The pages, in sidebar order.
fn pages() -> Vec<Page> {
    vec![
        Page { id: "home", title: "Home", icon: "applications-games-symbolic", build: home::build },
        Page { id: "settings", title: "Settings", icon: "emblem-system-symbolic", build: settings::build },
        Page { id: "fflags", title: "FastFlags", icon: "preferences-other-symbolic", build: fflags::build },
        Page { id: "mods", title: "Mods", icon: "folder-pictures-symbolic", build: mods::build },
        Page { id: "profiles", title: "Profiles", icon: "avatar-default-symbolic", build: profiles::build },
        Page { id: "activity", title: "Activity", icon: "document-open-recent-symbolic", build: activity::build },
    ]
}

/// Build the main application window.
pub fn build_window(app: &adw::Application) -> adw::ApplicationWindow {
    let pages = pages();

    // Content side: a header + a stack of pages, swapped by the sidebar.
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();
    for page in &pages {
        let child = (page.build)();
        stack.add_titled(&child, Some(page.id), page.title);
    }

    let content_header = adw::HeaderBar::new();
    let content_title = adw::WindowTitle::new("Pigment", "");
    content_header.set_title_widget(Some(&content_title));
    // Primary menu (feedback, docs, about). Wired to actions once the window
    // exists, below.
    let menu = about::menu_button();
    content_header.pack_end(&menu);

    let content_view = adw::ToolbarView::new();
    content_view.add_top_bar(&content_header);
    content_view.set_content(Some(&stack));

    let content_page = adw::NavigationPage::builder()
        .title("Pigment")
        .child(&content_view)
        .build();

    // Sidebar: a selectable list of page rows.
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::Single)
        .css_classes(["navigation-sidebar"])
        .build();
    for page in &pages {
        list.append(&sidebar_row(page.icon, page.title));
    }

    let sidebar_view = adw::ToolbarView::new();
    let sidebar_header = adw::HeaderBar::new();
    sidebar_header.set_title_widget(Some(&adw::WindowTitle::new("Pigment", "")));
    sidebar_view.add_top_bar(&sidebar_header);
    sidebar_view.set_content(Some(&list));

    let sidebar_page = adw::NavigationPage::builder()
        .title("Pigment")
        .child(&sidebar_view)
        .build();

    let split = adw::NavigationSplitView::builder()
        .sidebar(&sidebar_page)
        .content(&content_page)
        .min_sidebar_width(200.0)
        .max_sidebar_width(240.0)
        .build();

    // Selecting a sidebar row swaps the content stack and updates the title.
    {
        let stack = stack.clone();
        let ids: Vec<&'static str> = pages.iter().map(|p| p.id).collect();
        let titles: Vec<&'static str> = pages.iter().map(|p| p.title).collect();
        list.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let i = row.index() as usize;
                if let (Some(id), Some(title)) = (ids.get(i), titles.get(i)) {
                    stack.set_visible_child_name(id);
                    content_title.set_title(title);
                }
            }
        });
    }
    // Start on Home, unless PIGMENT_START_PAGE names another page (a testing
    // hook that lets screenshots target a specific page).
    let start_index = std::env::var("PIGMENT_START_PAGE")
        .ok()
        .and_then(|want| pages.iter().position(|p| p.id == want))
        .unwrap_or(0) as i32;
    if let Some(row) = list.row_at_index(start_index) {
        list.select_row(Some(&row));
    }

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Pigment")
        .default_width(940)
        .default_height(660)
        .content(&split)
        .build();
    about::install_menu(&window, &menu);

    // Testing hook: open the About window shortly after startup (once the main
    // window is presented) for screenshots.
    if std::env::var_os("PIGMENT_SHOW_ABOUT").is_some() {
        let window = window.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
            about::show_about(&window);
        });
    }
    window
}

/// A sidebar row: icon + label.
fn sidebar_row(icon: &str, title: &str) -> gtk::ListBoxRow {
    let box_ = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(12)
        .margin_top(10)
        .margin_bottom(10)
        .margin_start(6)
        .margin_end(6)
        .build();
    box_.append(&gtk::Image::from_icon_name(icon));
    box_.append(&gtk::Label::new(Some(title)));
    gtk::ListBoxRow::builder().child(&box_).build()
}

/// Standard vertical scroller wrapper for a page's content.
pub(crate) fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(child)
        .build()
}

/// Show a transient toast-like message via a simple message dialog is overkill;
/// callers use this to log to stderr until a toast overlay is wired in.
pub(crate) fn note(msg: &str) {
    glib::g_message!("pigment", "{msg}");
}

/// Absolute path to the sibling `pigment-launch` binary — the protocol handler.
///
/// It lives beside the running `pigment` binary in every layout we ship (dev
/// `target/…`, or `/usr/bin` when packaged), so we derive it from the current
/// executable rather than hard-coding a path.
pub(crate) fn launch_binary_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.with_file_name("pigment-launch"))
}

/// Snapshot the current Sober config into a new profile: every top-level config
/// key becomes a forced setting, and the current fflag map is captured. Shared by
/// the Profiles page ("save current setup") and the Mods page (auto-creating a
/// default profile when the first mod is enabled).
pub(crate) fn snapshot_profile(
    name: &str,
    sober: &pigment_core::Sober,
) -> Result<pigment_core::Profile, String> {
    let config = pigment_core::Config::load(sober.config_file()).map_err(|e| e.to_string())?;
    let mut profile = pigment_core::Profile::new(name);
    for key in config.keys().map(str::to_string).collect::<Vec<_>>() {
        if key == "fflags" {
            continue;
        }
        if let Some(v) = config.get(&key) {
            profile.settings.insert(key, v.clone());
        }
    }
    if let Some(flags) = config.fflags() {
        profile.fflags = flags.clone();
    }
    Ok(profile)
}
