use amm_abi::types::{AmmCaptureStreamFault, AmmCaptureStreamFrame, AmmStatus};
use amm_client::test_support::FakeTable;
use amm_core::errors::LinkError;
use amm_core::structs::capture::{CaptureStreamFailure, CaptureStreamFault, CaptureStreamFrame};
use amm_core::traits::AbiSized;

fn good(tag: u64, byte: u8) -> (AmmStatus, AmmCaptureStreamFrame, Vec<u8>) {
    let frame = AmmCaptureStreamFrame {
        tag,
        width: 2,
        height: 1,
        row_pitch: 8,
        format: 28,
        present_seq: 100 + tag,
        ..AmmCaptureStreamFrame::blank()
    };
    (AmmStatus::Ok, frame, vec![byte; 8])
}

#[test]
fn request_sends_the_tag_and_maps_a_full_ring_to_busy() {
    let link = FakeTable::link_without_frame();
    let stream = link.capture_stream();

    let first = stream.request(41);
    FakeTable::with(|state| state.stream_request_status = AmmStatus::Busy);
    let full = stream.request(42);

    assert_eq!(first, Ok(()));
    assert_eq!(full, Err(LinkError::Status(AmmStatus::Busy as i32)));
    assert_eq!(FakeTable::with(|state| state.stream_requests.clone()), vec![41, 42]);
}

#[test]
fn try_take_copies_each_frame_in_the_order_the_manager_gives_them() {
    let link = FakeTable::link_without_frame();
    let stream = link.capture_stream();
    FakeTable::with(|state| state.stream_takes.extend([good(1, 7), good(2, 9)]));

    let first = stream.try_take();
    let second = stream.try_take();

    let expected = |tag, byte| CaptureStreamFrame {
        tag,
        width: 2,
        height: 1,
        row_pitch: 8,
        format: 28,
        pixels: vec![byte; 8],
        present_seq: 100 + tag,
    };
    assert_eq!(first, Ok(Some(expected(1, 7))));
    assert_eq!(second, Ok(Some(expected(2, 9))));
}

#[test]
fn try_take_with_nothing_ready_is_none() {
    let link = FakeTable::link_without_frame();

    assert_eq!(link.capture_stream().try_take(), Ok(None));
}

#[test]
fn try_take_into_reuses_the_callers_buffer() {
    let link = FakeTable::link_without_frame();
    let stream = link.capture_stream();
    FakeTable::with(|state| state.stream_takes.push_back(good(3, 5)));
    let mut frame = CaptureStreamFrame { pixels: Vec::with_capacity(64), ..CaptureStreamFrame::default() };
    let before = frame.pixels.as_ptr();

    let taken = stream.try_take_into(&mut frame);

    assert_eq!(taken, Ok(true));
    assert_eq!((frame.tag, frame.pixels.as_slice()), (3, [5u8; 8].as_slice()));
    assert_eq!(frame.pixels.as_ptr(), before);
}

#[test]
fn a_failed_frame_is_an_error_that_names_its_tag_and_fault() {
    let link = FakeTable::link_without_frame();
    let stream = link.capture_stream();
    let failed = AmmCaptureStreamFrame {
        tag: 6,
        format: 10,
        fault: AmmCaptureStreamFault::UnsupportedFormat as u8,
        ..AmmCaptureStreamFrame::blank()
    };
    FakeTable::with(|state| state.stream_takes.push_back((AmmStatus::Ok, failed, Vec::new())));

    let taken = stream.try_take();

    let fault = CaptureStreamFault::UnsupportedFormat { format: 10 };
    assert_eq!(taken, Err(LinkError::StreamFrameFailed(CaptureStreamFailure::new(6, fault))));
}

#[test]
fn free_slots_reads_the_managers_count() {
    let link = FakeTable::link_without_frame();
    FakeTable::with(|state| state.stream_free = 3);

    assert_eq!(link.capture_stream().free_slots(), 3);
}

#[test]
fn the_stream_on_an_old_manager_is_unsupported_with_no_free_slot() {
    let mut api = FakeTable::api();
    api.capture_stream_request = None;
    api.capture_stream_take = None;
    api.capture_stream_free = None;
    api.capture_stream_close = None;
    let link = FakeTable::link_without_frame_on(api);
    let stream = link.capture_stream();

    assert_eq!(stream.request(1), Err(LinkError::Status(AmmStatus::Unsupported as i32)));
    assert_eq!(stream.try_take(), Err(LinkError::Status(AmmStatus::Unsupported as i32)));
    assert_eq!(stream.free_slots(), 0);
}

#[test]
fn close_and_drop_each_close_the_stream_once() {
    let link = FakeTable::link_without_frame();

    link.capture_stream().close();
    let after_close = FakeTable::with(|state| state.stream_closes);
    drop(link.capture_stream());
    let after_drop = FakeTable::with(|state| state.stream_closes);

    assert_eq!((after_close, after_drop), (1, 2));
}

