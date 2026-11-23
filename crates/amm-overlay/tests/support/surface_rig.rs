use std::sync::{Arc, Mutex, MutexGuard};

use amm_ffi::types::AmmHostFrameInput;
use amm_overlay::SurfaceRenderer;
use amml_abi::{AmmlDrawSurface, AmmlSurfaceLoss, FramePresent, SurfaceLost};
use amml_api::testing::FakeGame;
use amml_api::{Area, FramePresentExt, HookOrder, Listener};

use super::rig_outcome::RigOutcome;
use super::rig_request::RigRequest;
use super::test_host::TestHost;

/// A `SurfaceRenderer` driven as in the game: a `FramePresent` listener draws on the surface `FakeGame` lends it,
/// and a `SurfaceLost` listener calls `surface_lost`.
pub struct SurfaceRig {
    pub fake: FakeGame,
    pub host: &'static TestHost,
    renderer: Arc<Mutex<SurfaceRenderer>>,
    request: Arc<Mutex<RigRequest>>,
    outcome: Arc<Mutex<Option<RigOutcome>>>,
    _present: Listener,
    _lost: Listener,
}

impl SurfaceRig {
    pub fn new() -> Self {
        Self::with(None)
    }

    /// A rig on a loader that makes the surface on demand: the listener gets a waiting surface (no context, target
    /// or back buffer) and `surface` is made for it on its first ask, which the fake counts.
    pub fn on_demand(surface: AmmlDrawSurface) -> Self {
        Self::with(Some(surface))
    }

    /// The surface a loader that makes it on demand lends before a listener asks: no context, target or back buffer.
    pub fn waiting(surface: AmmlDrawSurface) -> AmmlDrawSurface {
        AmmlDrawSurface {
            context: std::ptr::null_mut(),
            target: std::ptr::null_mut(),
            back_buffer: std::ptr::null_mut(),
            ..surface
        }
    }

    fn with(on_demand: Option<AmmlDrawSurface>) -> Self {
        let fake = FakeGame::new();
        fake.offer::<FramePresent>();
        fake.offer::<SurfaceLost>();
        fake.offer_area(Area::OVERLAY_SURFACE);
        if let Some(surface) = on_demand {
            fake.offer_surface_on_demand();
            fake.set_prepared_surface(surface);
        }
        let host = TestHost::leaked();
        let renderer = Arc::new(Mutex::new(SurfaceRenderer::new(host)));
        let request = Arc::new(Mutex::new(RigRequest::Draw));
        let outcome = Arc::new(Mutex::new(None));
        let events = fake.ctx().events();
        let (drawing, asked, seen) = (renderer.clone(), request.clone(), outcome.clone());
        let present = events
            .listen::<FramePresent>(HookOrder::NORMAL, move |event, _| {
                let result = match event.surface() {
                    None => RigOutcome::NoSurface,
                    Some(surface) => {
                        let mut renderer = drawing.lock().unwrap();
                        match *asked.lock().unwrap() {
                            RigRequest::Draw => RigOutcome::Drawn(renderer.draw(&surface, &Self::input(&surface))),
                            RigRequest::DrawMods => {
                                RigOutcome::Drawn(renderer.draw_mods(&surface, &Self::input(&surface)))
                            }
                            RigRequest::DrawManager => RigOutcome::Drawn(renderer.draw_manager(&surface)),
                            RigRequest::RefreshPicture => RigOutcome::Picture(
                                renderer.refresh_game_picture(&surface).map_err(|error| error.to_string()),
                            ),
                        }
                    }
                };
                *seen.lock().unwrap() = Some(result);
            })
            .unwrap();
        let losing = renderer.clone();
        let lost =
            events.listen::<SurfaceLost>(HookOrder::NORMAL, move |_, _| losing.lock().unwrap().surface_lost()).unwrap();
        Self { fake, host, renderer, request, outcome, _present: present, _lost: lost }
    }

    /// Raises one `FramePresent` of `surface` whose listener draws, and gives what it saw.
    pub fn draw(&self, surface: AmmlDrawSurface) -> RigOutcome {
        self.present(RigRequest::Draw, surface)
    }

    /// Raises one `FramePresent` of `surface` whose listener draws only the mod layers, and gives what it saw.
    pub fn draw_mods(&self, surface: AmmlDrawSurface) -> RigOutcome {
        self.present(RigRequest::DrawMods, surface)
    }

    /// Raises one `FramePresent` of `surface` whose listener draws only the manager layers, and gives what it saw.
    pub fn draw_manager(&self, surface: AmmlDrawSurface) -> RigOutcome {
        self.present(RigRequest::DrawManager, surface)
    }

    /// Raises one `FramePresent` of `surface` whose listener refreshes the game picture, and gives what it saw.
    pub fn refresh_picture(&self, surface: AmmlDrawSurface) -> RigOutcome {
        self.present(RigRequest::RefreshPicture, surface)
    }

    /// Raises `SurfaceLost` for `epoch`.
    pub fn lose(&self, epoch: u64) {
        self.fake.raise(&mut SurfaceLost {
            header: FakeGame::header::<SurfaceLost>(),
            reason: AmmlSurfaceLoss::DEVICE_CHANGED,
            _reserved: [0; 7],
            device_epoch: epoch,
        });
    }

    pub fn renderer(&self) -> MutexGuard<'_, SurfaceRenderer> {
        self.renderer.lock().unwrap()
    }

    fn present(&self, request: RigRequest, surface: AmmlDrawSurface) -> RigOutcome {
        *self.request.lock().unwrap() = request;
        self.outcome.lock().unwrap().take();
        self.fake.raise(&mut FramePresent {
            header: FakeGame::header::<FramePresent>(),
            frame_id: 1,
            sync_interval: 1,
            flags: 0,
            surface,
        });
        self.outcome.lock().unwrap().take().unwrap()
    }

    fn input(surface: &amml_api::DrawSurface<'_>) -> AmmHostFrameInput {
        let size = surface.size();
        AmmHostFrameInput {
            width: size.width,
            height: size.height,
            pixels_per_point: 1.0,
            time_secs: 0.0,
            now_ms: 0,
            menu_screen: 0,
            format: surface.format(),
            client_width: size.width,
            client_height: size.height,
        }
    }
}
