//! The Alaydriem Mod Manager as an AMML mod: starts the manager host, ticks it from the game, tells it about
//! worlds, draws its layers through the overlay, and exports `amm_get_api` for the mods it hosts.

#![deny(unsafe_code)]

pub mod api_export;
pub mod dup_guard;
pub mod errors;
pub mod game_hooks;
pub mod manager_mod;
pub mod module_path;
pub mod paths;
pub mod services;
pub mod traits;
pub mod types;

pub use dup_guard::DupGuard;
pub use manager_mod::ManagerMod;
pub use module_path::ModulePath;
pub use paths::ManagerPaths;

amml_sdk::amml_mod!(ManagerMod);
