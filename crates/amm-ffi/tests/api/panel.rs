use crate::support::TestHost;

fn panel_open() -> u8 {
    // SAFETY: panel_open takes no arguments
    unsafe { TestHost::core_table().panel_open.unwrap_or_else(|| panic!("panel_open"))() }
}

#[test]
fn panel_open_follows_the_panel_and_is_0_without_a_host() {
    assert_eq!(panel_open(), 0);
    let host = TestHost::new();
    assert_eq!(panel_open(), 0);

    host.with_runtime(|runtime| runtime.manager_mut().set_panel_open(true));

    assert_eq!(panel_open(), 1);
}

#[test]
fn panel_open_answers_from_another_thread_while_the_host_lock_is_held() {
    let host = TestHost::new();
    host.with_runtime(|runtime| runtime.manager_mut().set_panel_open(true));

    let mut seen = 0;
    host.with_runtime(|_| {
        seen = std::thread::scope(|scope| scope.spawn(panel_open).join().unwrap_or(0));
    });

    assert_eq!(seen, 1);
}
