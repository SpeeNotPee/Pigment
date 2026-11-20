use std::fs;
use std::io;
use std::path::Path;

use time::OffsetDateTime;

/// 1 game session: a join and the leave that ended it
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub place_id: u64,
    pub job_id: String,
    /// Universe id
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

    /// Whether the game is still in progress
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
    /// not in a game.
    Idle,
}

/// Parse all sessions from a log file, oldest first. `live` = this is the log sober is
/// writing rn (what latest.log points at). any other log is dead, so whatever was still
/// open in it gets closed at its last timestamp instead of being "in progress" forever
pub fn sessions_from_file(path: impl AsRef<Path>, live: bool) -> io::Result<Vec<Session>> {
    let bytes = match fs::read(path.as_ref()) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    //lossy bc 1 weird byte from a game used to fail the whole read = "not in a game"
    let text = String::from_utf8_lossy(&bytes);
    let (mut out, last) = parse(&text);
    if !live {
        close_open(&mut out, last);
    }
    Ok(out)
}

/// every session across every log in the dir, newest first, capped at `max`
pub fn recent_sessions(log_dir: &Path, max: usize) -> Vec<Session> {
    let live = fs::canonicalize(log_dir.join("latest.log")).ok();
    let mut all = Vec::new();
    if let Ok(entries) = fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            //skip the latest.log symlink itself, its target gets read anyway
            if path.extension().and_then(|e| e.to_str()) == Some("log")
                && path.file_name().and_then(|n| n.to_str()) != Some("latest.log")
            {
                let is_live = live.is_some() && fs::canonicalize(&path).ok() == live;
                all.extend(sessions_from_file(&path, is_live).unwrap_or_default());
            }
        }
    }
    all.sort_by_key(|s| (s.joined_at.is_none(), std::cmp::Reverse(s.joined_at)));
    all.truncate(max);
    all
}

/// Parse sessions from log text, oldest first.
pub fn sessions(log: &str) -> Vec<Session> {
    parse(log).0
}

fn close_open(sessions: &mut [Session], at: Option<OffsetDateTime>) {
    if let Some(last) = sessions.last_mut() {
        if last.left_at.is_none() {
            last.left_at = at.or(last.joined_at);
        }
    }
}

/// the actual parser. also hands back the last timestamp seen so dead logs can be closed
fn parse(log: &str) -> (Vec<Session>, Option<OffsetDateTime>) {
    let mut out: Vec<Session> = Vec::new();
    let mut last_ts: Option<OffsetDateTime> = None;
    for line in log.lines() {
        let ts = parse_timestamp(line);
        if ts.is_some() {
            last_ts = ts;
        }
        if let Some((place_id, job_id, at)) = parse_join(line) {
            //new join kills whatever session was still open. sober dont always log a leave
            close_open(&mut out, at.or(last_ts));
            out.push(Session {
                place_id,
                job_id,
                universe_id: None,
                joined_at: at,
                left_at: None,
            });
        } else if let Some(universe_id) = parse_join_detail(line) {
            //bolt the uni id onto the most recent join. first one fine, rest ignored
            if let Some(last) = out.last_mut() {
                if last.universe_id.is_none() {
                    last.universe_id = Some(universe_id);
                }
            }
        } else if is_leave(line) {
            //onGameLeaveBegin lines got no `Roblox: <ts>`, so borrow the last one i saw
            close_open(&mut out, ts.or(last_ts));
        }
    }
    (out, last_ts)
}

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

/// universe id from a `GameJoinLoadTime` line.
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

/// Extract the number following key in a line.
fn parse_kv_number(line: &str, key: &str) -> Option<u64> {
    let after = line.split_once(key)?.1;
    let digits: String = after
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

fn parse_timestamp(line: &str) -> Option<OffsetDateTime> {
    let after = line.split_once("Roblox: ")?.1;
    let iso = after.split(',').next()?.trim();
    OffsetDateTime::parse(iso, &time::format_description::well_known::Rfc3339).ok()
}

///testingg..... | More auto generated
#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(
            dur.whole_seconds() > 150 && dur.whole_seconds() < 160,
            "{dur}"
        );
    }

    #[test]
    fn open_session_is_active_and_reported_as_status() {
        let log = [JOIN, DETAIL].join("\n"); // no leave
        let s = sessions(&log);
        assert_eq!(s.len(), 1);
        assert!(s[0].is_active());
        match current_status(&log) {
            Status::InGame {
                place_id,
                universe_id,
                ..
            } => {
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

    const JNI_LEAVE: &str = "debug: rbx.jni: onGameLeaveBegin() SessionReporterState_GameExitRequested placeId:17625359962";
    const LATER: &str =
        "info: Roblox: 2026-07-06T18:20:00.000Z,9.0,1773f6c0,6 [FLog::Output] whatever";

    #[test]
    fn join_then_join_closes_the_first() {
        let join2 = "info: Roblox: 2026-07-06T18:12:29.083Z,16.08,1c41f6c0,6 [FLog::Output] ! Joining game 'ff9cd0a6-6c4f-4bde-93fb-b45d1b0d86c1' place 999 at 10.18.6.146";
        let s = sessions(&[JOIN, join2].join("\n"));
        assert_eq!(s.len(), 2);
        assert!(!s[0].is_active());
        assert_eq!(s[0].left_at, s[1].joined_at);
        assert!(s[1].is_active());
    }

    #[test]
    fn timestampless_jni_leave_closes_the_session() {
        let s = sessions(&[JOIN, DETAIL, LATER, JNI_LEAVE].join("\n"));
        assert_eq!(s.len(), 1);
        assert!(!s[0].is_active(), "onGameLeaveBegin never fired");
        assert_eq!(s[0].left_at, parse_timestamp(LATER));
    }

    #[test]
    fn dead_log_that_ends_mid_game_is_closed() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("2026-07-06_18-00-00.log");
        fs::write(&old, [JOIN, DETAIL, LATER].join("\n")).unwrap();
        let live = dir.path().join("2026-07-07_10-00-00.log");
        fs::write(&live, [JOIN, DETAIL].join("\n")).unwrap();
        std::os::unix::fs::symlink(&live, dir.path().join("latest.log")).unwrap();

        assert!(!sessions_from_file(&old, false).unwrap()[0].is_active());
        let all = recent_sessions(dir.path(), 10);
        assert_eq!(all.len(), 2);
        assert_eq!(
            all.iter().filter(|s| s.is_active()).count(),
            1,
            "only the live log may stay open"
        );
    }

    #[test]
    fn non_utf8_log_still_parses() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.log");
        let mut bytes = JOIN.as_bytes().to_vec();
        bytes.extend_from_slice(b"\n\xff\xfe garbage\n");
        fs::write(&p, bytes).unwrap();
        assert_eq!(sessions_from_file(&p, true).unwrap().len(), 1);
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
