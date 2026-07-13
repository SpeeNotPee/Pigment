//! Parsing Sober's logs into game-session activity.
//!
//! Sober wraps the Roblox client's log lines with an `info: Roblox: ` prefix and
//! writes its own `debug: rbx.jni: …` lines. The events we care about, verified
//! against real logs:
//!
//! * **Join** — `[FLog::Output] ! Joining game '<jobId>' place <placeId> at <ip>`
//! * **Join detail** — `[FLog::GameJoinLoadTime] … placeid:<n>, … universeid:<n>`
//!   (the adjacent line, giving us the universe id for name lookup)
//! * **Leave** — `rbx.jni: onGameLeaveBegin()` or `[DFLog::NetworkClient]
//!   Client:Disconnect`
//!
//! [`sessions`] pairs joins with the following leave into [`Session`]s;
//! [`current_status`] reports whether a game is in progress right now (used to
//! drive Discord Rich Presence).

use std::fs;
use std::io;
use std::path::Path;

use time::OffsetDateTime;

/// One game session: a join and the leave that ended it (if any).
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub place_id: u64,
    pub job_id: String,
    /// Universe id, if the join-detail line was seen — used for name lookup.
    pub universe_id: Option<u64>,
    pub joined_at: Option<OffsetDateTime>,
    pub left_at: Option<OffsetDateTime>,
}

impl Session {
    /// Session length, if both timestamps are known.
    pub fn duration(&self) -> Option<time::Duration> {
        match (self.joined_at, self.left_at) {
            (Some(a), Some(b)) if b >= a => Some(b - a),
            _ => None,
        }
    }

    /// Whether the game is still in progress (joined, no leave recorded).
    pub fn is_active(&self) -> bool {
        self.left_at.is_none()
    }
}

/// Present activity, derived from the tail of the log.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// Currently in a game.
    InGame {
        place_id: u64,
        universe_id: Option<u64>,
        since: Option<OffsetDateTime>,
    },
    /// Not in a game.
    Idle,
}

