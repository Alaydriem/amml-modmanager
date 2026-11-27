use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use amm_abi::types::AmmCoreTable;
use amm_client::link::{LinkState, ManagerLink, PointerOp};
use amm_core::errors::LinkError;
use amm_core::structs::capture::ScreenshotLayers;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::{KeybindPhase, ModEvent, ReleaseReason};
use amm_core::structs::lease::AcquireOutcome;
use amml_api::{
    ApiError, Ctx, InputBlocks, InputLease, KeyCode, LeaseMode, LeaseRequest, Look, MouseAction, MouseButton,
    MoveIntent, Provided,
};

use crate::{ProbeLayer, ProbeRun, ProbeStep};

/// The probe's checks in order. `tick` runs once per `ClientTick`: it reads the events of the link, then does the
/// action of the current step and moves on when the step's line is logged. The input listeners of `ProbeMod` call the
/// recording methods. Each step logs one line; the in-game task reads them.
pub struct ProbeChecks {
    ctx: Ctx,
    frames: Arc<AtomicU64>,
    link: Mutex<Option<ManagerLink>>,
    run: Mutex<ProbeRun>,
    full: Mutex<Option<InputLease>>,
}

impl ProbeChecks {
    /// Ticks the key hold lasts.
    pub const HOLD_FRAMES: u32 = 40;
    /// Ticks in play the key hold waits before it presses W: the game changes its HUD screens just after the world
    /// opens and lets go of the keys it holds when it does.
    pub const SETTLE_TICKS: u32 = 120;
    /// Ticks the FULL input lease lasts.
    pub const FULL_FRAMES: u32 = 60;
    // Ticks the probe waits for the release event of a world leave before it reports `released=false`.
    const RELEASE_WAIT: u32 = 120;
    // The `probe.count` value the setting check writes.
    const COUNT: i64 = 7;
    // The yaw of the turn check, in degrees, and the least sum that passes it.
    const TURN_YAW: f32 = 90.0;
    const TURN_SEEN: f32 = 89.5;

    pub fn new(ctx: Ctx, frames: Arc<AtomicU64>, link: ManagerLink) -> Self {
        Self { ctx, frames, link: Mutex::new(Some(link)), run: Mutex::new(ProbeRun::default()), full: Mutex::new(None) }
    }

    pub fn new_shared(ctx: Ctx, frames: Arc<AtomicU64>, link: ManagerLink) -> Arc<Self> {
        Arc::new(Self::new(ctx, frames, link))
    }

    /// The step the probe is in.
    pub fn step(&self) -> ProbeStep {
        self.run().step
    }

    /// Reads the link's events, then does the action of the current step.
    pub fn tick(&self) {
        let events: Vec<ModEvent> = match self.link().as_mut() {
            Some(link) => link.events().collect(),
            None => Vec::new(),
        };
        let mut run = self.run();
        for event in events {
            self.on_event(&mut run, event);
        }
        self.advance(&mut run);
    }

    /// The game joined a world.
    pub fn world_joined(&self) {
        let mut run = self.run();
        run.joined = true;
        if run.has_left {
            run.has_left = false;
            self.ctx.log().info("probe world joined again", &[]);
        }
    }

    /// The game left the world. After `probe ok` the probe waits for the release of the key it holds.
    pub fn world_left(&self) {
        let mut run = self.run();
        run.joined = false;
        run.in_play = false;
        run.settle_ticks = 0;
        if run.step == ProbeStep::Done {
            run.has_left = true;
            run.left = true;
            run.left_ticks = 0;
        }
    }

    /// A `PlayerMoveInput` with the intent the game would read. The game raises it only in play, so the first one
    /// after the join marks the end of the loading screen.
    pub fn move_input(&self, intent: MoveIntent) {
        let mut run = self.run();
        if run.joined {
            run.in_play = true;
        }
        match run.step {
            ProbeStep::KeyHold if intent.forward > 0.0 => run.moved = true,
            ProbeStep::Move if intent.forward >= 1.0 => run.move_seen = true,
            _ => {}
        }
    }

    /// A `LookDelta` with its yaw, whether a mod injected it and whether it was cancelled. The loader raises the
    /// summed turn of all mods as one injected event with no single injector, so the turn step counts any injected
    /// `LookDelta`.
    pub fn look_delta(&self, yaw: f32, injected: bool, cancelled: bool) {
        let mut run = self.run();
        match run.step {
            ProbeStep::Turn if injected => run.yaw += yaw,
            ProbeStep::FullLease if !injected && cancelled => run.look_blocked = true,
            _ => {}
        }
    }

