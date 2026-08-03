/// I dont think discord works works tbh

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use adw::prelude::*;
use pigment_core::{activity, discord, roblox_api, Sober, Status};

/// I'm 99% this shit cannot be hjacked but I could always be wrong idk
const DISCORD_CLIENT_ID: &str = "1526262789927075950";

/// Newest N sessions to show.
const MAX_SESSIONS: usize = 20;

/// activity page
pub fn build() -> gtk::Widget {
    let Some(sober) = Sober::discover() else {
        return error_page("Could not determine your home directory.");
    };
    let log_path = sober.paths().latest_log();

    let page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(16)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();

    page.append(&discord_group(&log_path));
    page.append(&recent_games_group(&log_path));

    super::scrolled(&page).upcast()
}

/// thank you daddy claude
fn discord_group(log_path: &std::path::Path) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Discord Rich Presence")
        .description("Show the game you're playing on Discord. Requires Discord running. If Sober's own Discord presence is on, disable it to avoid duplicates.")
        .build();

    let row = adw::SwitchRow::builder()
        .title("Share your game on Discord")
        .subtitle("Off")
        .build();
    group.add(&row);

    // I actually dont know what this does but during testing, it keeps breaking whenever I delete so I lowk keep it there
    let (status_tx, status_rx) = async_channel::unbounded::<String>();
    {
        let row = row.clone();
        gtk::glib::spawn_future_local(async move {
            while let Ok(msg) = status_rx.recv().await {
                row.set_subtitle(&msg);
            }
        });
    }

    // yo I genuinely dont know what tf is, I pray to whoever tries to understand this
    let running: Rc<std::cell::RefCell<Option<Arc<AtomicBool>>>> =
        Rc::new(std::cell::RefCell::new(None));
    let log_path = log_path.to_path_buf();
    row.connect_active_notify(move |row| {
        if let Some(flag) = running.borrow_mut().take() {
            flag.store(false, Ordering::Relaxed);
        }
        if row.is_active() {
            let flag = Arc::new(AtomicBool::new(true));
            *running.borrow_mut() = Some(flag.clone());
            let (log_path, status_tx) = (log_path.clone(), status_tx.clone());
            std::thread::spawn(move || presence_loop(log_path, flag, status_tx));
        } else {
            let _ = status_tx.send_blocking("Off".to_string());
        }
    });

    group
}

/// presence loop
fn presence_loop(
    log_path: std::path::PathBuf,
    running: Arc<AtomicBool>,
    status: async_channel::Sender<String>,
) {
    let mut client = match discord::Client::connect(DISCORD_CLIENT_ID) {
        Ok(c) => {
            let _ = status.send_blocking("Connected to Discord".to_string());
            c
        }
        Err(e) => {
            let _ = status.send_blocking(format!("Could not connect to Discord: {e}"));
            return;
        }
    };

    let mut names: HashMap<u64, String> = HashMap::new();
    let mut current: Option<u64> = None; // game id

    while running.load(Ordering::Relaxed) {
        let text = std::fs::read_to_string(&log_path).unwrap_or_default();
        match activity::current_status(&text) {
            Status::InGame { place_id, universe_id, since } if current != Some(place_id) => {
                let name = resolve_name(&mut names, universe_id)
                    .unwrap_or_else(|| format!("Place {place_id}"));
                let start = since.map(|t| t.unix_timestamp());
                if client
                    .set_activity(&discord::Activity::playing(&name, start))
                    .is_ok()
                {
                    let _ = status.send_blocking(format!("Playing {name}"));
                    current = Some(place_id);
                }
            }
            Status::Idle if current.is_some() => {
                let _ = client.clear_activity();
                let _ = status.send_blocking("Connected — not in a game".to_string());
                current = None;
            }
            _ => {}
        }
        // Sleep in ~10s, but wake when stopped.
        for _ in 0..10 {
            if !running.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let _ = client.clear_activity();
}

/// Resolve a universe id to a game name (Roblox discord dev forums)
fn resolve_name(cache: &mut HashMap<u64, String>, universe_id: Option<u64>) -> Option<String> {
    let uid = universe_id?;
    if let Some(name) = cache.get(&uid) {
        return (!name.is_empty()).then(|| name.clone());
    }
    let name = roblox_api::game_name(uid).unwrap_or_default();
    cache.insert(uid, name.clone());
    (!name.is_empty()).then_some(name)
}

/// list of recent game session, I think chronological order is borken tho.
fn recent_games_group(log_path: &std::path::Path) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title("Recent Games").build();

    let sessions = recent_sessions(log_path);

    if sessions.is_empty() {
        group.add(
            &adw::ActionRow::builder()
                .title("No games yet")
                .subtitle("Play something and it will show up here")
                .build(),
        );
        return group;
    }

    let mut rows = Vec::new();
    let mut jobs = Vec::new();
    for (i, s) in sessions.iter().enumerate() {
        let row = adw::ActionRow::builder()
            .title(format!("Place {}", s.place_id))
            .subtitle(describe_session(s))
            .build();
        if s.is_active() {
            row.add_suffix(&gtk::Image::from_icon_name("media-playback-start-symbolic"));
        }
        if let Some(uid) = s.universe_id {
            jobs.push((i, uid));
        }
        rows.push(row.clone());
        group.add(&row);
    }

    if !jobs.is_empty() {
        let (tx, rx) = async_channel::unbounded::<(usize, String)>();
        std::thread::spawn(move || {
            let mut cache: HashMap<u64, String> = HashMap::new();
            for (i, uid) in jobs {
                if let Some(name) = resolve_name(&mut cache, Some(uid)) {
                    let _ = tx.send_blocking((i, name));
                }
            }
        });
        gtk::glib::spawn_future_local(async move {
            while let Ok((i, name)) = rx.recv().await {
                if let Some(row) = rows.get(i) {
                    row.set_title(&name);
                }
            }
        });
    }

    group
}

/// newest session so "ecent games spans launches rather than only the current session
fn recent_sessions(latest_log: &std::path::Path) -> Vec<pigment_core::Session> {
    let Some(dir) = latest_log.parent() else {
        return Vec::new();
    };
    let mut all = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Skip the `latest.log` symlink to avoid counting it twice.
            if path.extension().and_then(|e| e.to_str()) == Some("log")
                && path.file_name().and_then(|n| n.to_str()) != Some("latest.log")
            {
                all.extend(activity::sessions_from_file(&path).unwrap_or_default());
            }
        }
    }

    all.sort_by_key(|s| (s.joined_at.is_none(), std::cmp::Reverse(s.joined_at)));
    all.truncate(MAX_SESSIONS);
    all
}

/// session duration
fn describe_session(s: &pigment_core::Session) -> String {
    let when = s
        .joined_at
        .map(|t| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02} UTC",
                t.year(),
                u8::from(t.month()),
                t.day(),
                t.hour(),
                t.minute()
            )
        })
        .unwrap_or_else(|| "unknown time".to_string());

    if s.is_active() {
        return format!("{when} · in progress");
    }
    match s.duration() {
        Some(d) => {
            let (m, sec) = (d.whole_minutes(), d.whole_seconds() % 60);
            let dur = if m > 0 { format!("{m}m {sec}s") } else { format!("{sec}s") };
            format!("{when} · {dur}")
        }
        None => when,
    }
}

fn error_page(msg: &str) -> gtk::Widget {
    adw::StatusPage::builder()
        .icon_name("dialog-error-symbolic")
        .title("Activity unavailable")
        .description(msg)
        .build()
        .upcast()
}
