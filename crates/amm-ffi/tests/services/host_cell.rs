use std::sync::mpsc;
use std::thread::Scope;
use std::time::Duration;

use amm_abi::types::{AmmCapability, AmmUiEventKind};
use amm_ffi::host::{
    amm_host_char, amm_host_focus_lost, amm_host_game_point, amm_host_key, amm_host_pointer, amm_host_tick,
};
use amm_ffi::services::HostCell;
use amm_ffi::types::AmmHostPointerKind;

use crate::support::{Descriptors, SharedMeshClient, TestClient, TestHost, TestMesh};

// How long a caller may take while a frame callback is held; the old host lock made it wait for the whole callback.
const PROMPT: Duration = Duration::from_secs(2);
const KEY_A: u32 = 0x41;

// A client with an Overlay layer whose frame callback signals its entry and then waits for a release.
fn held_client(host: &TestHost, name: &str) -> (TestClient, mpsc::Receiver<()>, mpsc::Sender<()>) {
    let client = TestClient::register(host, &Descriptors::overlay(name), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    *client.state().gate.lock().unwrap_or_else(|error| panic!("gate: {error}")) = Some((entered_tx, release_rx));
    (client, entered_rx, release_tx)
}

// Runs `call` on its own thread and tells whether it returned within `PROMPT`. The thread is joined by the scope,
// so a caller that waits on the lock finishes once the callback is released instead of hanging the test.
fn returns_promptly<'scope>(scope: &'scope Scope<'scope, '_>, call: impl FnOnce() + Send + 'scope) -> bool {
    let (done_tx, done_rx) = mpsc::channel();
    scope.spawn(move || {
        call();
        let _ = done_tx.send(());
    });
    done_rx.recv_timeout(PROMPT).is_ok()
}

#[test]
fn input_and_tick_callers_do_not_wait_for_a_running_frame_callback() {
    let host = TestHost::new();
    let (client, entered, release) = held_client(&host, "slow-hud");

    let prompt = std::thread::scope(|scope| {
        let render = scope.spawn(|| host.frame(5).layer_count);
        entered.recv_timeout(Duration::from_secs(5)).expect("frame callback entered");
        let prompt = [
            (
                "key",
                returns_promptly(scope, || {
                    let _ = amm_host_key(KEY_A, true, false, 0, 6);
                }),
            ),
            (
                "pointer move",
                returns_promptly(scope, || {
                    let _ = amm_host_pointer(AmmHostPointerKind::Move as u8, 10.0, 10.0, 0, false, 0.0);
                }),
            ),
            (
                "pointer button",
                returns_promptly(scope, || {
                    let _ = amm_host_pointer(AmmHostPointerKind::Button as u8, 10.0, 10.0, 0, true, 0.0);
                }),
            ),
            (
                "char",
                returns_promptly(scope, || {
                    let _ = amm_host_char(u32::from('a'));
                }),
            ),
            (
                "focus lost",
                returns_promptly(scope, || {
                    let _ = amm_host_focus_lost();
                }),
            ),
            ("tick", returns_promptly(scope, || amm_host_tick(7))),
            (
                "game point",
                returns_promptly(scope, || {
                    let (mut x, mut y) = (0.0f32, 0.0f32);
                    // SAFETY: both out pointers are live locals for the call
                    let _ = unsafe { amm_host_game_point(1.0, 1.0, TestHost::WIDTH, TestHost::HEIGHT, &mut x, &mut y) };
                }),
            ),
        ];
        release.send(()).expect("release");
        assert_eq!(render.join().expect("render thread"), 1);
        prompt
    });
    client.state().gate.lock().expect("gate").take();

    let waited: Vec<&str> = prompt.iter().filter(|(_, prompt)| !prompt).map(|(name, _)| *name).collect();
    assert!(waited.is_empty(), "these callers waited for the frame callback: {waited:?}");
}

#[test]
fn a_pointer_move_routed_during_a_frame_callback_reaches_the_layer_in_the_next_frame() {
    let host = TestHost::new();
    let (client, entered, release) = held_client(&host, "late-move");

    std::thread::scope(|scope| {
        let render = scope.spawn(|| host.frame(5).layer_count);
        entered.recv_timeout(Duration::from_secs(5)).expect("frame callback entered");
        returns_promptly(scope, || {
            let _ = amm_host_pointer(AmmHostPointerKind::Move as u8, 40.0, 40.0, 0, false, 0.0);
        });
        release.send(()).expect("release");
        render.join().expect("render thread");
    });
    client.state().gate.lock().expect("gate").take();
    host.frame(6);

    let calls = client.seen_calls();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].1.is_empty(), "the held frame got events: {:?}", calls[0].1);
    assert!(calls[1].1.iter().any(|(kind, _)| *kind == AmmUiEventKind::PointerMove), "{:?}", calls[1].1);
}

