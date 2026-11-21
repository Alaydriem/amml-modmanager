use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use amm_abi::types::{AMM_ABI_MINOR, AmmAcquireOutcome, AmmAcquireResult, AmmCapability, AmmHandle, AmmStatus};
use amm_ffi::host::amm_host_textures_lost;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

const FAILURE_LIMIT: u32 = 30;

// a config folder for a call with no host, so the folder rules are never reached
const ABSOLUTE_DIR: &str = if cfg!(windows) { r"C:\amm-test\mods\@test\early\config" } else { "/amm-test/config" };

fn register_in(text: *const u8, len: usize, config_dir: &str) -> AmmStatus {
    let mut handle = AmmHandle { slot: 0, generation: 0 };
    let register = TestHost::core_table().register.unwrap_or_else(|| panic!("register"));
    // SAFETY: the text is null or lives for the call, and the folder and handle live for the call
    unsafe { register(text, len, config_dir.as_ptr(), config_dir.len(), &mut handle) }
}

fn config_document(handle: AmmHandle) -> Result<String, AmmStatus> {
    let (mut ptr, mut len) = (std::ptr::null(), 0);
    // SAFETY: both out pointers live for the call
    let status = unsafe {
        TestHost::settings_table().config_document.unwrap_or_else(|| panic!("config_document"))(
            handle, &mut ptr, &mut len,
        )
    };
    if status != AmmStatus::Ok {
        return Err(status);
    }
    // SAFETY: the manager keeps the text valid until the next config_document call
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    Ok(String::from_utf8(bytes.to_vec()).unwrap_or_else(|error| panic!("utf-8: {error}")))
}

fn drive_frames(host: &TestHost, count: u64) {
    for now in 0..count {
        host.frame(now);
    }
}

#[test]
fn null_descriptor_and_null_handle_return_null_pointer() {
    let host = TestHost::new();
    let text = Descriptors::overlay("solo");
    let dir = host.next_mod_dir().display().to_string();
    let register = TestHost::core_table().register.unwrap_or_else(|| panic!("register"));

    let null_text = register_in(std::ptr::null(), 10, &dir);
    // SAFETY: the text and the folder live for the call; the handle pointer is null on purpose
    let null_handle = unsafe { register(text.as_ptr(), text.len(), dir.as_ptr(), dir.len(), std::ptr::null_mut()) };

    assert_eq!(null_text, AmmStatus::NullPointer);
    assert_eq!(null_handle, AmmStatus::NullPointer);
}

#[test]
fn bad_descriptor_returns_invalid_descriptor() {
    let host = TestHost::new();
    let not_hcl = "id = ";
    let not_utf8 = [0xFFu8, 0xFE, 0x00];
    let dir = host.next_mod_dir().display().to_string();

    assert_eq!(register_in(not_hcl.as_ptr(), not_hcl.len(), &dir), AmmStatus::InvalidDescriptor);
    assert_eq!(register_in(not_utf8.as_ptr(), not_utf8.len(), &dir), AmmStatus::InvalidDescriptor);
}

#[test]
fn register_without_a_host_returns_manager_missing() {
    let text = Descriptors::overlay("early");

    assert_eq!(register_in(text.as_ptr(), text.len(), ABSOLUTE_DIR), AmmStatus::ManagerMissing);
}

#[test]
fn every_table_declares_its_size_and_the_abi_minor_and_fills_every_slot() {
    let (core, settings, keybinds, layers, leases) = (
        TestHost::core_table(),
        TestHost::settings_table(),
        TestHost::keybinds_table(),
        TestHost::layers_table(),
        TestHost::leases_table(),
    );

    assert_eq!(core.size as usize, size_of_val(core));
    assert_eq!(settings.size as usize, size_of_val(settings));
    assert_eq!(keybinds.size as usize, size_of_val(keybinds));
    assert_eq!(layers.size as usize, size_of_val(layers));
    assert_eq!(leases.size as usize, size_of_val(leases));
    assert_eq!([core.version, settings.version, keybinds.version, layers.version, leases.version], [AMM_ABI_MINOR; 5]);
    assert!(core.register.is_some() && core.unregister.is_some() && core.poll_events.is_some());
    assert!(core.locale.is_some() && core.panel_open.is_some());
    assert!(core.register_menu_entry.is_some() && core.set_icon.is_some());
    assert!(settings.config_document.is_some() && settings.set_setting.is_some());
    assert!(keybinds.chord_text.is_some() && keybinds.bind_down.is_some());
    assert!(layers.set_callbacks.is_some() && layers.texture_budget.is_some() && layers.set_cursor_mode.is_some());
    assert!(layers.game_picture.is_some() && layers.set_game_viewport.is_some());
    assert!(layers.set_render_size.is_some() && layers.render_size.is_some());
    assert!(leases.acquire.is_some() && leases.release.is_some());
    assert!(leases.holder.is_some() && leases.held_by_other.is_some());
}

