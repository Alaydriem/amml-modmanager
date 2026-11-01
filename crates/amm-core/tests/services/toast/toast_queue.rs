use amm_core::services::toast::ToastQueue;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::registry::ModIndex;
use amm_core::structs::toast::ToastKind;

const RATE_MS: u64 = 2000;

fn denied(capability: Capability, requester: u8) -> ToastKind {
    ToastKind::Denied { capability, holder: ModIndex(0), requester: ModIndex(requester) }
}

#[test]
fn toast_for_one_capability_and_requester_is_rate_limited_to_one_per_2000_ms() {
    let mut queue = ToastQueue::new(8, RATE_MS);

    assert!(queue.push(denied(Capability::Camera, 1), 0));
    assert!(!queue.push(denied(Capability::Camera, 1), 1999));
    assert!(queue.push(denied(Capability::Camera, 1), 2000));
    assert!(queue.push(denied(Capability::Camera, 2), 2001));
    assert!(queue.push(denied(Capability::Input, 1), 2002));
    assert_eq!(queue.len(), 4);
}

#[test]
fn blocked_toasts_share_the_limit_per_capability_and_mod() {
    let mut queue = ToastQueue::new(8, RATE_MS);
    let blocked = |action| ToastKind::Blocked {
        capability: Capability::Camera,
        holder: ModIndex(0),
        mod_index: ModIndex(1),
        action,
    };

    assert!(queue.push(blocked(0), 0));
    assert!(!queue.push(blocked(3), 500));
    assert!(queue.push(denied(Capability::Input, 1), 500));
}

#[test]
fn kinds_without_a_pair_are_never_rate_limited() {
    let mut queue = ToastQueue::new(8, RATE_MS);

    assert!(queue.push(ToastKind::SaveFailed, 0));
    assert!(queue.push(ToastKind::SaveFailed, 1));
    assert!(queue.push(ToastKind::FrameDisabled(ModIndex(2)), 2));
    assert!(queue.push(ToastKind::FrameDisabled(ModIndex(2)), 3));
}

#[test]
fn toast_queue_past_capacity_drops_the_oldest() {
    let mut queue = ToastQueue::new(8, RATE_MS);

    for n in 0..9u8 {
        assert!(queue.push(ToastKind::FrameDisabled(ModIndex(n)), u64::from(n)));
    }

    assert_eq!(queue.len(), 8);
    let kept: Vec<ToastKind> = queue.visible(9, 10_000).map(|toast| toast.kind).collect();
    let expected: Vec<ToastKind> = (1..9u8).map(|n| ToastKind::FrameDisabled(ModIndex(n))).collect();
    assert_eq!(kept, expected);
}

#[test]
fn visible_hides_toasts_older_than_the_ttl() {
    let mut queue = ToastQueue::new(8, RATE_MS);
    queue.push(ToastKind::SaveFailed, 0);
    queue.push(ToastKind::FrameDisabled(ModIndex(1)), 4000);

    let shown: Vec<ToastKind> = queue.visible(5000, 3000).map(|toast| toast.kind).collect();

    assert_eq!(shown, vec![ToastKind::FrameDisabled(ModIndex(1))]);
}
