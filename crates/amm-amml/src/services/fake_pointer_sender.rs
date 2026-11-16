use std::sync::atomic::{AtomicBool, AtomicI16, AtomicU32, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use amm_core::services::input::FakePointer;
use amm_core::structs::input::{PointerButton, PointerOp, TurnAxis};
use windows::Win32::System::Threading::GetCurrentThreadId;

use crate::traits::GameCalls;

/// Feeds the fake mouse actions mods queued with `send_pointer` to the game's own `MouseDevice::feed`, past the
/// manager's feed detour, so a fake action is never routed back to the manager and the manager's own block never
/// sees it.
///
/// The `MouseDevice` comes from the `MouseDevice.instance` entry (`set_device`, at enable), so no real mouse event
/// is needed, or else from the last real mouse event (`note`, called from the `feed` detour). With a device from the
/// entry, a real event on another device (the touch and pen devices of the game) changes nothing. The pointer
/// position and the thread come from the last real event on the device; before one, the position is 0, 0.
/// `flush` runs from `ManagerRuntime::tick` after the host tick and takes every queued action each time, so no
/// action reaches the game late:
///
/// - Each flush tells the host whether the feed is ready: a device is known and the tick runs on the thread of the
///   real feed. While it is not, the host refuses a button down and a wheel turn with `NotReady`, and an action
///   that is still taken (a button up) is dropped. A look turn needs no device: the queued turns of a flush add
///   into one `_applyTurnDelta` call of the original, made when `client` has a local player, and dropped otherwise.
/// - `feed` appends to a vector of the device with no lock, so a fake action is fed only from the thread the real
///   feed runs on. The assumption is that `ClientInstance::update` and the `feed` detour share that thread, as the
///   key hooks do for `FakeKeySender`; before the first real event on the device the tick thread is taken as that
///   thread. The first ready flush logs both thread ids once (a feed thread of 0: no real event yet). When they
///   differ, the feed is not ready, and one warning is logged.
pub struct FakePointerSender {
    device: AtomicUsize,
    // the device came from the entry, so `note` keeps it
    from_entry: AtomicBool,
    // 0 until a real event on the device
    thread: AtomicU32,
    x: AtomicI16,
    y: AtomicI16,
    device_logged: AtomicBool,
    threads_logged: AtomicBool,
    mismatch_logged: AtomicBool,
    // reused between ticks; one tick never holds more than the host queue, which the host bounds
    ops: Mutex<Vec<PointerOp>>,
}

impl FakePointerSender {
    /// The `actionButtonId` of `MouseDevice::feed` for the left button (`MouseDevice.feed` notes, recipe 2 call
    /// site `mov dl,1`; `MouseDevice.h`).
    pub const LEFT_BUTTON: u8 = 1;
    /// The `actionButtonId` for the right button.
    pub const RIGHT_BUTTON: u8 = 2;
    /// The `actionButtonId` for the middle button.
    pub const MIDDLE_BUTTON: u8 = 3;
    /// The `actionButtonId` for the wheel, which `feed` handles apart from the buttons.
    pub const WHEEL_BUTTON: u8 = 4;
    /// The `buttonData` of one wheel notch away from the player: the Windows `WHEEL_DELTA`. A notch toward the
    /// player is its negative.
    pub const WHEEL_NOTCH: i8 = 120;
    /// The info line, written once, at the first flush with a ready feed. Its `from_entry` field says whether the
    /// device came from the `MouseDevice.instance` entry, so fake buttons need no real mouse event.
    pub const DEVICE_LINE: &'static str = "fake mouse actions reach the game, the mouse device is known";
    const MILLIDEGREES: f32 = 1000.0;

    pub const fn new() -> Self {
        Self {
            device: AtomicUsize::new(0),
            from_entry: AtomicBool::new(false),
            thread: AtomicU32::new(0),
            x: AtomicI16::new(0),
            y: AtomicI16::new(0),
            device_logged: AtomicBool::new(false),
            threads_logged: AtomicBool::new(false),
            mismatch_logged: AtomicBool::new(false),
            ops: Mutex::new(Vec::new()),
        }
    }

    /// The game's `MouseDevice` at `device`, from the `MouseDevice.instance` entry. 0 changes nothing.
    pub fn set_device(&self, device: usize) {
        if device != 0 {
            self.from_entry.store(true, Ordering::Relaxed);
            // the release store pairs with the acquire load in `flush`
            self.device.store(device, Ordering::Release);
        }
    }

    /// A real mouse event reached `device` with the pointer at `x`, `y` in the pixels of the game's feed, on the
    /// calling thread. With a device from the entry, an event on another device is not noted.
    pub fn note(&self, device: usize, x: i16, y: i16) {
        if device == 0 || (self.from_entry.load(Ordering::Relaxed) && self.device.load(Ordering::Relaxed) != device) {
            return;
        }
        self.thread.store(Self::current_thread(), Ordering::Relaxed);
        self.x.store(x, Ordering::Relaxed);
        self.y.store(y, Ordering::Relaxed);
        // the release store pairs with the acquire load in `flush`, so a device comes with its thread
        self.device.store(device, Ordering::Release);
    }

    /// Tells the host whether the feed is ready and takes every queued action from the host. With a ready feed the
    /// actions are fed, oldest first; otherwise they are dropped. A wheel turn is one feed per notch, at most
    /// `FakePointer::MAX_WHEEL_STEPS`. The look turns of the batch are applied first as one call to the game's turn
    /// with the summed pitch and yaw, when `client` is not 0 and has a local player, in both states.
    pub fn flush(&self, calls: &dyn GameCalls, client: usize) {
        let device = self.device.load(Ordering::Acquire);
        let ready = device != 0 && self.on_feed_thread();
        let mut ops = self.ops.lock().unwrap_or_else(PoisonError::into_inner);
        ops.clear();
        calls.take_fake_pointer(ready, &mut ops);
        if ready && !self.device_logged.load(Ordering::Relaxed) && !self.device_logged.swap(true, Ordering::Relaxed) {
            curia::info!("fake mouse actions reach the game, the mouse device is known", {
                device: device,
                from_entry: self.from_entry.load(Ordering::Relaxed),
            });
        }
        if ops.is_empty() {
            return;
        }
        Self::apply_turns(calls, client, &ops);
        if !ready {
            return;
        }
        let (x, y) = (self.x.load(Ordering::Relaxed), self.y.load(Ordering::Relaxed));
        for op in ops.iter() {
            match *op {
                PointerOp::Button { button, down } => {
                    calls.game_mouse(device, Self::button_id(button), i8::from(down), x, y);
                }
                PointerOp::Turn { .. } => {}
                PointerOp::Wheel { steps } => {
                    let notch = if steps < 0 { -Self::WHEEL_NOTCH } else { Self::WHEEL_NOTCH };
                    for _ in 0..steps.unsigned_abs().min(FakePointer::MAX_WHEEL_STEPS.unsigned_abs()) {
                        calls.game_mouse(device, Self::WHEEL_BUTTON, notch, x, y);
                    }
                }
            }
        }
    }

    // one call of the original turn for the summed look turns of `ops`; none when there is no turn, no client or no
    // local player
    fn apply_turns(calls: &dyn GameCalls, client: usize, ops: &[PointerOp]) {
        let (mut pitch, mut yaw) = (0i64, 0i64);
        for op in ops {
            if let PointerOp::Turn { axis, millidegrees } = *op {
                match axis {
                    TurnAxis::Pitch => pitch += i64::from(millidegrees),
                    TurnAxis::Yaw => yaw += i64::from(millidegrees),
                }
            }
        }
        if (pitch == 0 && yaw == 0) || client == 0 {
            return;
        }
        let player = calls.local_player(client);
        if player != 0 {
            calls.game_turn(player, pitch as f32 / Self::MILLIDEGREES, yaw as f32 / Self::MILLIDEGREES);
        }
    }

    // true when no real event has set a feed thread, or the calling thread is that thread; logs both ids once, and a
    // mismatch once
    fn on_feed_thread(&self) -> bool {
        let (feed, tick) = (self.thread.load(Ordering::Relaxed), Self::current_thread());
        let same = feed == 0 || feed == tick;
        if same && !self.threads_logged.load(Ordering::Relaxed) && !self.threads_logged.swap(true, Ordering::Relaxed) {
            curia::info!("fake mouse actions are fed from the game tick", { feed_thread: feed, tick_thread: tick });
        }
        if same {
            return true;
        }
        if !self.mismatch_logged.swap(true, Ordering::Relaxed) {
            curia::warn!("the mouse feed runs on another thread than the game tick, fake mouse actions are dropped", {
                feed_thread: feed,
                tick_thread: tick,
            });
        }
        false
    }

    #[allow(unsafe_code)]
    fn current_thread() -> u32 {
        // SAFETY: no input
        unsafe { GetCurrentThreadId() }
    }

    fn button_id(button: PointerButton) -> u8 {
        match button {
            PointerButton::Left => Self::LEFT_BUTTON,
            PointerButton::Right => Self::RIGHT_BUTTON,
            PointerButton::Middle => Self::MIDDLE_BUTTON,
        }
    }
}

impl Default for FakePointerSender {
    fn default() -> Self {
        Self::new()
    }
}
