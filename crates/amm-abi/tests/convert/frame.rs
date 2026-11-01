use amm_abi::types::{AmmCursorShape, AmmFrameOutput, AmmMeshView, AmmStatus};
use amm_core::structs::frame::{CursorShape, FrameReplyData};
use amm_core::structs::mesh::MeshBuffers;
use amm_core::traits::AbiSized;

#[test]
fn every_cursor_byte_in_range_converts_to_the_matching_core_shape() {
    for (byte, shape) in
        [(0u8, CursorShape::Default), (1, CursorShape::Arrow), (2, CursorShape::Hand), (3, CursorShape::Text)]
    {
        let abi = AmmCursorShape::try_from(byte);
        assert_eq!(abi.map(CursorShape::from), Ok(shape));
    }
}

#[test]
fn a_bad_cursor_byte_is_rejected() {
    assert_eq!(AmmCursorShape::try_from(4), Err(AmmStatus::InvalidArgument));
    assert_eq!(AmmCursorShape::try_from(255), Err(AmmStatus::InvalidArgument));
}

fn output(cursor: u8, wants_pointer: u8, wants_keyboard: u8) -> AmmFrameOutput {
    let mesh = AmmMeshView::from(&MeshBuffers::new());
    AmmFrameOutput {
        mesh,
        wants_pointer,
        wants_keyboard,
        cursor,
        layer_rect: [1.0, 2.0, 3.0, 4.0],
        ..AmmFrameOutput::blank()
    }
}

#[test]
fn frame_output_with_size_below_base_is_invalid() {
    let unset = AmmFrameOutput { size: 0, ..output(2, 1, 0) };
    let short = AmmFrameOutput { size: (AmmFrameOutput::BASE_SIZE - 1) as u32, ..output(2, 1, 0) };

    assert_eq!(FrameReplyData::try_from(&unset), Err(AmmStatus::InvalidArgument));
    assert_eq!(FrameReplyData::try_from(&short), Err(AmmStatus::InvalidArgument));
}

#[test]
fn frame_output_with_good_values_converts() {
    let reply = FrameReplyData::try_from(&output(2, 1, 0));
    assert_eq!(
        reply,
        Ok(FrameReplyData {
            cursor: CursorShape::Hand,
            wants_pointer: true,
            wants_keyboard: false,
            layer_rect: [1.0, 2.0, 3.0, 4.0]
        })
    );
}

#[test]
fn frame_output_with_bad_wants_or_cursor_is_rejected() {
    assert_eq!(FrameReplyData::try_from(&output(9, 0, 0)), Err(AmmStatus::InvalidArgument));
    assert_eq!(FrameReplyData::try_from(&output(0, 2, 0)), Err(AmmStatus::InvalidArgument));
    assert_eq!(FrameReplyData::try_from(&output(0, 0, 255)), Err(AmmStatus::InvalidArgument));
}
