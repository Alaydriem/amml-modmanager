use amm_core::services::abi::AbiPrefix;

const BASE: usize = 32;
const FULL: usize = 48;

#[test]
fn abi_prefix_refuses_a_size_below_base() {
    assert_eq!(AbiPrefix::len(0, BASE, FULL), None);
    assert_eq!(AbiPrefix::len(31, BASE, FULL), None);
}

#[test]
fn abi_prefix_reads_a_shorter_struct() {
    assert_eq!(AbiPrefix::len(40, BASE, FULL), Some(40));
}

#[test]
fn abi_prefix_reads_an_equal_struct() {
    assert_eq!(AbiPrefix::len(48, BASE, FULL), Some(48));
}

#[test]
fn abi_prefix_clamps_a_longer_struct() {
    assert_eq!(AbiPrefix::len(4096, BASE, FULL), Some(48));
    assert_eq!(AbiPrefix::len(u32::MAX, BASE, FULL), Some(48));
}
