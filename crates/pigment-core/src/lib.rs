//! `pigment-core` — all logic for the Pigment Roblox launcher, with no UI.
//!
//! Pigment is a manager/frontend for [Sober], the closed-source Flatpak runtime
//! that runs the Android Roblox client on Linux. This crate never reimplements
//! the runtime; it drives Sober as a black box: reading and safely rewriting its
//! config, composing mods into its asset-overlay directory, discovering and
//! launching it, and parsing its logs.
//!
//! [Sober]: https://sober.vinegarhq.org/

pub mod activity;
pub mod config;
pub mod discord;
pub mod library;
pub mod mods;
pub mod paths;
pub mod profiles;
pub mod protocol;
pub mod roblox_api;
pub mod sober;
mod util;

pub use activity::{Session, Status};
pub use config::{Config, ConfigError};
pub use discord::{Activity, Client as DiscordClient};
pub use library::ModLibrary;
pub use mods::{ApkAssetTree, ComposeReport, Conflict, ModSource};
pub use paths::{PigmentPaths, SoberPaths};
pub use profiles::{ApplyReport, Profile, ProfileError, ProfileStore};
pub use protocol::ProtocolError;
pub use sober::{LaunchSpec, Sober};