    /// A `MouseInput` the loader raised.
    pub fn mouse_input(&self, action: MouseAction, injected: bool) {
        if !injected {
            return;
        }
        let mut run = self.run();
        match action {
            MouseAction::Wheel { notches } => run.wheel_notches = Some(notches),
            MouseAction::Button { button: MouseButton::Left, down: true } => run.click_seen = true,
            _ => {}
        }
    }

    /// A `GamepadPoll`, `cancelled` when the loader skips the poll.
    pub fn gamepad_poll(&self, cancelled: bool) {
        let mut run = self.run();
        if run.step == ProbeStep::FullLease && cancelled {
            run.gamepad_skipped += 1;
        }
    }

    /// A provider is going. When it is the manager's core table, the probe logs it and calls the manager once more.
    pub fn provider_gone(&self, name: &str) {
        if name != AmmCoreTable::NAME {
            return;
        }
        let log = self.ctx.log();
        log.info("probe provider gone", &[]);
        let result = match self.link().as_ref() {
            Some(link) => link.set_setting("probe.count", &hcl::Value::from(0_i64)),
            None => Err(LinkError::NotConnected),
        };
        let text = match result {
            Err(LinkError::Gone) => "Gone".to_owned(),
            Ok(()) => "Ok".to_owned(),
            Err(error) => format!("{error:?}"),
        };
        log.info("probe call after gone", &[("result", &text)]);
    }

    fn run(&self) -> MutexGuard<'_, ProbeRun> {
        self.run.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn link(&self) -> MutexGuard<'_, Option<ManagerLink>> {
        self.link.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn with_link<R>(&self, f: impl FnOnce(&ManagerLink) -> R) -> Option<R> {
        self.link().as_ref().map(f)
    }

    fn warn(&self, message: &str, error: &dyn std::fmt::Debug) {
        self.ctx.log().warn(message, &[("error", &format!("{error:?}"))]);
    }

    fn on_event(&self, run: &mut ProbeRun, event: ModEvent) {
        let log = self.ctx.log();
        match event {
            ModEvent::Keybind { action, phase: KeybindPhase::Pressed } => {
                log.info("probe keybind fired", &[("index", &action.to_string())]);
            }
            ModEvent::SettingsChanged { .. } => self.settings_changed(run),
            ModEvent::Granted(Capability::Hud) if run.step == ProbeStep::Hud => self.hud_granted(run),
            ModEvent::FakeKeysReleased { reason } => {
                log.info("probe injected released", &[("reason", &format!("{reason:?}"))]);
                if reason == ReleaseReason::FocusLost && run.step == ProbeStep::Done {
                    run.rehold = true;
                }
                if reason == ReleaseReason::WorldLeft && run.left {
                    run.left = false;
                    log.info("probe world left", &[("released", "true")]);
                }
            }
            ModEvent::ScreenshotWritten { id, ok } => {
                if let Some(at) = run.shots.iter().position(|(shot, _, _)| *shot == id) {
                    let (_, layers, path) = run.shots.remove(at);
                    log.info(
                        "probe screenshot written",
                        &[("layers", layers), ("ok", &ok.to_string()), ("path", &path.display().to_string())],
                    );
                }
            }
            _ => {}
        }
    }

    fn advance(&self, run: &mut ProbeRun) {
        match run.step {
            ProbeStep::Link => self.link_step(run),
            ProbeStep::Layer => {
                if self.frames.load(Ordering::Relaxed) >= ProbeLayer::TARGET_FRAMES {
                    run.enter(ProbeStep::Setting);
                }
            }
            ProbeStep::Setting => self.setting_step(run),
            ProbeStep::Hud => self.hud_step(run),
            ProbeStep::KeyHold => self.key_hold_step(run),
            ProbeStep::Move => self.move_step(run),
            ProbeStep::Turn => self.turn_step(run),
            ProbeStep::Mouse => self.mouse_step(run),
            ProbeStep::FullLease => self.full_lease_step(run),
            ProbeStep::Screenshot => self.screenshot_step(run),
            ProbeStep::Done => self.done_step(run),
        }
    }

    fn link_step(&self, run: &mut ProbeRun) {
        if self.with_link(|link| link.state()) != Some(LinkState::Connected) {
            return;
        }
        self.ctx.log().info("probe linked", &[]);
        if let Some(Err(error)) = self.with_link(|link| link.acquire(Capability::Overlay)) {
            self.warn("probe overlay lease failed", &error);
        }
        run.enter(ProbeStep::Layer);
    }

