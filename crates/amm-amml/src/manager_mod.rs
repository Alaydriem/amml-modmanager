use amml_api::amml_abi::{
    ClientTick, FramePresent, FramePresented, KeyInput, ModsLoaded, MouseGrab, MouseInput, RenderDeviceReset, Shutdown,
    SurfaceLost, SwapChainResize, SwapChainResized, WindowChanged, WindowChar, WindowCursor, WindowFocus,
    WindowPointer, WindowSize, WorldJoined, WorldLeft,
};
use amml_api::{AmmlMod, ApiError, Area, Ctx, DisableCtx, Uses};

use crate::services::ManagerLifecycle;

/// The AMML entry point. Load reads the folders; enable starts the host, provides the five tables and adds the
/// listeners; disable takes them down in the order of `ManagerLifecycle`.
pub struct ManagerMod;

impl ManagerMod {
    /// The module string the loader knows this mod by: `@<NAMESPACE>/<NAME>`.
    pub const MODULE: &'static str = "@amml/modmanager";

    /// What the manager needs from the loader. `host`, `overlay` and `input` must be served; `hud` turns off one
    /// flag write per missing area.
    pub const USES: Uses = amml_api::amml_uses! {
        feature "host" {
            required ClientTick;
            required WorldJoined;
            required WorldLeft;
            required ModsLoaded;
            required area Area::CLIENT_READ;
            optional Shutdown;
            optional area Area::CLIENT_TOP_SCREEN;
        }
        feature "overlay" {
            required FramePresent;
            required SwapChainResize;
            required SwapChainResized;
            required SurfaceLost;
            required WindowPointer;
            required WindowChar;
            required WindowFocus;
            required WindowSize;
            required WindowChanged;
            required WindowCursor;
            required area Area::WINDOW_READ;
            required area Area::OVERLAY_SURFACE;
            optional FramePresented;
            optional RenderDeviceReset;
            optional area Area::WINDOW_REQUEST_SIZE;
        }
        feature "input" {
            required KeyInput;
            required area Area::INPUT_LEASE;
            optional MouseInput;
            optional MouseGrab;
        }
        feature "hud" {
            optional area Area::CLIENT_HUD;
            optional area Area::CLIENT_MOUSE;
        }
    };
}

impl AmmlMod for ManagerMod {
    const NAMESPACE: &'static str = "amml";
    const NAME: &'static str = "modmanager";
    const USES: Uses = ManagerMod::USES;

    fn load(ctx: &Ctx) -> Result<(), ApiError> {
        ManagerLifecycle::load(ctx)
    }

    fn enable(ctx: &Ctx) -> Result<(), ApiError> {
        ManagerLifecycle::installed()?.enable(ctx)
    }

    fn disable(ctx: &Ctx, how: &DisableCtx) {
        if let Ok(lifecycle) = ManagerLifecycle::installed() {
            lifecycle.disable(ctx, how);
        }
    }
}
