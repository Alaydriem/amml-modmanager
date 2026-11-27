use std::path::Path;

use amm_client::link::{FrameHandler, ManagerLink};
use amm_core::structs::descriptor::Descriptor;
use amm_ffi::api::ManagerTables;
use amml_api::amml_abi::{AmmlDirKind, ProviderGone, ProviderReady};
use amml_api::testing::FakeGame;

/// A fake AMML loader that holds the manager's five tables (the host the `BenchHost` started serves them), so a
/// bench links a mod through `amm-client` and the registry as the game does.
pub struct BenchLoader {
    game: FakeGame,
}

impl BenchLoader {
    /// A loader with the five tables provided. Start the host first.
    pub fn new() -> Self {
        let game = FakeGame::new();
        game.offer::<ProviderReady>();
        game.offer::<ProviderGone>();
        let provided = ManagerTables::provide_all(&game.ctx().registry());
        provided.unwrap_or_else(|error| panic!("provide the manager tables: {error:?}"));
        Self { game }
    }

    /// Starts a link for a mod whose config folder is `config_dir`, drawing with `handler`. The tables are there, so
    /// the link is connected when this returns.
    pub fn link<H: FrameHandler + Send + 'static>(
        &self,
        descriptor: &'static str,
        config_dir: &Path,
        handler: H,
    ) -> ManagerLink {
        self.game.set_dir(AmmlDirKind::CONFIG, &config_dir.display().to_string());
        let id = hcl::from_str::<Descriptor>(descriptor).map(|parsed| parsed.id()).unwrap_or_default();
        let ctx = self.game.ctx_named(Box::leak(id.into_boxed_str()));
        ManagerLink::start(&ctx, descriptor, Some(handler)).unwrap_or_else(|error| panic!("start the link: {error:?}"))
    }
}

impl Default for BenchLoader {
    fn default() -> Self {
        Self::new()
    }
}
