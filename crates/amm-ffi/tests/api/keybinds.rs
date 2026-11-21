use amm_abi::types::AmmHandle;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

const KEY_P: u32 = 0x50;
// the index of the `toggle` keybind (`Ctrl+P`) in the freecam fixture
const TOGGLE: u32 = 0;

fn chord_text(handle: AmmHandle, index: u32, capacity: usize) -> (u32, Vec<u8>) {
    let mut buffer = vec![0u8; capacity];
    let chord_text = TestHost::keybinds_table().chord_text.unwrap_or_else(|| panic!("chord_text"));
    // SAFETY: the buffer has room for its length in bytes for the call
    let len = unsafe { chord_text(handle, index, buffer.as_mut_ptr(), buffer.len()) };
    (len, buffer)
}

fn bind_down(handle: AmmHandle, index: u32) -> u8 {
    // SAFETY: bind_down takes plain values
    unsafe { TestHost::keybinds_table().bind_down.unwrap_or_else(|| panic!("bind_down"))(handle, index) }
}

#[test]
fn chord_text_gives_the_persons_chord_of_a_descriptor_bind() {
    let host = TestHost::new();
    let freecam = TestClient::register(&host, Descriptors::FREECAM, TestMesh::triangle());

    let (len, buffer) = chord_text(freecam.handle(), TOGGLE, 32);

    assert_eq!(&buffer[..len as usize], b"Ctrl+P");
}

#[test]
fn chord_text_with_a_buffer_too_small_writes_nothing_and_gives_the_length() {
    let host = TestHost::new();
    let freecam = TestClient::register(&host, Descriptors::FREECAM, TestMesh::triangle());

    let (len, buffer) = chord_text(freecam.handle(), TOGGLE, 3);

    assert_eq!(len, 6);
    assert_eq!(buffer, vec![0; 3]);
}

#[test]
fn chord_text_of_an_index_past_the_list_or_a_stale_handle_is_0() {
    let host = TestHost::new();
    let freecam = TestClient::register(&host, Descriptors::FREECAM, TestMesh::triangle());
    let stale = AmmHandle { slot: freecam.handle().slot, generation: freecam.handle().generation.wrapping_add(1) };

    assert_eq!(chord_text(freecam.handle(), 999, 32).0, 0);
    assert_eq!(chord_text(stale, TOGGLE, 32).0, 0);
}

#[test]
fn bind_down_is_1_between_the_press_and_the_release_of_the_chord() {
    let host = TestHost::new();
    let freecam = TestClient::register(&host, Descriptors::FREECAM, TestMesh::triangle());
    assert_eq!(bind_down(freecam.handle(), TOGGLE), 0);

    host.key(KEY_P, true, TestHost::CTRL, 0);
    let held = bind_down(freecam.handle(), TOGGLE);
    host.key(KEY_P, false, TestHost::CTRL, 10);

    assert_eq!(held, 1);
    assert_eq!(bind_down(freecam.handle(), TOGGLE), 0);
    assert_eq!(bind_down(freecam.handle(), 999), 0);
}