#[test]
fn failing_client_frame_is_disabled_after_thirty_failures_and_the_other_client_still_draws() {
    let host = TestHost::new();
    let failing = TestClient::register(&host, &Descriptors::overlay("failing"), TestMesh::triangle());
    let healthy = TestClient::register(&host, &Descriptors::overlay("healthy"), TestMesh::triangle());
    failing.acquire(AmmCapability::Overlay);
    healthy.acquire(AmmCapability::Overlay);
    failing.fail_with(AmmStatus::InvalidArgument);

    let mut healthy_drawn = 0;
    for now in 0..40 {
        let frame = host.frame(now);
        let owners = TestHost::owners(&frame);
        assert!(!owners.contains(&failing.owner()));
        healthy_drawn += usize::from(owners.contains(&healthy.owner()));
    }

    assert_eq!(failing.frames(), FAILURE_LIMIT);
    assert_eq!(healthy.frames(), 40);
    assert_eq!(healthy_drawn, 40);
}

#[test]
fn bad_bytes_in_a_frame_output_count_as_failures() {
    let host = TestHost::new();
    let bad_cursor = TestClient::register(&host, &Descriptors::overlay("bad-cursor"), TestMesh::triangle());
    let mut bad_sampler_cmd = TestMesh::cmd([0.0, 0.0, 10.0, 10.0]);
    bad_sampler_cmd.sampler = 9;
    let bad_sampler =
        TestClient::register(&host, &Descriptors::overlay("bad-sampler"), TestMesh::with_cmds(vec![bad_sampler_cmd]));
    let mut bad_delta_mesh = TestMesh::with_texture(1, 4);
    bad_delta_mesh.deltas[0].full = 7;
    let bad_delta = TestClient::register(&host, &Descriptors::overlay("bad-delta"), bad_delta_mesh);
    let mut out_of_range_cmd = TestMesh::cmd([0.0, 0.0, 10.0, 10.0]);
    out_of_range_cmd.index_count = 4;
    let bad_range =
        TestClient::register(&host, &Descriptors::overlay("bad-range"), TestMesh::with_cmds(vec![out_of_range_cmd]));
    let unknown_status = TestClient::register(&host, &Descriptors::overlay("bad-status"), TestMesh::triangle());
    let clients = [&bad_cursor, &bad_sampler, &bad_delta, &bad_range, &unknown_status];
    for client in clients {
        client.acquire(AmmCapability::Overlay);
    }
    bad_cursor.set_cursor_byte(9);
    unknown_status.state().status.store(1234, Ordering::SeqCst);

    for now in 0..40 {
        let owners = TestHost::owners(&host.frame(now));
        assert!(clients.iter().all(|client| !owners.contains(&client.owner())));
    }

    assert!(clients.iter().all(|client| client.frames() == FAILURE_LIMIT));
}

#[test]
fn textures_lost_reaches_every_client() {
    let host = TestHost::new();
    let first = TestClient::register(&host, &Descriptors::overlay("first"), TestMesh::triangle());
    let second = TestClient::register(&host, &Descriptors::overlay("second"), TestMesh::triangle());
    first.acquire(AmmCapability::Overlay);
    host.frame(0);

    amm_host_textures_lost();

    assert_eq!(first.textures_lost(), 1);
    assert_eq!(second.textures_lost(), 1);
}

#[test]
fn unregister_frees_textures_and_ends_cursor_mode() {
    let host = TestHost::new();
    let mut client = TestClient::register(&host, &Descriptors::overlay("painter"), TestMesh::with_texture(5, 16));
    client.acquire(AmmCapability::Overlay);
    client.set_cursor_mode(1);
    let owner = client.owner();
    let drawn = host.frame(0);
    assert!(drawn.cursor_free);
    assert_eq!(drawn.freed_owners, 0);

    client.unregister();
    let after = host.frame(1);
    let later = host.frame(2);

    assert_eq!(after.freed_owners, 1 << owner);
    assert!(!after.cursor_free);
    assert_eq!(after.layer_count, 0);
    assert_eq!(later.freed_owners, 0);
}

