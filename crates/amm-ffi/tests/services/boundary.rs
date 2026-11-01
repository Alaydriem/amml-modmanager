use amm_abi::types::AmmStatus;
use amm_core::traits::AbiSized;
use amm_ffi::services::Boundary;

// A sized struct whose minor 0 ends after `a`; `b` came in a later minor
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
struct Probe {
    size: u32,
    a: u32,
    b: u64,
}

impl AbiSized for Probe {
    const BASE_SIZE: usize = 8;

    fn blank() -> Self {
        Self { size: size_of::<Self>() as u32, a: 0, b: 0 }
    }
}

const SENTINEL: u8 = 0xAB;

// room for a Probe and 8 more bytes, so a write past the declared size shows up in the sentinel bytes
#[repr(C, align(8))]
struct Room([u8; 24]);

impl Room {
    fn declaring(size: u32) -> Room {
        let mut bytes = [SENTINEL; 24];
        bytes[..4].copy_from_slice(&size.to_le_bytes());
        Room(bytes)
    }

    fn as_probe(&mut self) -> *mut Probe {
        self.0.as_mut_ptr().cast::<Probe>()
    }
}

#[test]
fn sized_input_fills_missing_fields_with_blank() {
    let older = Probe { size: 8, a: 5, b: 99 };

    // SAFETY: older is a live Probe for the call
    let read = unsafe { Boundary::sized_input(&older) };

    assert_eq!(read, Ok(Probe { size: 8, a: 5, b: 0 }));
}

#[test]
fn sized_input_never_reads_past_its_own_size() {
    let newer = Probe { size: u32::MAX, a: 5, b: 7 };

    // SAFETY: newer is a live Probe for the call; the declared size is larger than the struct on purpose
    let read = unsafe { Boundary::sized_input(&newer) };

    assert_eq!(read, Ok(Probe { size: 16, a: 5, b: 7 }));
}

#[test]
fn sized_input_refuses_size_below_base() {
    for size in [0, 4, 7] {
        let short = Probe { size, a: 5, b: 7 };

        // SAFETY: short is a live Probe for the call
        let read = unsafe { Boundary::sized_input(&short) };

        assert_eq!(read, Err(AmmStatus::InvalidArgument), "size {size}");
    }
}

#[test]
fn sized_input_refuses_null_and_unaligned() {
    let room = Room::declaring(16);
    let unaligned = room.0[1..].as_ptr().cast::<Probe>();

    // SAFETY: null is refused before any read
    let null = unsafe { Boundary::sized_input::<Probe>(std::ptr::null()) };
    // SAFETY: an unaligned pointer is refused before any read
    let skewed = unsafe { Boundary::sized_input(unaligned) };

    assert_eq!(null, Err(AmmStatus::NullPointer));
    assert_eq!(skewed, Err(AmmStatus::NullPointer));
}

#[test]
fn sized_output_writes_only_the_callers_size() {
    let mut room = Room::declaring(8);
    let value = Probe { size: 16, a: 7, b: 9 };

    // SAFETY: room holds 24 writable bytes, more than the 8 it declares
    let written = unsafe { Boundary::sized_output(room.as_probe(), &value) };

    assert_eq!(written, Ok(()));
    assert_eq!(room.0[..4], 8u32.to_le_bytes());
    assert_eq!(room.0[4..8], 7u32.to_le_bytes());
    assert!(room.0[8..].iter().all(|byte| *byte == SENTINEL));
}

#[test]
fn sized_output_refuses_size_below_base_and_writes_nothing() {
    let mut room = Room::declaring(4);
    let value = Probe { size: 16, a: 7, b: 9 };

    // SAFETY: room holds 24 writable bytes
    let written = unsafe { Boundary::sized_output(room.as_probe(), &value) };

    assert_eq!(written, Err(AmmStatus::InvalidArgument));
    assert_eq!(room.0[..4], 4u32.to_le_bytes());
    assert!(room.0[4..].iter().all(|byte| *byte == SENTINEL));
}

#[test]
fn sized_output_refuses_null() {
    let value = Probe::blank();

    // SAFETY: null is refused before any write
    let written = unsafe { Boundary::sized_output::<Probe>(std::ptr::null_mut(), &value) };

    assert_eq!(written, Err(AmmStatus::NullPointer));
}
