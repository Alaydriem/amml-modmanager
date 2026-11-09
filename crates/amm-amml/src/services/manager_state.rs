use super::{
    FakeKeySender, FakePointerSender, FrameState, HudControl, KeyState, MenuScreen, MouseGrab, StopLatch, TickService,
};

/// The state the detours, the overlay host, the input sink and the lifecycle share.
pub struct ManagerState {
    latch: StopLatch,
    tick: TickService,
    frame: FrameState,
    keys: KeyState,
    menu: MenuScreen,
    grab: MouseGrab,
    hud: HudControl,
    fake_keys: FakeKeySender,
    fake_pointer: FakePointerSender,
}

impl ManagerState {
    pub const fn new() -> Self {
        Self {
            latch: StopLatch::new(),
            tick: TickService::new(),
            frame: FrameState::new(),
            keys: KeyState::new(),
            menu: MenuScreen::new(),
            grab: MouseGrab::new(),
            hud: HudControl::new(),
            fake_keys: FakeKeySender::new(),
            fake_pointer: FakePointerSender::new(),
        }
    }

    /// A state that lives for the process: the overlay and the detours keep references to it.
    pub fn leaked() -> &'static Self {
        Box::leak(Box::new(Self::new()))
    }

    pub fn latch(&self) -> &StopLatch {
        &self.latch
    }

    pub fn tick(&self) -> &TickService {
        &self.tick
    }

    pub fn frame(&self) -> &FrameState {
        &self.frame
    }

    pub fn keys(&self) -> &KeyState {
        &self.keys
    }

    pub fn menu(&self) -> &MenuScreen {
        &self.menu
    }

    pub fn grab(&self) -> &MouseGrab {
        &self.grab
    }

    pub fn hud(&self) -> &HudControl {
        &self.hud
    }

    pub fn fake_keys(&self) -> &FakeKeySender {
        &self.fake_keys
    }

    pub fn fake_pointer(&self) -> &FakePointerSender {
        &self.fake_pointer
    }
}

impl Default for ManagerState {
    fn default() -> Self {
        Self::new()
    }
}