#[test]
fn unregister_waits_for_a_running_frame_callback_and_no_callback_starts_after_it() {
    let host = TestHost::new();
    let mut client = TestClient::register(&host, &Descriptors::overlay("slow"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    *client.state().gate.lock().expect("gate") = Some((entered_tx, release_rx));
    let handle = client.handle();

    let returned_early = std::thread::scope(|scope| {
        let render = scope.spawn(|| host.frame(5).layer_count);
        entered_rx.recv_timeout(Duration::from_secs(5)).expect("frame callback entered");
        let game = scope.spawn(move || TestClient::unregister_handle(handle));
        std::thread::sleep(Duration::from_millis(150));
        let returned_early = game.is_finished();
        release_tx.send(()).expect("release");
        render.join().expect("render thread");
        game.join().expect("game thread");
        returned_early
    });
    client.state().gate.lock().expect("gate").take();
    let frames_at_unregister = client.frames();
    client.unregister();
    host.frame(6);

    assert!(!returned_early);
    assert_eq!(frames_at_unregister, 1);
    assert_eq!(client.frames(), 1);
}

#[test]
fn acquire_with_a_bad_capability_byte_is_denied_with_no_holder() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("asker"), TestMesh::triangle());

    let result = client.acquire_raw(42);

    assert_eq!(result, AmmAcquireResult { outcome: AmmAcquireOutcome::Denied, holder: AmmAcquireResult::NO_HOLDER });
    assert!(client.poll().is_empty());
}

#[test]
fn release_cursor_mode_and_holder_ignore_bad_bytes() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("holder"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);

    client.release_raw(42);
    client.set_cursor_mode(7);
    let (held_len, held) = TestClient::holder_id(AmmCapability::Overlay as u8, 64);
    let (bad_len, _) = TestClient::holder_id(42, 64);
    let (nobody_len, _) = TestClient::holder_id(AmmCapability::Camera as u8, 64);
    let frame = host.frame(0);

    assert_eq!(&held[..held_len as usize], b"@test/holder");
    assert_eq!(bad_len, 0);
    assert_eq!(nobody_len, 0);
    assert!(TestHost::owners(&frame).contains(&client.owner()));
    assert!(!frame.cursor_free);
}

#[test]
fn stale_handle_is_rejected_by_every_function() {
    let host = TestHost::new();
    let mut first = TestClient::register(&host, &Descriptors::overlay("reused"), TestMesh::triangle());
    let stale = first.handle();
    first.unregister();
    let mut second = TestClient::register(&host, &Descriptors::overlay("reused"), TestMesh::triangle());
    let (path, value) = ("missing.path", "1");
    assert_eq!(second.handle().slot, stale.slot);
    assert_ne!(second.handle().generation, stale.generation);
    assert_eq!(second.acquire(AmmCapability::Overlay).outcome, AmmAcquireOutcome::Granted);

    let (settings, keybinds, leases, layers) =
        (TestHost::settings_table(), TestHost::keybinds_table(), TestHost::leases_table(), TestHost::layers_table());
    // SAFETY: the path and value live for the call
    let set =
        unsafe { settings.set_setting.expect("set_setting")(stale, path.as_ptr(), path.len(), value.as_ptr(), 1) };
    // SAFETY: bind_down takes plain values
    let bind = unsafe { keybinds.bind_down.expect("bind_down")(stale, 0) };
    let acquired = {
        // SAFETY: acquire takes plain values
        unsafe { leases.acquire.expect("acquire")(stale, AmmCapability::Overlay as u8) }
    };
    // SAFETY: release and set_cursor_mode take plain values
    unsafe {
        leases.release.expect("release")(stale, AmmCapability::Overlay as u8);
        layers.set_cursor_mode.expect("set_cursor_mode")(stale, 1);
    }
    let unregistered = TestClient::unregister_handle(stale);
    let (holder_len, holder) = TestClient::holder_id(AmmCapability::Overlay as u8, 64);
    let frame = host.frame(0);

    assert_eq!(config_document(stale), Err(AmmStatus::UnknownHandle));
    assert_eq!(set, AmmStatus::UnknownHandle);
    assert_eq!(bind, 0);
    assert_eq!(acquired.outcome, AmmAcquireOutcome::Denied);
    assert!(TestClient::poll_handle(stale).is_empty());
    assert!(config_document(second.handle()).is_ok());
    assert_eq!(unregistered, AmmStatus::UnknownHandle);
    assert_eq!(&holder[..holder_len as usize], b"@test/reused");
    assert!(TestHost::owners(&frame).contains(&second.owner()));
    assert!(!frame.cursor_free);
    second.unregister();
}

