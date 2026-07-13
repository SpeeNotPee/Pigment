//! Profiles page: create, activate, apply, and delete profiles.
//!
//! A profile is a named snapshot of Sober settings + FastFlags (+ mods, assigned
//! on the Mods page). The *active* profile is what `pigment-launch` applies each
//! time you launch. "Apply now" writes a profile onto Sober immediately.
//!
//! The list refreshes in place after every action via the free [`populate`]
//! function, which re-invokes itself from button callbacks — clean recursion,
//! no `Rc`-cycle gymnastics.

use std::rc::Rc;

use adw::prelude::*;
use pigment_core::{ProfileStore, Sober};

/// Build the Profiles page.
pub fn build() -> gtk::Widget {
    let (Some(store), Some(sober)) = (ProfileStore::discover(), Sober::discover()) else {
        return error_page("Could not determine your configuration directories.");
    };
    let store = Rc::new(store);
    let sober = Rc::new(sober);

    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(14)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();

    let status = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["dim-label"])
        .build();
    let status = Rc::new(status);

    // The profile list.
    let list = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .css_classes(["boxed-list"])
        .build();
    let list = Rc::new(list);

    // Create-a-profile controls.
    let name_entry = gtk::Entry::builder()
        .placeholder_text("New profile name")
        .hexpand(true)
        .build();
    let create = gtk::Button::builder()
        .label("Save current setup as profile")
        .css_classes(["suggested-action"])
        .build();
    {
        let (store, sober, list, status, name_entry) = (
            store.clone(),
            sober.clone(),
            list.clone(),
            status.clone(),
            name_entry.clone(),
        );
        create.connect_clicked(move |_| {
            let name = name_entry.text().trim().to_string();
            if name.is_empty() {
                set_error(&status, "Enter a name for the profile.");
                return;
            }
            match super::snapshot_profile(&name, &sober) {
                Ok(profile) => match store.save(&profile) {
                    Ok(()) => {
                        name_entry.set_text("");
                        set_ok(&status, &format!("Saved profile “{name}” from your current setup."));
                        populate(&list, &store, &sober, &status);
                    }
                    Err(e) => set_error(&status, &format!("Could not save profile: {e}")),
                },
                Err(e) => set_error(&status, &format!("Could not read current setup: {e}")),
            }
        });
    }

    let create_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .build();
    create_row.append(&name_entry);
    create_row.append(&create);

    let intro = gtk::Label::builder()
        .label("Profiles are applied one at a time. The active profile is applied automatically when you launch Roblox.")
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();

    page.append(&intro);
    page.append(list.as_ref());
    page.append(&create_row);
    page.append(status.as_ref());

    populate(&list, &store, &sober, &status);
    super::scrolled(&page).upcast()
}

/// Rebuild the profile list rows to reflect current storage.
fn populate(
    list: &Rc<gtk::ListBox>,
    store: &Rc<ProfileStore>,
    sober: &Rc<Sober>,
    status: &Rc<gtk::Label>,
) {
    list.remove_all();
    let names = store.list().unwrap_or_default();
    let active = store.active();

    if names.is_empty() {
        let empty = adw::ActionRow::builder()
            .title("No profiles yet")
            .subtitle("Save your current setup to create one")
            .build();
        list.append(&empty);
        return;
    }

    for name in names {
        let is_active = active.as_deref() == Some(name.as_str());
        let row = adw::ActionRow::builder().title(&name).build();
        if is_active {
            row.set_subtitle("Active — applied on launch");
            let check = gtk::Image::from_icon_name("emblem-ok-symbolic");
            row.add_prefix(&check);
        }

        // Apply now.
        let apply = icon_button("media-playback-start-symbolic", "Apply to Sober now");
        {
            let (store, sober, status, name) =
                (store.clone(), sober.clone(), status.clone(), name.clone());
            apply.connect_clicked(move |_| match store.load(&name) {
                Ok(p) => match store.apply(&p, sober.paths()) {
                    Ok(rep) => {
                        let mut msg = format!("Applied “{name}”.");
                        if !rep.missing_mods.is_empty() {
                            msg.push_str(&format!(" Missing mods: {}", rep.missing_mods.join(", ")));
                        }
                        msg.push_str(" Restart Roblox to see changes.");
                        set_ok(&status, &msg);
                    }
                    Err(e) => set_error(&status, &format!("Apply failed: {e}")),
                },
                Err(e) => set_error(&status, &format!("Could not load profile: {e}")),
            });
        }

        // Set active.
        let activate = icon_button("starred-symbolic", "Make active");
        activate.set_sensitive(!is_active);
        {
            let (store, sober, list, status, name) = (
                store.clone(),
                sober.clone(),
                list.clone(),
                status.clone(),
                name.clone(),
            );
            activate.connect_clicked(move |_| {
                if let Err(e) = store.set_active(Some(&name)) {
                    set_error(&status, &format!("Could not set active: {e}"));
                    return;
                }
                set_ok(&status, &format!("“{name}” is now the active profile."));
                populate(&list, &store, &sober, &status);
            });
        }

        // Delete.
        let delete = icon_button("user-trash-symbolic", "Delete profile");
        delete.add_css_class("destructive-action");
        {
            let (store, sober, list, status, name) = (
                store.clone(),
                sober.clone(),
                list.clone(),
                status.clone(),
                name.clone(),
            );
            delete.connect_clicked(move |_| {
                if let Err(e) = store.delete(&name) {
                    set_error(&status, &format!("Could not delete: {e}"));
                    return;
                }
                set_ok(&status, &format!("Deleted “{name}”."));
                populate(&list, &store, &sober, &status);
            });
        }

        let controls = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .valign(gtk::Align::Center)
            .build();
        controls.append(&apply);
        controls.append(&activate);
        controls.append(&delete);
        row.add_suffix(&controls);
        list.append(&row);
    }
}

/// A flat, tooltipped icon button.
fn icon_button(icon: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .icon_name(icon)
        .tooltip_text(tooltip)
        .css_classes(["flat"])
        .valign(gtk::Align::Center)
        .build()
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
        .title("Profiles unavailable")
        .description(msg)
        .build()
        .upcast()
}
