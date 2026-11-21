//! The Alaydriem Mod Manager as an AMML v3 mod: it starts the manager host, ticks it from `ClientTick`, tells it
//! about worlds, routes the keys of the game to it, writes the HUD and mouse flags the layers ask for, and gives
//! its five tables to the mods it hosts through the AMML registry. It uses only `amml-api` for the game.

#![deny(unsafe_code)]

pub mod errors;
pub mod manager_mod;
pub mod paths;
pub mod services;
pub mod types;

pub use manager_mod::ManagerMod;
pub use paths::ManagerPaths;

amml_api::amml_mod!(ManagerMod);
