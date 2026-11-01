use amm_abi::types::{AmmEventKind, AmmHandle};
use amm_ffi::types::AmmHostLayer;
use amm_ui::model::PanelAction;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

// the vertices of the manager's own layer in one host frame
fn panel_vertices(host: &TestHost, now_ms: u64) -> usize {
    let frame = host.frame(now_ms);
    let layers = TestHost::layers(&frame);
    let panel = layers.iter().find(|layer| layer.owner == AmmHostLayer::MANAGER);
    panel.map_or(0, |layer| layer.mesh.vertex_count)
}

#[test]
fn a_language_pick_redraws_the_open_panel() {
    let host = TestHost::new();
    let _client = TestClient::register(&host, &Descriptors::overlay("locale-redraw"), TestMesh::triangle());
    host.with_runtime(|runtime| runtime.apply_panel_actions(&[PanelAction::Open]));
    let english = panel_vertices(&host, 100);

    host.with_runtime(|runtime| runtime.apply_panel_actions(&[PanelAction::SetLanguage("en_XA".to_owned())]));
    let pseudo = panel_vertices(&host, 200);

    assert!(english > 0, "the open panel draws");
    // the pseudo-locale wraps every message in brackets and padding: more glyphs, more vertices
    assert!(pseudo > english, "the panel still draws {pseudo} vertices, as in English ({english})");
}

#[test]
fn host_applies_a_language() {
    let host = TestHost::new();
    let reader = TestClient::register(&host, &Descriptors::overlay("locale-panel"), TestMesh::triangle());
    let _ = read_locale(reader.handle(), 8);
    let _ = reader.poll();

    host.with_runtime(|runtime| runtime.apply_panel_actions(&[PanelAction::SetLanguage("en_XA".to_owned())]));

    let changes = reader.poll().iter().filter(|event| event.kind == AmmEventKind::LocaleChanged).count();
    assert_eq!(changes, 1, "the reader gets one LocaleChanged");
    let (len, bytes) = read_locale(reader.handle(), 8);
    assert_eq!(&bytes[..len as usize], b"en_XA");
}

// calls the table `locale` with a buffer of `cap` bytes; returns the length and the bytes it wrote
fn read_locale(handle: AmmHandle, cap: usize) -> (u32, Vec<u8>) {
    let locale = TestHost::api().locale.unwrap_or_else(|| panic!("locale"));
    let mut buffer = vec![0u8; cap];
    let pointer = if cap == 0 { std::ptr::null_mut() } else { buffer.as_mut_ptr() };
    // SAFETY: the buffer holds `cap` writable bytes for the call, or is null with `cap` 0
    let len = unsafe { locale(handle, pointer, cap) };
    (len, buffer)
}

#[test]
fn locale_copies_the_code() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("locale-copy"), TestMesh::triangle());
    host.with_runtime(|runtime| runtime.set_locale("en_XA"));

    let (len, bytes) = read_locale(client.handle(), 8);
    assert_eq!(len, 5);
    assert_eq!(&bytes[..5], b"en_XA");

    let (short_len, short) = read_locale(client.handle(), 2);
    assert_eq!(short_len, 5);
    assert_eq!(short, [0, 0], "a short buffer was written");

    assert_eq!(read_locale(client.handle(), 0).0, 5);
}

#[test]
fn english_is_an_empty_code() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("locale-english"), TestMesh::triangle());

    assert_eq!(read_locale(client.handle(), 8).0, 0);
}

#[test]
fn only_a_reader_gets_the_locale_event() {
    let host = TestHost::new();
    let reader = TestClient::register(&host, &Descriptors::overlay("locale-reader"), TestMesh::triangle());
    let older = TestClient::register(&host, &Descriptors::overlay("locale-older"), TestMesh::triangle());
    let _ = read_locale(reader.handle(), 8);
    let _ = reader.poll();
    let _ = older.poll();

    host.with_runtime(|runtime| runtime.set_locale("en_XA"));

    assert!(reader.poll().iter().any(|event| event.kind == AmmEventKind::LocaleChanged));
    assert!(older.poll().iter().all(|event| event.kind != AmmEventKind::LocaleChanged));
}
