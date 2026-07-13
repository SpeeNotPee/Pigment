//! Minimal, best-effort lookups against Roblox's public web API.
//!
//! Used to turn the numeric universe id we parse from the logs into a human game
//! name for the Activity view and Discord Rich Presence. Every call is
//! best-effort: any network or parse failure yields `None`, and callers fall back
//! to showing the id. Requests are blocking with a short timeout, so callers run
//! them off the UI thread.

use std::time::Duration;

/// The games endpoint: `?universeIds=<id>` → `{ "data": [ { "name": … } ] }`.
const GAMES_ENDPOINT: &str = "https://games.roblox.com/v1/games";

/// Resolve a universe id to its experience name, or `None` on any failure.
pub fn game_name(universe_id: u64) -> Option<String> {
    let url = format!("{GAMES_ENDPOINT}?universeIds={universe_id}");
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(8)))
        .build();
    let agent: ureq::Agent = config.into();
    let body = agent.get(&url).call().ok()?.body_mut().read_to_string().ok()?;
    parse_game_name(&body)
}

/// Extract `data[0].name` from the games-endpoint JSON. Pure, so it's unit-tested
/// without the network.
fn parse_game_name(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("data")?
        .get(0)?
        .get("name")?
        .as_str()
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_from_games_response() {
        let json = r#"{"data":[{"id":6035872082,"rootPlaceId":17625359962,"name":"Grow a Garden","description":"x"}]}"#;
        assert_eq!(parse_game_name(json).as_deref(), Some("Grow a Garden"));
    }

    #[test]
    fn missing_or_empty_data_is_none() {
        assert_eq!(parse_game_name(r#"{"data":[]}"#), None);
        assert_eq!(parse_game_name(r#"{"errors":[{"code":1}]}"#), None);
        assert_eq!(parse_game_name("not json"), None);
    }
}
