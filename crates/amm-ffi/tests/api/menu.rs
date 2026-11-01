use amm_abi::types::{AmmEventKind, AmmStatus};

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

fn register_entry(client: &TestClient, label: &str, mark: &str, screens: u8) -> Result<u16, AmmStatus> {
    let register = TestHost::api().register_menu_entry.unwrap_or_else(|| panic!("register_menu_entry"));
    let mut id = 0u16;
    // SAFETY: the label, the mark and the out id live for the call
    let status =
        unsafe { register(client.handle(), label.as_ptr(), label.len(), mark.as_ptr(), mark.len(), screens, &mut id) };
    if status == AmmStatus::Ok { Ok(id) } else { Err(status) }
}

#[test]
fn register_menu_entry_gives_ids_from_one() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("menu-ids"), TestMesh::triangle());

    let first = register_entry(&client, "Dejavu", "DV", 1);
    let second = register_entry(&client, "Capture", "CP", 2);

    assert_eq!(first, Ok(1));
    assert_eq!(second, Ok(2));
}

#[test]
fn register_menu_entry_refuses_bad_text_and_a_null_out_pointer() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("menu-bad"), TestMesh::triangle());
    let register = TestHost::api().register_menu_entry.unwrap_or_else(|| panic!("register_menu_entry"));

    let long_mark = register_entry(&client, "Dejavu", "DEJA", 1);
    let no_screen = register_entry(&client, "Dejavu", "DV", 0);
    // SAFETY: the label and the mark live for the call; the out pointer is null on purpose
    let null_out =
        unsafe { register(client.handle(), "Dejavu".as_ptr(), 6, "DV".as_ptr(), 2, 1, std::ptr::null_mut()) };

    assert_eq!(long_mark, Err(AmmStatus::InvalidArgument));
    assert_eq!(no_screen, Err(AmmStatus::InvalidArgument));
    assert_eq!(null_out, AmmStatus::NullPointer);
}

#[test]
fn old_client_never_sees_menu_entry_events() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("menu-old"), TestMesh::triangle());
    let _ = client.poll();

    for now in 0..3 {
        host.frame_with(now, 1);
    }

    assert!(client.poll().iter().all(|event| event.kind != AmmEventKind::MenuEntryClicked));
}
