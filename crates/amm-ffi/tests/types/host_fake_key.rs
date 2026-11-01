use std::mem::{align_of, offset_of, size_of};

use amm_ffi::types::AmmHostFakeKey;

#[test]
fn amm_host_fake_key_is_8_bytes() {
    assert_eq!((size_of::<AmmHostFakeKey>(), align_of::<AmmHostFakeKey>()), (8, 4));
    assert_eq!(offset_of!(AmmHostFakeKey, vk), 0);
    assert_eq!(offset_of!(AmmHostFakeKey, down), 4);
}
