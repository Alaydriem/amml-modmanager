use amm_client::link::EventDrain;
use amm_core::structs::event::{ModEvent, ReleaseReason};
use amml_api::ShotId;
use amml_api::amml_abi::{
    AmmlLostReason, AmmlReleaseReason, InjectedInputReleased, InputLeaseLost, InputLeaseResumed, InputLeaseSuspended,
    ScreenshotTaken,
};
use amml_api::testing::FakeGame;

const LEASE: u64 = 0x0000_0001_0000_0002;

// The loader never delivers these events cancelled (they cannot be cancelled), so the header is set here, as a
// listener ahead of the link or a later loader could.
fn cancelled<E: amml_api::Event>(mut event: E) -> E {
    // SAFETY: by the `Event` contract `E` starts with its header, and the borrow of `event` covers it
    let header = unsafe { &mut *std::ptr::from_mut(&mut event).cast::<amml_api::amml_abi::AmmlEventHeader>() };
    header.cancelled = 1;
    event
}

fn released(reason: AmmlReleaseReason) -> InjectedInputReleased {
    let header = FakeGame::header::<InjectedInputReleased>();
    InjectedInputReleased { header, lease: LEASE, reason, input: InjectedInputReleased::ALL, code: 0, _reserved: 0 }
}

#[test]
fn cancelled_input_events_are_not_passed_on() {
    let release = cancelled(released(AmmlReleaseReason::FOCUS_LOST));
    let suspended = cancelled(InputLeaseSuspended { header: FakeGame::header::<InputLeaseSuspended>(), lease: LEASE });
    let resumed = cancelled(InputLeaseResumed { header: FakeGame::header::<InputLeaseResumed>(), lease: LEASE });
    let lost = cancelled(InputLeaseLost {
        header: FakeGame::header::<InputLeaseLost>(),
        lease: LEASE,
        reason: AmmlLostReason::PREEMPTED,
    });
    let taken =
        cancelled(ScreenshotTaken { header: FakeGame::header::<ScreenshotTaken>(), id: 1, ok: 1, _reserved: [0; 7] });

    assert_eq!(EventDrain::released(&release, Some(LEASE)), None);
    assert_eq!(EventDrain::suspended(&suspended, Some(LEASE)), None);
    assert_eq!(EventDrain::resumed(&resumed, Some(LEASE)), None);
    assert_eq!(EventDrain::lost(&lost, Some(LEASE)), None);
    assert_eq!(EventDrain::taken(&taken), None);
}

#[test]
fn input_events_pass_only_for_the_links_lease() {
    let release = released(AmmlReleaseReason::REPLACED);

    assert_eq!(
        EventDrain::released(&release, Some(LEASE)),
        Some(ModEvent::FakeKeysReleased { reason: ReleaseReason::Replaced })
    );
    assert_eq!(EventDrain::released(&release, Some(LEASE + 1)), None);
    assert_eq!(EventDrain::released(&release, None), None);
}

#[test]
fn an_unknown_release_reason_keeps_its_value_and_a_failed_shot_is_not_ok() {
    let release = released(AmmlReleaseReason(42));
    let taken = ScreenshotTaken { header: FakeGame::header::<ScreenshotTaken>(), id: 9, ok: 0, _reserved: [0; 7] };

    assert_eq!(
        EventDrain::released(&release, Some(LEASE)),
        Some(ModEvent::FakeKeysReleased { reason: ReleaseReason::Other(42) })
    );
    assert_eq!(EventDrain::taken(&taken), Some(ModEvent::ScreenshotTaken { id: ShotId(9), ok: false }));
}
