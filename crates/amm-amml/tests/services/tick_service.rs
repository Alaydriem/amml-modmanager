use amml_modmanager::services::{ManagerState, TickService};

use crate::support::recording_calls::{Call, RecordingCalls};

#[test]
fn leaving_a_world_tells_the_host_and_forgets_the_client() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let tick = TickService::new();
    tick.tick(calls, 0x1000);
    tick.world(calls, true);
    assert_eq!(tick.client(), 0x1000);

    tick.world(calls, false);

    assert_eq!(tick.client(), 0);
    assert_eq!(calls.take(), vec![Call::HostTick(0), Call::HostWorld(true), Call::HostWorld(false)]);
}

#[test]
fn a_tick_counts_as_recent_only_inside_the_window() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let tick = TickService::new();
    assert!(!tick.ticked_within(0, 1000), "no tick yet");
    calls.set_now(5000);
    tick.tick(calls, 0x1000);

    assert!(tick.ticked_within(6000, 1000));
    assert!(!tick.ticked_within(6001, 1000));
}