    fn setting_step(&self, run: &mut ProbeRun) {
        if !run.entered {
            run.entered = true;
            let value = hcl::Value::from(Self::COUNT);
            match self.with_link(|link| link.set_setting("probe.count", &value)) {
                // The manager sends no `SettingsChanged` to the mod that made the change; its saved document shows it
                Some(Ok(())) if self.count_saved() => self.settings_changed(run),
                Some(Err(error)) => self.warn("probe setting failed", &error),
                _ => {}
            }
        }
        if run.settings_changed {
            run.enter(ProbeStep::Hud);
        }
    }

    fn settings_changed(&self, run: &mut ProbeRun) {
        if !run.settings_changed {
            run.settings_changed = true;
            self.ctx.log().info("probe settings changed", &[]);
        }
    }

    // Whether the manager's saved document of this mod holds the value the setting check wrote.
    fn count_saved(&self) -> bool {
        let saved = self.with_link(|link| link.config::<hcl::Value>());
        let count = match &saved {
            Some(Ok(hcl::Value::Object(root))) => match root.get("probe") {
                Some(hcl::Value::Object(section)) => section.get("count"),
                _ => root.get("probe.count"),
            },
            _ => None,
        };
        count.and_then(hcl::Value::as_i64) == Some(Self::COUNT)
    }

    fn hud_step(&self, run: &mut ProbeRun) {
        if run.entered {
            return;
        }
        run.entered = true;
        match self.with_link(|link| link.acquire(Capability::Hud)) {
            Some(Ok(AcquireOutcome::Granted)) => self.hud_granted(run),
            other => self.warn("probe hud lease not granted", &other),
        }
    }

    fn hud_granted(&self, run: &mut ProbeRun) {
        self.ctx.log().info("probe hud lease granted", &[]);
        run.enter(ProbeStep::KeyHold);
    }

    fn acquire_input(&self) {
        if let Some(Err(error)) = self.with_link(|link| link.acquire(Capability::Input)) {
            self.warn("probe input lease failed", &error);
        }
    }

    // Takes the input lease and presses W. Gives false while the loader's key feed is not ready, so the caller tries
    // again at the next tick; any other refusal is logged and counts as done.
    fn press_w(&self, run: &mut ProbeRun) -> bool {
        self.acquire_input();
        match self.with_link(|link| link.send_key(KeyCode::W, true)) {
            Some(Err(LinkError::Api(ApiError::NotReady))) => {
                if !run.key_waiting {
                    run.key_waiting = true;
                    self.ctx.log().info("probe key waits for the key feed", &[]);
                }
                false
            }
            Some(Err(error)) => {
                self.warn("probe key failed", &error);
                true
            }
            _ => {
                run.key_waiting = false;
                true
            }
        }
    }

    fn send_key(&self, key: KeyCode, down: bool) {
        if let Some(Err(error)) = self.with_link(|link| link.send_key(key, down)) {
            self.warn("probe key failed", &error);
        }
    }

    // The world join comes while the loading screen still shows, and a key held then never moves the player, so the
    // hold starts `SETTLE_TICKS` after the game first reads the movement.
    fn key_hold_step(&self, run: &mut ProbeRun) {
        if !run.joined || !run.in_play {
            return;
        }
        if run.settle_ticks < Self::SETTLE_TICKS {
            run.settle_ticks += 1;
            return;
        }
        if !run.entered {
            if !self.press_w(run) {
                return;
            }
            run.entered = true;
        }
        run.ticks += 1;
        if run.ticks >= Self::HOLD_FRAMES {
            self.send_key(KeyCode::W, false);
            let moved = run.moved.to_string();
            self.ctx
                .log()
                .info("probe key hold done", &[("frames", &Self::HOLD_FRAMES.to_string()), ("moved", &moved)]);
            run.enter(ProbeStep::Move);
        }
    }

    fn move_step(&self, run: &mut ProbeRun) {
        if !run.entered {
            run.entered = true;
            let intent = MoveIntent { forward: 1.0, ..MoveIntent::default() };
            if let Some(Err(error)) = self.with_link(|link| link.send_move(intent)) {
                self.warn("probe move failed", &error);
            }
        }
        if run.move_seen {
            let _ = self.with_link(|link| link.send_move(MoveIntent::default()));
            self.ctx.log().info("probe move seen", &[("forward", "1")]);
            run.enter(ProbeStep::Turn);
        }
    }

    fn turn_step(&self, run: &mut ProbeRun) {
        if !run.entered {
            run.entered = true;
            let turn = PointerOp::Turn(Look { yaw: Self::TURN_YAW, pitch: 0.0 });
            if let Some(Err(error)) = self.with_link(|link| link.send_pointer(turn)) {
                self.warn("probe turn failed", &error);
            }
        }
        if run.yaw >= Self::TURN_SEEN {
            self.ctx.log().info("probe turn seen", &[("yaw", "90")]);
            run.enter(ProbeStep::Mouse);
        }
    }