#[test]
fn registration_with_no_file_writes_defaults_as_config_hcl() {
    let host = TestHost::new();
    let schema = "section \"keys\" {\n  title = \"Keys\"\n  field \"keys.toggle\" {\n    label = \"Toggle\"\n    \
                  kind = \"chord\"\n    default = \"Ctrl+L\"\n    apply = \"Live\"\n  }\n}\n";
    let client = TestClient::register(
        &host,
        &Descriptors::with_claims("writer", &[("Overlay", "Shared")], schema),
        TestMesh::triangle(),
    );
    let document = config_document(client.handle()).expect("document");
    let config_dir = client.config_dir().to_path_buf();
    drop(client);
    host.destroy();

    let path = config_dir.join("config.hcl");
    let written = std::fs::read_to_string(&path).expect("config.hcl written");

    assert!(document.contains("toggle = \"Ctrl+L\""));
    assert_eq!(written, document);
}

#[test]
fn a_failed_frame_streak_shows_a_toast_layer() {
    let host = TestHost::new();
    let failing = TestClient::register(&host, &Descriptors::overlay("toaster"), TestMesh::triangle());
    failing.acquire(AmmCapability::Overlay);
    failing.fail_with(AmmStatus::Panic);
    drive_frames(&host, u64::from(FAILURE_LIMIT));

    let frame = host.frame(100);

    assert_eq!(TestHost::owners(&frame), vec![TestHost::MANAGER]);
}

#[test]
fn a_table_call_from_inside_a_frame_callback_is_refused_instead_of_waiting_on_the_host_lock() {
    let host = TestHost::new();
    let client = TestClient::register(
        &host,
        &Descriptors::with_claims("reentrant", &[("Overlay", "Shared"), ("Camera", "Shared")], ""),
        TestMesh::triangle(),
    );
    client.acquire(AmmCapability::Overlay);
    *client.state().call_back_in.lock().expect("call back") = Some(client.handle());

    let frame = host.frame(0);

    assert!(client.state().called_back.load(Ordering::SeqCst));
    assert_eq!(client.state().inner_outcome.load(Ordering::SeqCst), AmmAcquireOutcome::Denied as u8);
    assert_eq!(TestHost::owners(&frame), vec![client.owner()]);
    assert_eq!(client.acquire(AmmCapability::Camera).outcome, AmmAcquireOutcome::Granted);
}

#[test]
fn register_with_a_relative_config_dir_is_invalid_argument() {
    let host = TestHost::new();
    let text = Descriptors::overlay("placed");
    let good = host.next_mod_dir().display().to_string();
    let parent = format!("{good}{}..{}other", std::path::MAIN_SEPARATOR, std::path::MAIN_SEPARATOR);

    assert_eq!(register_in(text.as_ptr(), text.len(), "mods/placed/config"), AmmStatus::InvalidArgument);
    assert_eq!(register_in(text.as_ptr(), text.len(), ""), AmmStatus::InvalidArgument);
    assert_eq!(register_in(text.as_ptr(), text.len(), &parent), AmmStatus::InvalidArgument);
    assert_eq!(register_in(text.as_ptr(), text.len(), &good), AmmStatus::Ok);
}

#[test]
fn layer_memory_stays_valid_after_the_mod_unregisters() {
    let host = TestHost::new();
    let mut mesh = TestMesh::with_texture(3, 2);
    for (i, vertex) in mesh.vertices.iter_mut().enumerate() {
        vertex.pos = [i as f32, 1.0];
    }
    mesh.bytes = (0..16).collect();
    let (vertices, indices, bytes) = (mesh.vertices.clone(), mesh.indices.clone(), mesh.bytes.clone());
    let mut client = TestClient::register(&host, &Descriptors::overlay("leaver"), mesh);
    client.acquire(AmmCapability::Overlay);
    let client_vertices = client.state().mesh.vertices.as_ptr();

    let frame = host.frame(0);
    let layer = TestHost::layers(&frame)[0];
    client.unregister();
    drop(client);

    assert_ne!(layer.mesh.vertices, client_vertices);
    // SAFETY: the host releases an unregistered mod's copies only at the start of the next frame, so the layer
    // read before the unregister still points at live memory here
    let (seen_vertices, seen_indices, seen_bytes, delta) = unsafe {
        (
            std::slice::from_raw_parts(layer.mesh.vertices, layer.mesh.vertex_count),
            std::slice::from_raw_parts(layer.mesh.indices, layer.mesh.index_count),
            std::slice::from_raw_parts(layer.mesh.bytes, layer.mesh.byte_count),
            *layer.mesh.deltas,
        )
    };
    let start = delta.byte_offset as usize;
    assert_eq!(seen_vertices, vertices.as_slice());
    assert_eq!(seen_indices, indices.as_slice());
    assert_eq!(&seen_bytes[start..start + delta.byte_len as usize], bytes.as_slice());
    assert_eq!(host.frame(1).layer_count, 0);
}

