use amm_core::services::input::FakeKeys;
use amm_core::structs::input::FakeKeyOp;
use amm_core::structs::registry::ModIndex;

const VK_SHIFT: u32 = 0x10;
const VK_W: u32 = 0x57;

fn ops(keys: &mut FakeKeys) -> Vec<FakeKeyOp> {
    let mut out = Vec::new();
    keys.take_ops(&mut out);
    out
}

#[test]
fn hold_then_release_queues_down_then_up() {
    let mut keys = FakeKeys::new();
    let m = ModIndex(0);

    assert!(keys.hold(m, VK_SHIFT));
    assert!(keys.is_held(VK_SHIFT));
    assert!(keys.release(m, VK_SHIFT));
    assert!(!keys.is_held(VK_SHIFT));

    assert_eq!(ops(&mut keys), vec![FakeKeyOp { vk: VK_SHIFT, down: true }, FakeKeyOp { vk: VK_SHIFT, down: false }]);
    assert!(ops(&mut keys).is_empty());
}

#[test]
fn holding_a_held_key_or_releasing_a_free_one_queues_nothing() {
    let mut keys = FakeKeys::new();
    let m = ModIndex(0);

    assert!(keys.hold(m, VK_W));
    assert!(!keys.hold(m, VK_W));
    assert!(!keys.release(m, VK_SHIFT));
    assert!(!keys.hold(m, 256));

    assert_eq!(ops(&mut keys), vec![FakeKeyOp { vk: VK_W, down: true }]);
}

#[test]
fn a_key_two_mods_hold_goes_up_with_the_last_release() {
    let mut keys = FakeKeys::new();
    let (a, b) = (ModIndex(0), ModIndex(1));

    keys.hold(a, VK_W);
    keys.hold(b, VK_W);
    keys.release(a, VK_W);
    assert!(keys.is_held(VK_W));
    keys.release(b, VK_W);

    assert_eq!(ops(&mut keys), vec![FakeKeyOp { vk: VK_W, down: true }, FakeKeyOp { vk: VK_W, down: false }]);
}

#[test]
fn release_all_reports_every_mod() {
    let mut keys = FakeKeys::new();
    let (a, b) = (ModIndex(0), ModIndex(3));
    keys.hold(a, VK_SHIFT);
    keys.hold(b, VK_W);
    ops(&mut keys);

    let lost = keys.release_all();

    assert_eq!(lost.iter().collect::<Vec<_>>(), vec![a, b]);
    assert!(!keys.is_held(VK_SHIFT) && !keys.is_held(VK_W));
    let mut queued = ops(&mut keys);
    queued.sort_by_key(|op| op.vk);
    assert_eq!(queued, vec![FakeKeyOp { vk: VK_SHIFT, down: false }, FakeKeyOp { vk: VK_W, down: false }]);
}

#[test]
fn release_mod_keeps_the_keys_of_other_mods() {
    let mut keys = FakeKeys::new();
    let (a, b) = (ModIndex(0), ModIndex(1));
    keys.hold(a, VK_SHIFT);
    keys.hold(b, VK_W);
    ops(&mut keys);

    assert_eq!(keys.release_mod(a).iter().collect::<Vec<_>>(), vec![a]);
    assert!(keys.release_mod(a).is_empty());

    assert!(keys.is_held(VK_W));
    assert_eq!(ops(&mut keys), vec![FakeKeyOp { vk: VK_SHIFT, down: false }]);
}

#[test]
fn release_all_except_keeps_the_kept_mods_keys() {
    let mut keys = FakeKeys::new();
    let (kept, other) = (ModIndex(0), ModIndex(1));
    keys.hold(kept, VK_W);
    keys.hold(other, VK_W);
    keys.hold(other, VK_SHIFT);
    ops(&mut keys);

    let lost = keys.release_all_except(kept);

    assert!(lost.contains(other) && !lost.contains(kept));
    assert!(keys.is_held(VK_W));
    assert!(!keys.is_held(VK_SHIFT));
    assert_eq!(ops(&mut keys), vec![FakeKeyOp { vk: VK_SHIFT, down: false }]);
}

#[test]
fn a_game_up_is_queued_and_refused_once_max_ops_wait() {
    let mut keys = FakeKeys::new();
    let m = ModIndex(0);

    assert!(keys.queue_game_up(VK_W));
    assert_eq!(ops(&mut keys), vec![FakeKeyOp { vk: VK_W, down: false }]);

    for vk in 0..FakeKeys::MAX_OPS as u32 {
        keys.hold(m, vk);
    }
    assert!(!keys.queue_game_up(VK_W));
    assert_eq!(keys.pending(), FakeKeys::MAX_OPS);
}
