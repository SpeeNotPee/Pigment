// I am unsure about how this goes but whatever

mod about;
mod activity;
mod fflags;
mod home;
mod mods;
mod profiles;
mod settings;

use adw::prelude::*;
use gtk::glib;

/// page
struct Page {
    id: &'static str,
    title: &'static str,
    icon: &'static str,
    build: fn() -> gtk::Widget,
    /// rebuild every time its shown. pages that mirror config.json/profiles go stale
    /// otherwise n "apply" writes back startup values over newer ones (B7)
    fresh_on_show: bool,
}

/// The pages, in sidebar order.
fn pages() -> Vec<Page> {
    vec![
        Page {
            id: "home",
            title: "Home",
            icon: "applications-games-symbolic",
            build: home::build,
            fresh_on_show: false,
        },
        Page {
            id: "settings",
            title: "Settings",
            //emblem-system is gone from debian 13's adwaita, this one's everywhere
            icon: "preferences-system-symbolic",
            build: settings::build,
            fresh_on_show: true,
        },
        Page {
            id: "fflags",
            title: "FastFlags",
            icon: "preferences-other-symbolic",
            build: fflags::build,
            fresh_on_show: true,
        },
        Page {
            id: "mods",
            title: "Mods",
            icon: "folder-pictures-symbolic",
            build: mods::build,
            fresh_on_show: true,
        },
        Page {
            id: "profiles",
            title: "Profiles",
            icon: "avatar-default-symbolic",
            build: profiles::build,
            fresh_on_show: true,
        },
        //activity owns the discord thread, rebuilding would orphan it. built once, lazily
        Page {
            id: "activity",
            title: "Activity",
            icon: "document-open-recent-symbolic",
            build: activity::build,
            fresh_on_show: false,
        },
    ]
}

/// Build the main application window.
pub fn build_window(app: &adw::Application) -> adw::ApplicationWindow {
    let pages = pages();

    // content side
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();
    //every page starts as an empty placeholder n gets built the first time its picked.
    //b4 this all 6 built at startup = catalog fetch + game name lookups w/o opening em
    for page in &pages {
        stack.add_titled(
            &gtk::Box::new(gtk::Orientation::Vertical, 0),
            Some(page.id),
            page.title,
        );
    }

    let content_header = adw::HeaderBar::new();
    let content_title = adw::WindowTitle::new("Pigment", "");
    content_header.set_title_widget(Some(&content_title));
    let menu = about::menu_button();
    content_header.pack_end(&menu);

    let content_view = adw::ToolbarView::new();
    content_view.add_top_bar(&content_header);
    content_view.set_content(Some(&stack));

    let content_page = adw::NavigationPage::builder()
        .title("Pigment")
        .child(&content_view)
        .build();

    // sidebar
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

    // selecting a sidebar row swaps the content stack and updates the title.
    {
        let stack = stack.clone();
        let built = std::rc::Rc::new(std::cell::RefCell::new(vec![false; pages.len()]));
        let pages = std::rc::Rc::new(pages_meta(&pages));
        list.connect_row_selected(move |_, row| {
            let Some(row) = row else { return };
            let i = row.index() as usize;
            let Some(&(id, title, build, fresh)) = pages.get(i) else {
                return;
            };
            let first = !built.borrow()[i];
            //dont nuke half typed fastflags just bc u peeked at another page
            let keep = id == "fflags" && fflags::has_unsaved_edits();
            if first || (fresh && !keep) {
                if let Some(old) = stack.child_by_name(id) {
                    stack.remove(&old);
                }
                stack.add_titled(&build(), Some(id), title);
                built.borrow_mut()[i] = true;
            }
            stack.set_visible_child_name(id);
            content_title.set_title(title);
        });
    }
    // Start on Home, unless PIGMENT_START_PAGE names another page
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

    // Testing hook
    if std::env::var_os("PIGMENT_SHOW_ABOUT").is_some() {
        let window = window.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
            about::show_about(&window);
        });
    }
    window
}

type PageMeta = (&'static str, &'static str, fn() -> gtk::Widget, bool);

fn pages_meta(pages: &[Page]) -> Vec<PageMeta> {
    pages
        .iter()
        .map(|p| (p.id, p.title, p.build, p.fresh_on_show))
        .collect()
}

/// sidebar row
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

/// vertical scroller wrapper shit
pub(crate) fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(child)
        .build()
}

// even more shit bro
pub(crate) fn note(msg: &str) {
    glib::g_message!("pigment", "{msg}");
}

// holy fuck I'm tired
pub(crate) fn launch_binary_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.with_file_name("pigment-launch"))
}

/// Theres a Negev on site on B -ohnepixel
/// snapshot lives in core now (managed keys only), this is js the string-error wrapper
pub(crate) fn snapshot_profile(
    name: &str,
    sober: &pigment_core::Sober,
) -> Result<pigment_core::Profile, String> {
    let config = pigment_core::Config::load(sober.config_file()).map_err(|e| e.to_string())?;
    Ok(pigment_core::Profile::snapshot(name, &config))
}

/// launch thru the active profile + reap the child. Ok(warning) = launched, maybe w/ a
/// profile problem worth showing. Err = sober itself didnt start
pub(crate) fn launch(
    sober: &pigment_core::Sober,
    uri: Option<&str>,
) -> Result<Option<String>, String> {
    let outcome = pigment_core::launch_with_active_profile(sober, uri);
    match outcome.child {
        Ok(child) => {
            pigment_core::reap(child);
            Ok(outcome.profile_warning)
        }
        Err(e) => Err(format!("Could not start Sober: {e}")),
    }
}

/// yes/no confirm dialog. `on_yes` only runs if they hit the destructive button
pub(crate) fn confirm(
    parent: &impl IsA<gtk::Widget>,
    heading: &str,
    body: &str,
    yes_label: &str,
    on_yes: impl Fn() + 'static,
) {
    let window = parent.root().and_downcast::<gtk::Window>();
    let dialog = adw::MessageDialog::builder()
        .heading(heading)
        .body(body)
        .modal(true)
        .build();
    if let Some(w) = window.as_ref() {
        dialog.set_transient_for(Some(w));
    }
    dialog.add_responses(&[("cancel", "Cancel"), ("yes", yes_label)]);
    dialog.set_response_appearance("yes", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, resp| {
        if resp == "yes" {
            on_yes();
        }
    });
    dialog.present();
}
