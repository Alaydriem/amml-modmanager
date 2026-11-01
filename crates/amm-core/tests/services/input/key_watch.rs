use amm_core::services::input::KeyWatch;
use amm_core::structs::registry::ModIndex;

const VK_SHIFT: u32 = 0x10;
const VK_W: u32 = 0x57;

#[test]
fn set_replaces_previous_set() {
    let mut watch = KeyWatch::new();
    let m = ModIndex(2);

    watch.set(m, &[VK_SHIFT, VK_W]);
    watch.set(m, &[VK_W]);

    assert!(watch.watchers(VK_SHIFT).is_empty());
    assert_eq!(watch.watchers(VK_W).iter().collect::<Vec<_>>(), vec![m]);
}

#[test]
fn forget_clears_mod() {
    let mut watch = KeyWatch::new();
    let (a, b) = (ModIndex(0), ModIndex(1));
    watch.set(a, &[VK_W]);
    watch.set(b, &[VK_W]);

    watch.forget(a);

    assert_eq!(watch.watchers(VK_W).iter().collect::<Vec<_>>(), vec![b]);
}

#[test]
fn watchers_of_unwatched_key_is_empty() {
    let mut watch = KeyWatch::new();
    watch.set(ModIndex(0), &[VK_W]);

    assert!(watch.watchers(VK_SHIFT).is_empty());
    assert!(watch.watchers(4096).is_empty());
}

#[test]
fn an_empty_slice_clears_the_set() {
    let mut watch = KeyWatch::new();
    watch.set(ModIndex(0), &[VK_W]);

    watch.set(ModIndex(0), &[]);

    assert!(watch.watchers(VK_W).is_empty());
}