#[test]
fn a_resized_frame_is_its_own_fault() {
    let link = FakeTable::link_without_frame();
    let stream = link.capture_stream();
    let failed =
        AmmCaptureStreamFrame { tag: 4, fault: AmmCaptureStreamFault::Resized as u8, ..AmmCaptureStreamFrame::blank() };
    FakeTable::with(|state| state.stream_takes.push_back((AmmStatus::Ok, failed, Vec::new())));

    let taken = stream.try_take();

    assert_eq!(taken, Err(LinkError::StreamFrameFailed(CaptureStreamFailure::new(4, CaptureStreamFault::Resized))));
}

#[test]
fn request_at_sends_the_present_number() {
    let link = FakeTable::link_without_frame();
    let stream = link.capture_stream();

    let named = stream.request_at(5, 12);
    let next = stream.request(6);

    assert_eq!((named, next), (Ok(()), Ok(())));
    assert_eq!(FakeTable::with(|state| state.stream_requests.clone()), vec![5, 6]);
    assert_eq!(FakeTable::with(|state| state.stream_present_seqs.clone()), vec![12, 0]);
}

#[test]
fn missed_frame_is_stream_frame_failed() {
    let link = FakeTable::link_without_frame();
    let stream = link.capture_stream();
    let missed =
        AmmCaptureStreamFrame { tag: 9, fault: AmmCaptureStreamFault::Missed as u8, ..AmmCaptureStreamFrame::blank() };
    FakeTable::with(|state| state.stream_takes.push_back((AmmStatus::Ok, missed, Vec::new())));

    let taken = stream.try_take();

    assert_eq!(taken, Err(LinkError::StreamFrameFailed(CaptureStreamFailure::new(9, CaptureStreamFault::Missed))));
}

#[test]
fn lend_reads_pixels_without_copy() {
    let link = FakeTable::link_without_frame();
    let stream = link.capture_stream();
    let (status, mut frame, pixels) = good(5, 3);
    frame.lend_id = 77;
    FakeTable::with(|state| state.stream_takes.push_back((status, frame, pixels)));

    let lent = stream.lend().unwrap_or_else(|error| panic!("{error:?}")).unwrap_or_else(|| panic!("a lent frame"));

    assert_eq!((lent.tag, lent.width, lent.height, lent.row_pitch, lent.format), (5, 2, 1, 8, 28));
    assert_eq!((lent.present_seq, lent.lend_id), (105, 77));
    // SAFETY: no give_back ran, and the link and stream are alive and registered
    let pixels = unsafe { lent.pixels() };
    assert_eq!(pixels, [3u8; 8].as_slice());
    assert_eq!(pixels.as_ptr() as usize, FakeTable::with(|state| state.stream_lent_buffers[0].as_ptr() as usize));
}

#[test]
fn lend_with_nothing_ready_is_none_and_a_failed_frame_is_its_error() {
    let link = FakeTable::link_without_frame();
    let stream = link.capture_stream();
    let failed =
        AmmCaptureStreamFrame { tag: 9, fault: AmmCaptureStreamFault::Missed as u8, ..AmmCaptureStreamFrame::blank() };

    let nothing = stream.lend().map(|lent| lent.is_some());
    FakeTable::with(|state| state.stream_takes.push_back((AmmStatus::Ok, failed, Vec::new())));
    let missed = stream.lend().map(|lent| lent.is_some());

    assert_eq!(nothing, Ok(false));
    assert_eq!(missed, Err(LinkError::StreamFrameFailed(CaptureStreamFailure::new(9, CaptureStreamFault::Missed))));
}

#[test]
fn give_back_sends_the_id() {
    let link = FakeTable::link_without_frame();
    let stream = link.capture_stream();

    let first = stream.give_back(41);
    FakeTable::with(|state| state.stream_give_back_status = AmmStatus::InvalidArgument);
    let unknown = stream.give_back(42);

    assert_eq!(first, Ok(()));
    assert_eq!(unknown, Err(LinkError::Status(AmmStatus::InvalidArgument as i32)));
    assert_eq!(FakeTable::with(|state| state.stream_give_backs.clone()), vec![41, 42]);
}

#[test]
fn lend_on_a_manager_without_the_slot_is_unsupported() {
    let mut api = FakeTable::api();
    api.capture_stream_lend = None;
    api.capture_stream_give_back = None;
    let link = FakeTable::link_without_frame_on(api);
    let stream = link.capture_stream();

    assert_eq!(stream.lend().map(|lent| lent.is_some()), Err(LinkError::Status(AmmStatus::Unsupported as i32)));
    assert_eq!(stream.give_back(1), Err(LinkError::Status(AmmStatus::Unsupported as i32)));
}