/// Parse all sessions from a log file, oldest first. A missing file yields an
/// empty list rather than an error.
pub fn sessions_from_file(path: impl AsRef<Path>) -> io::Result<Vec<Session>> {
    match fs::read_to_string(path.as_ref()) {
        Ok(text) => Ok(sessions(&text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// Parse sessions from log text, oldest first.
pub fn sessions(log: &str) -> Vec<Session> {
    let mut out: Vec<Session> = Vec::new();
    for line in log.lines() {
        if let Some((place_id, job_id, at)) = parse_join(line) {
            // A new join closes any still-open session implicitly (no leave seen).
            out.push(Session {
                place_id,
                job_id,
                universe_id: None,
                joined_at: at,
                left_at: None,
            });
        } else if let Some(universe_id) = parse_join_detail(line) {
            // Attach the universe id to the most recent join.
            if let Some(last) = out.last_mut() {
                if last.universe_id.is_none() {
                    last.universe_id = Some(universe_id);
                }
            }
        } else if is_leave(line) {
            // Close the open session only on a leave line that carries a
            // timestamp (the `Client:Disconnect` lines do; the timestamp-less
            // `onGameLeaveBegin` does not), so durations are accurate. Multiple
            // disconnect lines are ignored after the first via the None guard.
            if let Some(ts) = parse_timestamp(line) {
                if let Some(last) = out.last_mut() {
                    if last.left_at.is_none() {
                        last.left_at = Some(ts);
                    }
                }
            }
        }
    }
    out
}

/// The current activity status, from the newest session in the log.
pub fn current_status(log: &str) -> Status {
    match sessions(log).into_iter().next_back() {
        Some(s) if s.is_active() => Status::InGame {
            place_id: s.place_id,
            universe_id: s.universe_id,
            since: s.joined_at,
        },
        _ => Status::Idle,
    }
}

/// Parse a join line → (place_id, job_id, timestamp).
fn parse_join(line: &str) -> Option<(u64, String, Option<OffsetDateTime>)> {
    let rest = line.split_once("! Joining game '")?.1;
    let (job_id, rest) = rest.split_once('\'')?;
    let after_place = rest.split_once("place ")?.1;
    let place_str: String = after_place
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let place_id = place_str.parse().ok()?;
    Some((place_id, job_id.to_string(), parse_timestamp(line)))
}

/// Parse the universe id from a `GameJoinLoadTime` line.
fn parse_join_detail(line: &str) -> Option<u64> {
    if !line.contains("GameJoinLoadTime") {
        return None;
    }
    parse_kv_number(line, "universeid:")
}

/// Whether a line marks leaving/disconnecting from a game.
fn is_leave(line: &str) -> bool {
    line.contains("onGameLeaveBegin(") || line.contains("[DFLog::NetworkClient] Client:Disconnect")
}

/// Extract the number following `key` (e.g. `universeid:`) in a line.
fn parse_kv_number(line: &str, key: &str) -> Option<u64> {
    let after = line.split_once(key)?.1;
    let digits: String = after
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// Parse the ISO-8601 timestamp that Roblox lines carry right after `Roblox: `.
fn parse_timestamp(line: &str) -> Option<OffsetDateTime> {
    let after = line.split_once("Roblox: ")?.1;
    // The timestamp is the first comma-separated field, e.g. 2026-07-06T18:08:20.542Z
    let iso = after.split(',').next()?.trim();
    OffsetDateTime::parse(iso, &time::format_description::well_known::Rfc3339).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real lines captured from this machine's Sober logs.
    const JOIN: &str = "info: Roblox: 2026-07-06T18:08:20.542Z,66.542740,1773f6c0,6 [FLog::Output] ! Joining game '135d0895-503a-4884-885e-c963758024e3' place 17625359962 at 10.186.9.73";
    const DETAIL: &str = "info: Roblox: 2026-07-06T18:08:20.542Z,66.542816,1773f6c0,6 [FLog::GameJoinLoadTime] Report game_join_loadtime: sid:afca873c, clienttime:1783361301.27, join_time:0.25, referral_page:, placeid:17625359962, userid:1337903335, universeid:6035872082, ";
    const LEAVE: &str = "info: Roblox: 2026-07-06T18:10:54.544Z,220.544342,206cf6c0,6,Info [DFLog::NetworkClient] Client:Disconnect";

    #[test]
    fn parses_a_join_line() {
        let (place, job, at) = parse_join(JOIN).unwrap();
        assert_eq!(place, 17625359962);
        assert_eq!(job, "135d0895-503a-4884-885e-c963758024e3");
        assert_eq!(at.unwrap().year(), 2026);
    }

    #[test]
    fn parses_universe_id_from_detail() {
        assert_eq!(parse_join_detail(DETAIL), Some(6035872082));
        assert_eq!(parse_join_detail(JOIN), None);
    }

    #[test]
    fn recognizes_leave_lines() {
        assert!(is_leave(LEAVE));
        assert!(is_leave("debug: rbx.jni: onGameLeaveBegin() SessionReporterState_GameExitRequested placeId:17625359962"));
        assert!(!is_leave(JOIN));
    }

    #[test]
    fn pairs_a_full_session() {
        let log = [JOIN, DETAIL, LEAVE].join("\n");
        let s = sessions(&log);
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].place_id, 17625359962);
        assert_eq!(s[0].universe_id, Some(6035872082));
        assert!(!s[0].is_active());
        let dur = s[0].duration().unwrap();
        assert!(dur.whole_seconds() > 150 && dur.whole_seconds() < 160, "{dur}");
    }

    #[test]
    fn open_session_is_active_and_reported_as_status() {
        let log = [JOIN, DETAIL].join("\n"); // no leave
        let s = sessions(&log);
        assert_eq!(s.len(), 1);
        assert!(s[0].is_active());
        match current_status(&log) {
            Status::InGame { place_id, universe_id, .. } => {
                assert_eq!(place_id, 17625359962);
                assert_eq!(universe_id, Some(6035872082));
            }
            Status::Idle => panic!("expected InGame"),
        }
    }

    #[test]
    fn idle_when_last_session_closed() {
        let log = [JOIN, DETAIL, LEAVE].join("\n");
        assert_eq!(current_status(&log), Status::Idle);
    }

    #[test]
    fn multiple_sessions_are_ordered() {
        let join2 = "info: Roblox: 2026-07-06T18:12:29.083Z,16.08,1c41f6c0,6 [FLog::Output] ! Joining game 'ff9cd0a6-6c4f-4bde-93fb-b45d1b0d86c1' place 999 at 10.18.6.146";
        let log = [JOIN, DETAIL, LEAVE, join2].join("\n");
        let s = sessions(&log);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].place_id, 17625359962);
        assert_eq!(s[1].place_id, 999);
        assert!(s[1].is_active());
    }
}
