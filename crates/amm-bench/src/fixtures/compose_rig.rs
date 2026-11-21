use amm_client::link::ManagerLink;
use amm_core::structs::descriptor::Capability;

use super::{BenchHost, BenchLoader, DemoSizedHandler, Descriptors};

/// A host with egui clients connected through `amm-client`, each running a demo-sized pass on every layer it holds.
pub struct ComposeRig {
    // declared before the loader and the host so the links drop first
    _links: Vec<ManagerLink>,
    _loader: Option<BenchLoader>,
    host: BenchHost,
}

impl ComposeRig {
    /// A host with no client.
    pub fn idle() -> Self {
        Self { _links: Vec::new(), _loader: None, host: BenchHost::new() }
    }

    /// A host with one client per entry of `layers`, each holding the listed leases (Hud and Overlay).
    pub fn with_clients(layers: &[&[Capability]]) -> Self {
        let host = BenchHost::new();
        let loader = BenchLoader::new();
        let mut links = Vec::with_capacity(layers.len());
        for (n, held) in layers.iter().enumerate() {
            let claims: Vec<(&str, &str)> = held
                .iter()
                .map(|capability| (if *capability == Capability::Hud { "Hud" } else { "Overlay" }, "Shared"))
                .collect();
            // The link needs a static descriptor text; a bench process leaks a few small strings.
            let descriptor: &'static str =
                Box::leak(Descriptors::with_claims(&format!("rig{n}"), &claims).into_boxed_str());
            let dir = host.game_dir().join(format!("rig-config-{n}"));
            std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("rig config dir: {error}"));
            let link = loader.link(descriptor, &dir, DemoSizedHandler::new());
            for capability in *held {
                link.acquire(*capability).unwrap_or_else(|error| panic!("acquire: {error:?}"));
            }
            links.push(link);
        }
        Self { _links: links, _loader: Some(loader), host }
    }

    pub fn host(&self) -> &BenchHost {
        &self.host
    }
}
