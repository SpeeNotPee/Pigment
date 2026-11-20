//! pigments own lil state file. load-modify-save always, never rewrite from scratch,
//! otherwise flipping the active profile wiped the saved url handlers (B12)

use std::collections::BTreeMap;
use std::io;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::paths::PigmentPaths;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub active_profile: Option<String>,
    /// whatever owned each roblox scheme b4 pigment took over, so switch back is honest
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub previous_handlers: BTreeMap<String, String>,
    /// the activity pages discord switch, so it comes back on after a restart
    #[serde(default)]
    pub discord_presence: bool,
    /// keys from a newer pigment i dont know abt. kept so downgrades dont eat em
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl State {
    /// missing or broken file = default. a corrupt state should never brick launching
    pub fn load(paths: &PigmentPaths) -> Self {
        std::fs::read_to_string(paths.state_file())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, paths: &PigmentPaths) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(self)?;
        crate::util::write_atomic(&paths.state_file(), &json)
    }
}
