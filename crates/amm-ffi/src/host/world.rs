use crate::services::{Boundary, HostCell};

/// Game thread. A world finished loading (`true`) or was left (`false`).
pub extern "C" fn amm_host_world(loaded: bool) {
    let _ = Boundary::guard(|| {
        HostCell::global().with(|runtime| {
            runtime.world(loaded);
            Ok(())
        })
    });
}
