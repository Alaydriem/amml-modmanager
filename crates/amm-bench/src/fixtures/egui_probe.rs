use amm_client::link::ManagerLink;
use amm_core::structs::descriptor::Capability;

use super::{BenchHost, BenchLoader, RectHandler};

/// One mod connected through `amm-client` with a trivial egui handler and an Overlay lease, to count what one egui
/// pass costs in allocations.
pub struct EguiProbe {
    // declared before the loader and the host so the link drops first
    _link: ManagerLink,
    _loader: BenchLoader,
    host: BenchHost,
}

impl EguiProbe {
    const DESCRIPTOR: &'static str = "namespace = \"test\"\nname = \"probe\"\ndisplay_name = \"probe\"\nversion = \"0.1.0\"\nauthor = \"Bench\"\n\
        config_path = \"config.hcl\"\nclaim \"Overlay\" {\n  mode = \"Shared\"\n  reason = \"bench\"\n}\n";

    pub fn new() -> Self {
        let host = BenchHost::new();
        let loader = BenchLoader::new();
        let dir = host.game_dir().join("probe-config");
        std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("probe config dir: {error}"));
        let link = loader.link(Self::DESCRIPTOR, &dir, RectHandler);
        link.acquire(Capability::Overlay).unwrap_or_else(|error| panic!("acquire: {error:?}"));
        Self { _link: link, _loader: loader, host }
    }

    pub fn host(&self) -> &BenchHost {
        &self.host
    }
}

impl Default for EguiProbe {
    fn default() -> Self {
        Self::new()
    }
}
