//! The consumer test mod of the Alaydriem Mod Manager (`@amml/managerprobe`). It uses the manager through
//! `amm-client` only, runs one check after another in the game and logs one line per step. The in-game task reads
//! those lines and passes on `probe ok`.

#![deny(unsafe_code)]

pub mod probe_checks;
pub mod probe_layer;
pub mod probe_mod;
pub mod probe_run;
pub mod probe_step;

pub use probe_checks::ProbeChecks;
pub use probe_layer::ProbeLayer;
pub use probe_mod::ProbeMod;
pub use probe_run::ProbeRun;
pub use probe_step::ProbeStep;

amml_api::amml_mod!(ProbeMod);