#[test]
fn holder_with_a_buffer_too_small_writes_nothing_and_returns_the_id_length() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("long-holder-id"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);

    let (len, buffer) = TestClient::holder_id(AmmCapability::Overlay as u8, 4);
    // SAFETY: a null buffer with no room is a length query
    let queried = unsafe {
        TestHost::leases_table().holder.expect("holder")(AmmCapability::Overlay as u8, std::ptr::null_mut(), 0)
    };

    assert_eq!(len as usize, "@test/long-holder-id".len());
    assert_eq!(queried, len);
    assert_eq!(buffer, vec![0u8; 4]);
}

#[test]
fn holder_id_stays_valid_after_the_holder_book_is_republished() {
    let host = TestHost::new();
    let holder = TestClient::register(&host, &Descriptors::overlay("long-lived-holder"), TestMesh::triangle());
    holder.acquire(AmmCapability::Overlay);
    let (len, before) = TestClient::holder_id(AmmCapability::Overlay as u8, 64);
    let other = TestClient::register(
        &host,
        &Descriptors::with_claims("other", &[("Camera", "Shared")], ""),
        TestMesh::triangle(),
    );

    for _ in 0..8 {
        other.acquire(AmmCapability::Camera);
        host.frame(0);
        other.release_raw(AmmCapability::Camera as u8);
        host.frame(1);
    }
    let (again_len, after) = TestClient::holder_id(AmmCapability::Overlay as u8, 64);

    assert_eq!(&before[..len as usize], b"@test/long-lived-holder");
    assert_eq!(&after[..again_len as usize], &before[..len as usize]);
}

#[test]
fn unregister_from_inside_the_mods_own_frame_callback_is_refused_and_the_mod_stays() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("self-unregister"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    *client.state().unregister_in.lock().expect("unregister in") = Some(client.handle());

    let first = host.frame(0);
    client.state().unregister_in.lock().expect("unregister in").take();
    let second = host.frame(1);

    assert_eq!(client.state().inner_unregister.load(Ordering::SeqCst), AmmStatus::NotPermitted as i32);
    assert_eq!(TestHost::owners(&first), vec![client.owner()]);
    assert_eq!(TestHost::owners(&second), vec![client.owner()]);
}

#[test]
fn register_refuses_network_device_and_verbatim_config_dirs() {
    let host = TestHost::new();
    let text = Descriptors::overlay("remote");
    let refused = [r"\\server\share", r"\\.\pipe\x", r"\\?\C:\x", "//server/share/x"];

    for dir in refused {
        assert_eq!(register_in(text.as_ptr(), text.len(), dir), AmmStatus::InvalidArgument, "{dir}");
    }
    let good = host.next_mod_dir().display().to_string();
    assert_eq!(register_in(text.as_ptr(), text.len(), &good), AmmStatus::Ok);
}

#[test]
fn register_refuses_the_managers_own_config_folder_and_folders_inside_it() {
    let host = TestHost::new();
    let text = Descriptors::overlay("intruder");
    let own = host.config_dir().display().to_string();
    let inside = host.config_dir().join("nested").display().to_string();
    let upper = own.to_uppercase();

    assert_eq!(register_in(text.as_ptr(), text.len(), &own), AmmStatus::InvalidArgument);
    assert_eq!(register_in(text.as_ptr(), text.len(), &inside), AmmStatus::InvalidArgument);
    assert_eq!(register_in(text.as_ptr(), text.len(), &upper), AmmStatus::InvalidArgument);
}

#[test]
fn register_refuses_a_config_folder_another_live_mod_uses_until_it_unregisters() {
    let host = TestHost::new();
    let mut first = TestClient::register(&host, &Descriptors::overlay("owner"), TestMesh::triangle());
    let taken = first.config_dir().display().to_string();
    let text = Descriptors::overlay("squatter");

    let while_live = register_in(text.as_ptr(), text.len(), &taken);
    first.unregister();
    let after = register_in(text.as_ptr(), text.len(), &taken);

    assert_eq!(while_live, AmmStatus::InvalidArgument);
    assert_eq!(after, AmmStatus::Ok);
}