#[test]
fn a_frame_after_the_host_stopped_mid_frame_has_no_host() {
    let host = TestHost::new();
    let (client, entered, release) = held_client(&host, "outlived");

    let stopped_early = std::thread::scope(|scope| {
        let render = scope.spawn(|| host.frame(5).layer_count);
        entered.recv_timeout(Duration::from_secs(5)).expect("frame callback entered");
        let stopper = scope.spawn(|| host.destroy());
        std::thread::sleep(Duration::from_millis(150));
        let stopped_early = stopper.is_finished();
        release.send(()).expect("release");
        render.join().expect("render thread");
        stopper.join().expect("stopper");
        stopped_early
    });
    client.state().gate.lock().expect("gate").take();

    assert!(!stopped_early, "the host stopped while a frame callback of one of its mods ran");
    assert!(HostCell::global().shared().is_none());
}

#[test]
fn a_pointer_move_nothing_can_take_passes_without_the_host_lock_and_is_tracked_later() {
    let host = TestHost::new();
    host.frame(0);
    let (locked_tx, locked_rx) = mpsc::channel();
    let (unlock_tx, unlock_rx) = mpsc::channel::<()>();

    let prompt = std::thread::scope(|scope| {
        let holding = &host;
        let holder = scope.spawn(move || {
            holding.with_runtime(move |_| {
                let _ = locked_tx.send(());
                let _ = unlock_rx.recv_timeout(Duration::from_secs(10));
            })
        });
        locked_rx.recv_timeout(Duration::from_secs(5)).expect("host lock held");
        let prompt = returns_promptly(scope, || {
            let _ = amm_host_pointer(AmmHostPointerKind::Move as u8, 40.0, 30.0, 0, false, 0.0);
        });
        unlock_tx.send(()).expect("unlock");
        holder.join().expect("holder thread");
        prompt
    });
    let mut tracked = (0.0, 0.0);
    host.with_runtime(|runtime| tracked = runtime.pointer());

    assert!(prompt, "the pointer move waited for the host lock");
    assert_eq!(tracked, (40.0, 30.0));
}

#[test]
fn a_mod_that_registers_while_a_frame_callback_runs_keeps_its_texture_grant() {
    let host = TestHost::with_config("textures_per_mod = 1");
    let (held, entered, release) = held_client(&host, "slow-hud");
    let mut three = TestMesh::with_texture(1, 2);
    let first = three.deltas[0];
    three.deltas.push(amm_abi::types::AmmTextureDelta { id: 2, ..first });
    three.deltas.push(amm_abi::types::AmmTextureDelta { id: 3, ..first });

    // `register` runs on its own thread while the frame callback is held, and the callback is released only once
    // `register` returned, so it ran inside the unlocked gap. The callbacks are set after the frame, since
    // `set_callbacks` waits on the frame gate.
    let config_dir = host.next_mod_dir();
    let (handle, in_gap) = std::thread::scope(|scope| {
        let render = scope.spawn(|| host.frame(5).layer_count);
        entered.recv_timeout(Duration::from_secs(5)).expect("frame callback entered");
        let (registered_tx, registered_rx) = mpsc::channel();
        let dir = config_dir.clone();
        scope.spawn(move || {
            let descriptor = Descriptors::with_claims(
                "late",
                &[("Overlay", "Shared")],
                "texture_budget_count = 2
",
            );
            let _ = registered_tx.send(TestClient::register_handle(&descriptor, &dir));
        });
        let registered = registered_rx.recv_timeout(PROMPT);
        let in_gap = !render.is_finished();
        release.send(()).expect("release");
        render.join().expect("render thread");
        let handle = registered.expect("register returned while the frame callback was held").expect("register");
        (handle, in_gap)
    });
    let late = TestClient::attach(handle, config_dir, three);
    late.acquire(AmmCapability::Overlay);
    held.state().gate.lock().expect("gate").take();
    let frame = host.frame(6);

    let layer = TestHost::layers(&frame).iter().find(|layer| layer.owner == late.owner()).copied().expect("late layer");
    assert!(in_gap, "register did not run while the frame callback was held");
    assert_eq!(layer.mesh.delta_count, 2, "the grant of 2 textures was lost");
}

