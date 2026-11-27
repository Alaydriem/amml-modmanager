use std::sync::Arc;
use std::sync::atomic::AtomicU8;

use amm_abi::types::AmmStatus;
use amm_ffi::api::ManagerTables;
use amm_ffi::services::HostCell;
use amml_api::amml_abi::AmmlStatus;
use amml_api::{ApiError, Ctx, DisableCtx};

use super::{
    ConsoleMirror, ManagerClock, ManagerInputSink, ManagerListeners, ManagerOverlayHost, ManagerRuntime, ManagerState,
    TickService,
};
use crate::paths::ManagerPaths;

/// The order the manager starts and stops in. Load reads the folders and keeps the runtime; it starts no thread, so
/// it stays free of side effects before the game's static initialisers. Enable starts the host (and with it the
/// config writer thread) when none runs, so the tables have a host to serve from their first call, provides the
/// five tables, pushes the lease ranking and adds the listeners. Disable closes the tables first (a late call from a
/// consumer gets `Disabled`), ends the panel lease, drops the listeners, leaves the client flags to the loader
/// (it writes back the HUD and the mouse grab of a disabled mod), and stops the host last.
#[derive(Clone, Copy)]
pub struct ManagerLifecycle {
    runtime: &'static ManagerRuntime,
    cell: &'static HostCell,
}

impl ManagerLifecycle {
    pub fn new(runtime: &'static ManagerRuntime, cell: &'static HostCell) -> Self {
        Self { runtime, cell }
    }

    /// The lifecycle over the runtime `load` installed; `NotReady` when load did not run.
    pub fn installed() -> Result<Self, ApiError> {
        ManagerRuntime::get().map(|runtime| Self::new(runtime, HostCell::global())).ok_or(ApiError::NotReady)
    }

    /// Reads the manager folders and keeps the runtime with its overlay host. A missing config or data folder is
    /// `NotAvailable`, with a log line. No thread starts here.
    pub fn load(ctx: &Ctx) -> Result<(), ApiError> {
        let paths = ManagerPaths::from_dirs(&ctx.dirs()).map_err(|error| {
            ctx.log().error("manager folders missing", &[("error", &error.to_string())]);
            ApiError::NotAvailable
        })?;
        let menu = Arc::new(AtomicU8::new(TickService::MENU_NONE));
        let overlay = ManagerOverlayHost::new(HostCell::global(), *ctx, Arc::clone(&menu));
        ManagerRuntime::install(ManagerRuntime::new(paths, ManagerClock::new(), menu, overlay));
        Ok(())
    }

    /// A failure after the host started stops it again, with the console mirror and the tables, as `disable` does:
    /// the loader runs no `disable` for a mod whose `enable` failed.
    pub fn enable(&self, ctx: &Ctx) -> Result<(), ApiError> {
        if self.cell.shared().is_none() {
            self.start_host(ctx)?;
        }
        let enabled = self.enable_on_host(ctx);
        if enabled.is_err() {
            self.stop_host();
        }
        enabled
    }

    fn enable_on_host(&self, ctx: &Ctx) -> Result<(), ApiError> {
        ConsoleMirror::start(ctx.log());
        self.cell.set_console(Some(ConsoleMirror::write));
        ManagerTables::provide_all(&ctx.registry())?;
        let runtime = self.runtime;
        let sink = ManagerInputSink::new(self.cell);
        let state = Arc::new(ManagerState::new(ctx, sink, runtime.clock(), runtime.menu(), runtime.overlay()));
        state.sync_ranking();
        let listeners = ManagerListeners::install(ctx, &state, runtime.overlay(), runtime.clock())?;
        runtime.activate(state, listeners);
        ctx.log().info("manager enabled", &[]);
        Ok(())
    }

    pub fn disable(&self, ctx: &Ctx, how: &DisableCtx) {
        let _ = self.cell.with(|runtime| {
            runtime.disable_tables();
            Ok(())
        });
        let (state, listeners) = self.runtime.deactivate();
        if how.game_alive
            && let Some(state) = state.as_ref()
        {
            // a loss held back under the open panel would never reach the game: the manager listens no more
            state.replay_held_focus();
        }
        drop(state.as_ref().and_then(|state| state.take_panel_lease()));
        drop(listeners);
        drop(state);
        if how.game_alive && how.client().is_some() {
            ctx.log().debug("client flags left to the loader", &[]);
        }
        self.stop_host();
    }

    // Closes the tables, stops the console mirror, and stops the host with its writer thread.
    fn stop_host(&self) {
        let _ = self.cell.with(|runtime| {
            runtime.disable_tables();
            Ok(())
        });
        self.cell.set_console(None);
        ConsoleMirror::stop();
        self.cell.destroy();
    }

    fn start_host(&self, ctx: &Ctx) -> Result<(), ApiError> {
        let paths = self.runtime.paths();
        let created = self.cell.create(paths.config_dir.clone(), paths.data_dir.clone(), paths.version.clone());
        created.map_err(|status: AmmStatus| {
            ctx.log().error("manager host did not start", &[("status", &format!("{status:?}"))]);
            ApiError::Loader(AmmlStatus::FAILED)
        })
    }
}
