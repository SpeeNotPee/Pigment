// I dont think discord works works tbh

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use adw::prelude::*;
use pigment_core::state::State;
use pigment_core::{activity, discord, roblox_api, PigmentPaths, Sober, Status};

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
    page.append(&recent_games_group(&sober, &log_path));

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

    //the running flag of whichever presence thread is alive. flipping the switch off (or
    //on again) tells the old one to stop
    let running: Rc<std::cell::RefCell<Option<Arc<AtomicBool>>>> =
        Rc::new(std::cell::RefCell::new(None));
    let log_path = log_path.to_path_buf();
    row.connect_active_notify(move |row| {
        if let Some(flag) = running.borrow_mut().take() {
            flag.store(false, Ordering::Relaxed);
        }
        //remember it for next launch. failing to save is not worth bugging anyone abt
        if let Some(paths) = PigmentPaths::discover() {
            let mut st = State::load(&paths);
            st.discord_presence = row.is_active();
            let _ = st.save(&paths);
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
    //switch comes back on if it was on last time
    if PigmentPaths::discover().is_some_and(|p| State::load(&p).discord_presence) {
        row.set_active(true);
    }

    group
}

/// sleep `secs` in 1s slices so turning the switch off is near instant. false = stopped
fn nap(running: &AtomicBool, secs: u64) -> bool {
    for _ in 0..secs {
        if !running.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    running.load(Ordering::Relaxed)
}

/// presence loop. keeps trying while the switch is on: discord not running yet, discord
/// restarted, whatever. any send error drops the client n we reconnect w/ backoff
fn presence_loop(
    log_path: std::path::PathBuf,
    running: Arc<AtomicBool>,
    status: async_channel::Sender<String>,
) {
    let mut names: HashMap<u64, String> = HashMap::new();
    let mut client: Option<discord::Client> = None;
    let mut current: Option<u64> = None; // game id
    let mut backoff = 5u64;

    while running.load(Ordering::Relaxed) {
        let Some(c) = client.as_mut() else {
            match discord::Client::connect(DISCORD_CLIENT_ID) {
                Ok(c) => {
                    let _ = status.send_blocking("Connected to Discord".to_string());
                    client = Some(c);
                    current = None;
                    backoff = 5;
                }
                Err(e) => {
                    let _ = status.send_blocking(format!(
                        "Waiting for Discord ({e}) — retrying in {backoff}s"
                    ));
                    if !nap(&running, backoff) {
                        break;
                    }
                    backoff = (backoff * 2).min(60);
                }
            }
            continue;
        };

        //lossy read, 1 bad byte in the log used to read as "not in a game"
        let text = std::fs::read(&log_path)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        let sent = match activity::current_status(&text) {
            Status::InGame {
                place_id,
                universe_id,
                since,
            } if current != Some(place_id) => {
                let name = resolve_name(&mut names, universe_id)
                    .unwrap_or_else(|| format!("Place {place_id}"));
                let start = since.map(|t| t.unix_timestamp());
                c.set_activity(&discord::Activity::playing(&name, start))
                    .map(|()| {
                        let _ = status.send_blocking(format!("Playing {name}"));
                        current = Some(place_id);
                    })
            }
            Status::Idle if current.is_some() => c.clear_activity().map(|()| {
                let _ = status.send_blocking("Connected — not in a game".to_string());
                current = None;
            }),
            _ => Ok(()),
        };
        if let Err(e) = sent {
            let _ = status.send_blocking(format!("Lost Discord ({e}) — reconnecting"));
            client = None;
            continue;
        }
        if !nap(&running, 10) {
            break;
        }
    }
    if let Some(mut c) = client {
        let _ = c.clear_activity();
    }
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

/// apply the active profile then hand the deep link to sober. same helper as every other
/// launch button now, so a broken profile never eats the join n errors show up in the ui
fn join(sober: &Sober, place_id: u64, job_id: Option<&str>, status: &gtk::Label) {
    let uri = pigment_core::join_uri(place_id, job_id);
    match super::launch(sober, Some(&uri)) {
        Ok(None) => {
            status.set_css_classes(&["dim-label"]);
            status.set_text("Joining…");
            super::note(&format!("joining {uri}"));
        }
        Ok(Some(w)) => {
            status.set_css_classes(&["warning"]);
            status.set_text(&format!("Joining, but {w}."));
        }
        Err(e) => {
            status.set_css_classes(&["error"]);
            status.set_text(&e);
        }
    }
}

/// lil suffix button for a session row
fn join_button(label: &str, tooltip: &str) -> gtk::Button {
    gtk::Button::builder()
        .label(label)
        .tooltip_text(tooltip)
        .valign(gtk::Align::Center)
        .css_classes(["flat"])
        .build()
}

/// list of recent game sessions, newest first. parsing every log is slow-ish so it
/// happens on a worker n the rows fill in after
fn recent_games_group(sober: &Sober, log_path: &std::path::Path) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Recent Games")
        .description("Join drops you in a fresh server. Same server rejoins the exact one — it errors if that server is gone.")
        .build();
    let loading = adw::ActionRow::builder()
        .title("Reading Sober logs…")
        .build();
    group.add(&loading);
    let status = gtk::Label::builder()
        .wrap(true)
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();

    let (tx, rx) = async_channel::bounded::<Vec<pigment_core::Session>>(1);
    let dir = log_path.parent().map(std::path::Path::to_path_buf);
    std::thread::spawn(move || {
        let sessions = dir
            .map(|d| activity::recent_sessions(&d, MAX_SESSIONS))
            .unwrap_or_default();
        let _ = tx.send_blocking(sessions);
    });

    let (group2, sober) = (group.clone(), sober.clone());
    gtk::glib::spawn_future_local(async move {
        let sessions = rx.recv().await.unwrap_or_default();
        group2.remove(&loading);
        fill_sessions(&group2, &sober, &sessions, &status);
    });
    group
}

fn fill_sessions(
    group: &adw::PreferencesGroup,
    sober: &Sober,
    sessions: &[pigment_core::Session],
    status: &gtk::Label,
) {
    if sessions.is_empty() {
        group.add(
            &adw::ActionRow::builder()
                .title("No games yet")
                .subtitle("Play something and it will show up here")
                .build(),
        );
        return;
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

        let fresh = join_button("Join", "Join this game on a fresh server");
        {
            let (sober, place_id, status) = (sober.clone(), s.place_id, status.clone());
            fresh.connect_clicked(move |_| join(&sober, place_id, None, &status));
        }
        row.add_suffix(&fresh);

        let same = join_button("Same server", "Rejoin the exact server this session was on");
        {
            let (sober, place_id, job_id, status) =
                (sober.clone(), s.place_id, s.job_id.clone(), status.clone());
            same.connect_clicked(move |_| join(&sober, place_id, Some(&job_id), &status));
        }
        same.set_sensitive(!s.job_id.is_empty());
        row.add_suffix(&same);

        if let Some(uid) = s.universe_id {
            jobs.push((i, uid));
        }
        rows.push(row.clone());
        group.add(&row);
    }
    group.add(
        &adw::PreferencesRow::builder()
            .activatable(false)
            .child(status)
            .build(),
    );

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
}

/// session duration
fn describe_session(s: &pigment_core::Session) -> String {
    //local time via glib. time crates now_local is unsound w/ threads so its a no
    let when = s
        .joined_at
        .and_then(|t| gtk::glib::DateTime::from_unix_local(t.unix_timestamp()).ok())
        .and_then(|d| d.format("%Y-%m-%d %H:%M").ok())
        .map(|g| g.to_string())
        .unwrap_or_else(|| "unknown time".to_string());

    if s.is_active() {
        return format!("{when} · in progress");
    }
    match s.duration() {
        Some(d) => {
            let (m, sec) = (d.whole_minutes(), d.whole_seconds() % 60);
            let dur = if m > 0 {
                format!("{m}m {sec}s")
            } else {
                format!("{sec}s")
            };
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
