#![allow(unsafe_code)]

use amm_abi::types::AMM_ABI_MAJOR;
use amm_client::link::{ApiTable, ManagerLink};
use amm_core::structs::descriptor::Capability;
use amm_ffi::api::amm_api_for;

use super::{BenchHost, DemoSizedHandler, Descriptors};

/// A host with egui clients connected through `amm-client`, each running a demo-sized pass on every layer it holds.
pub struct ComposeRig {
    // declared before the host so the links drop first
    _links: Vec<ManagerLink>,
    host: BenchHost,
}

impl ComposeRig {
    /// A host with no client.
    pub fn idle() -> Self {
        Self { host: BenchHost::new(), _links: Vec::new() }
    }

    /// A host with one client per entry of `layers`, each holding the listed leases (Hud and Overlay).
    pub fn with_clients(layers: &[&[Capability]]) -> Self {
        let host = BenchHost::new();
        let table_ptr = amm_api_for(AMM_ABI_MAJOR);
        let mut links = Vec::with_capacity(layers.len());
        for (n, held) in layers.iter().enumerate() {
            // SAFETY: amm_api_for returns null or the static table of the manager
            let table = unsafe { ApiTable::new(table_ptr) }.unwrap_or_else(|error| panic!("api table: {error:?}"));
            let claims: Vec<(&str, &str)> = held
                .iter()
                .map(|capability| (if *capability == Capability::Hud { "Hud" } else { "Overlay" }, "Shared"))
                .collect();
            // The link needs a static descriptor text; a bench process leaks a few small strings.
            let descriptor: &'static str =
                Box::leak(Descriptors::with_claims(&format!("rig{n}"), &claims).into_boxed_str());
            let dir = host.game_dir().join(format!("rig-config-{n}"));
            std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("rig config dir: {error}"));
            let link = ManagerLink::new(table, descriptor, &dir, DemoSizedHandler::new())
                .unwrap_or_else(|error| panic!("register: {error:?}"));
            for capability in *held {
                link.acquire(*capability).unwrap_or_else(|error| panic!("acquire: {error:?}"));
            }
            links.push(link);
        }
        Self { host, _links: links }
    }

    pub fn host(&self) -> &BenchHost {
        &self.host
    }
}
