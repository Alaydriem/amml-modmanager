use amm_abi::types::{AmmScreenshotLayers, AmmStatus};
use amm_core::structs::capture::ScreenshotLayers;

#[test]
fn a_layers_byte_other_than_0_or_1_is_rejected() {
    let read = |byte: u8| AmmScreenshotLayers::try_from(byte).map(ScreenshotLayers::from);

    assert_eq!(read(0), Ok(ScreenshotLayers::GameOnly));
    assert_eq!(read(1), Ok(ScreenshotLayers::WithModLayers));
    for byte in [2, 3, 0x80, 0xFF] {
        assert_eq!(read(byte), Err(AmmStatus::InvalidArgument));
    }
}