    fn mouse_step(&self, run: &mut ProbeRun) {
        if !run.entered {
            run.entered = true;
            let wheel = self.with_link(|link| link.send_pointer(PointerOp::Wheel { notches: 1.0 }));
            let click =
                self.with_link(|link| link.send_pointer(PointerOp::Button { button: MouseButton::Left, down: true }));
            for result in [wheel, click].into_iter().flatten() {
                match result {
                    Err(LinkError::Api(ApiError::NotReady)) => return self.waive_mouse(run, "NotReady"),
                    Err(LinkError::Api(ApiError::NotAvailable)) => return self.waive_mouse(run, "NotAvailable"),
                    Err(error) => self.warn("probe mouse failed", &error),
                    Ok(()) => {}
                }
            }
            return;
        }
        if run.ticks == 0 {
            let up = PointerOp::Button { button: MouseButton::Left, down: false };
            let _ = self.with_link(|link| link.send_pointer(up));
        }
        run.ticks += 1;
        if let (Some(notches), true) = (run.wheel_notches, run.click_seen) {
            let log = self.ctx.log();
            log.info("probe wheel seen", &[("notches", &notches.to_string())]);
            log.info("probe click seen", &[("button", "Left")]);
            run.enter(ProbeStep::FullLease);
        }
    }

    fn waive_mouse(&self, run: &mut ProbeRun, reason: &str) {
        self.ctx.log().info("probe mouse waived", &[("reason", reason)]);
        let up = PointerOp::Button { button: MouseButton::Left, down: false };
        let _ = self.with_link(|link| link.send_pointer(up));
        run.enter(ProbeStep::FullLease);
    }

    fn full_lease_step(&self, run: &mut ProbeRun) {
        if !run.entered {
            run.entered = true;
            let request =
                LeaseRequest { mode: LeaseMode::Shared, blocks: InputBlocks::FULL, passthrough: &[], preempt: false };
            match self.ctx.input().acquire(request) {
                Ok(lease) => *self.full.lock().unwrap_or_else(PoisonError::into_inner) = Some(lease),
                Err(error) => self.warn("probe full lease failed", &error),
            }
        }
        run.ticks += 1;
        if run.ticks >= Self::FULL_FRAMES {
            let log = self.ctx.log();
            log.info(
                "probe full lease",
                &[
                    ("gamepad_skipped", &run.gamepad_skipped.to_string()),
                    ("look_blocked", &run.look_blocked.to_string()),
                ],
            );
            drop(self.full.lock().unwrap_or_else(PoisonError::into_inner).take());
            log.info("probe full lease released", &[]);
            run.enter(ProbeStep::Screenshot);
        }
    }

    fn screenshot_step(&self, run: &mut ProbeRun) {
        if !run.shots_asked {
            run.shots_asked = true;
            self.ask_screenshots(run);
        }
        if run.shots.is_empty() {
            let _ = self.with_link(|link| link.release(Capability::Input));
            self.ctx.log().info("probe ok", &[]);
            run.enter(ProbeStep::Done);
        }
    }

    // Asks for both screenshots in the mod's data folder. A request the loader refuses is logged as a failed write,
    // so the reader sees the failure and does not wait.
    fn ask_screenshots(&self, run: &mut ProbeRun) {
        let log = self.ctx.log();
        let dir = self.ctx.dirs().data_dir().or_else(|| self.ctx.dirs().mod_dir());
        let wanted = [(ScreenshotLayers::GameOnly, "GameOnly"), (ScreenshotLayers::WithModLayers, "WithOverlays")];
        for (layers, name) in wanted {
            let path = dir.clone().unwrap_or_default().join(format!("probe-{}.png", name.to_lowercase()));
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match self.with_link(|link| link.request_screenshot_with(&path, layers)) {
                Some(Ok(id)) => run.shots.push((id, name, path)),
                other => {
                    self.warn("probe screenshot refused", &other);
                    log.info(
                        "probe screenshot written",
                        &[("layers", name), ("ok", "false"), ("path", &path.display().to_string())],
                    );
                }
            }
        }
    }

    // After `probe ok` the probe holds W so a focus loss and a world leave have a key to release.
    fn done_step(&self, run: &mut ProbeRun) {
        if (!run.entered || run.rehold) && self.press_w(run) {
            run.entered = true;
            run.rehold = false;
        }
        if run.left {
            run.left_ticks += 1;
            if run.left_ticks >= Self::RELEASE_WAIT {
                run.left = false;
                self.ctx.log().info("probe world left", &[("released", "false")]);
            }
        }
    }
}