#[test]
fn each_layer_of_a_mod_that_shares_one_mesh_buffer_draws_its_own_mesh() {
    let host = TestHost::new();
    let client = SharedMeshClient::register(&host, "shared-buffer");

    let frame = host.frame(0);

    let layers: Vec<_> =
        TestHost::layers(&frame).iter().filter(|layer| layer.owner == client.owner()).copied().collect();
    assert_eq!(layers.len(), 2);
    let colors: Vec<Vec<[u8; 4]>> = layers
        .iter()
        .map(|layer| {
            // SAFETY: the host keeps each layer's copied vertices valid until the next frame
            unsafe { std::slice::from_raw_parts(layer.mesh.vertices, layer.mesh.vertex_count) }
                .iter()
                .map(|vertex| vertex.color)
                .collect()
        })
        .collect();
    assert_eq!(colors[0], vec![SharedMeshClient::HUD_COLOR; SharedMeshClient::HUD_VERTICES]);
    assert_eq!(colors[1], vec![SharedMeshClient::OVERLAY_COLOR; SharedMeshClient::OVERLAY_VERTICES]);
}

#[test]
fn a_panic_in_the_unlocked_step_keeps_the_compositor_and_later_frames_draw() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("survivor"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    let input = amm_ffi::types::AmmHostFrameInput {
        width: TestHost::WIDTH,
        height: TestHost::HEIGHT,
        pixels_per_point: 1.0,
        time_secs: 0.0,
        now_ms: 0,
        menu_screen: TestHost::PLAY,
        format: TestHost::R8G8B8A8_UNORM,
        client_width: TestHost::WIDTH,
        client_height: TestHost::HEIGHT,
    };

    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        HostCell::global().frame_with_step(&input, |_, _| panic!("step failed")).map(|frame| frame.layer_count)
    }));
    let after = host.frame(1);

    assert_eq!(panicked.ok(), Some(Ok(0)), "the panic left the frame");
    assert_eq!(TestHost::owners(&after), vec![client.owner()]);
}

#[test]
fn a_client_size_read_while_the_window_resizes_is_not_kept() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("viewer"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    host.frame(0);
    let size = amm_core::structs::render::RenderSize::new(TestHost::WIDTH, TestHost::HEIGHT);
    let cell = HostCell::global();
    let reads = std::sync::atomic::AtomicU32::new(0);

    // a window size arrives while the old size is read
    cell.game_point(|| {
        cell.forget_client_size();
        Some(size)
    });
    cell.game_point(|| {
        reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Some(size)
    });
    cell.game_point(|| {
        reads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Some(size)
    });

    assert_eq!(reads.load(std::sync::atomic::Ordering::SeqCst), 1, "the size read across a resize was kept");
}

#[test]
fn a_panic_inside_the_frame_callbacks_step_keeps_every_job_and_later_frames_call_every_mod() {
    let host = TestHost::new();
    let first = TestClient::register(&host, &Descriptors::overlay("first"), TestMesh::triangle());
    let second = TestClient::register(&host, &Descriptors::overlay("second"), TestMesh::triangle());
    first.acquire(AmmCapability::Overlay);
    second.acquire(AmmCapability::Overlay);
    let input = amm_ffi::types::AmmHostFrameInput {
        width: TestHost::WIDTH,
        height: TestHost::HEIGHT,
        pixels_per_point: 1.0,
        time_secs: 0.0,
        now_ms: 0,
        menu_screen: TestHost::PLAY,
        format: TestHost::R8G8B8A8_UNORM,
        client_width: TestHost::WIDTH,
        client_height: TestHost::HEIGHT,
    };

    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        HostCell::global().frame_with_step(&input, |compositor, input| {
            // SAFETY: `frame_with_step` holds the frame gate while the step runs
            unsafe { compositor.run_callbacks_with(input, |_| panic!("stopped after the first job")) };
        })
    }));
    let (first_before, second_before) = (first.frames(), second.frames());
    let after = host.frame(1);

    assert!(stopped.is_ok_and(|frame| frame.is_ok()), "the panic left the frame");
    assert_eq!((first_before, second_before), (1, 0));
    assert_eq!((first.frames(), second.frames()), (2, 1));
    assert_eq!(TestHost::layers(&after).len(), 2);
}
