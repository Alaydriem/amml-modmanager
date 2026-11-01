use std::path::Path;

use amm_core::config::ManagerConfig;
use amm_core::errors::CaptureError;
use amm_core::services::capture::CaptureStreamBook;
use amm_core::services::manager::ManagerService;
use amm_core::structs::capture::{CaptureStreamFailure, CaptureStreamFault, CaptureStreamRequest};
use amm_core::structs::descriptor::Capability;
use amm_core::structs::registry::ModIndex;
use amm_core::structs::render::RenderSize;

const RGBA: u32 = 28;
const SIZE: RenderSize = RenderSize { width: 2, height: 1 };

fn overlay_mod(manager: &mut ManagerService, id: &str) -> ModIndex {
    let text = format!(
        "namespace = \"test\"\nname = \"{id}\"\ndisplay_name = \"{id}\"\nversion = \"0.1.0\"\nauthor = \"A\"\nconfig_path = \"config.hcl\"\n\
         claim \"Overlay\" {{\n  mode = \"Shared\"\n  reason = \"r\"\n}}\n"
    );
    let dir = Path::new("mods").join(id);
    let index = manager.register(&text, None, &dir).unwrap_or_else(|error| panic!("{id}: {error}")).0;
    manager.acquire(index, Capability::Overlay, 0).unwrap_or_else(|error| panic!("{id}: {error}"));
    index
}

fn offer(book: &mut CaptureStreamBook, manager: &ManagerService) -> Vec<u32> {
    let mut ids = [0; CaptureStreamBook::RING];
    let count = book.offer(manager.arbiter(), SIZE, &mut ids);
    ids[..count].to_vec()
}

// the overlay fills the buffer of `id` with `byte` and reports it in `format`
fn fill(book: &mut CaptureStreamBook, id: u32, byte: u8, format: u32) {
    let (buffer, _) = book.buffer(id, 2, 1).unwrap_or_else(|| panic!("buffer for {id}"));
    buffer.fill(byte);
    assert!(book.report(id, true, format));
}

fn take_tag(book: &mut CaptureStreamBook, index: ModIndex) -> Option<Result<u64, CaptureStreamFailure>> {
    book.take(index).map(|taken| taken.map(|frame| frame.tag))
}

#[test]
fn frames_come_back_in_request_order_when_a_later_copy_is_read_first() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    for tag in [10, 11] {
        assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(tag)), Ok(()));
    }
    let ids = offer(&mut book, &manager);

    fill(&mut book, ids[1], 2, RGBA);
    let before_first = take_tag(&mut book, index);
    fill(&mut book, ids[0], 1, RGBA);
    let first = book.take(index).map(|taken| taken.map(|frame| (frame.tag, frame.pixels.clone())));
    let second = book.take(index).map(|taken| taken.map(|frame| (frame.tag, frame.pixels.clone())));

    assert_eq!(ids.len(), 2);
    assert_eq!(before_first, None);
    assert_eq!(first, Some(Ok((10, vec![1; 8]))));
    assert_eq!(second, Some(Ok((11, vec![2; 8]))));
    assert_eq!(take_tag(&mut book, index), None);
}

#[test]
fn a_request_is_offered_once_by_the_next_present() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(1)), Ok(()));

    let pending = book.needs_present();
    let first = offer(&mut book, &manager);
    let again = offer(&mut book, &manager);

    assert!(pending);
    assert_eq!(first.len(), 1);
    assert!(again.is_empty());
}

#[test]
fn the_ring_is_bounded_and_a_full_ring_is_busy() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();

    for tag in 0..CaptureStreamBook::RING as u64 {
        assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(tag)), Ok(()));
    }

    assert_eq!(book.free_slots(index), 0);
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(99)), Err(CaptureError::Busy));
    assert_eq!(offer(&mut book, &manager).len(), CaptureStreamBook::RING);
}

#[test]
fn a_buffer_asked_for_and_not_reported_by_the_next_present_fails_the_frame() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(5)), Ok(()));
    let ids = offer(&mut book, &manager);
    assert!(book.buffer(ids[0], 2, 1).is_some());

    book.advance(SIZE);

    assert_eq!(take_tag(&mut book, index), Some(Err(CaptureStreamFailure::new(5, CaptureStreamFault::CopyFailed))));
    assert!(!book.report(ids[0], true, RGBA));
}

#[test]
fn a_pending_frame_of_a_mod_that_lost_its_lease_fails_not_permitted() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(4)), Ok(()));

    manager.release(index, Capability::Overlay);
    let offered = offer(&mut book, &manager);

    assert!(offered.is_empty());
    assert_eq!(take_tag(&mut book, index), Some(Err(CaptureStreamFailure::new(4, CaptureStreamFault::NotPermitted))));
}

#[test]
fn a_taken_frame_reuses_its_buffer_for_the_next_frame_of_the_same_size() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    let mut seen = Vec::new();

    for tag in 0..3u64 {
        assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(tag)), Ok(()));
        let id = offer(&mut book, &manager)[0];
        fill(&mut book, id, tag as u8, RGBA);
        book.advance(SIZE);
        let pointer = book.take(index).and_then(Result::ok).map(|frame| frame.pixels.as_ptr() as usize);
        seen.push(pointer.unwrap_or_else(|| panic!("frame {tag}")));
    }

    // two buffers take turns: the lent one and the one in the ring, so no frame allocates after the first two
    assert_eq!(seen[0], seen[2]);
    assert_ne!(seen[0], seen[1]);
}

#[test]
fn close_frees_every_buffer_tells_the_overlay_once_and_a_new_stream_starts() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    for tag in 0..2u64 {
        assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(tag)), Ok(()));
    }
    let ids = offer(&mut book, &manager);
    fill(&mut book, ids[0], 1, RGBA);
    fill(&mut book, ids[1], 2, RGBA);
    book.advance(SIZE);
    assert!(book.take(index).is_some());
    let held_before = book.held_bytes();

    book.close(index);
    let held_after = book.held_bytes();
    let release = (book.take_release(), book.take_release());
    let free = book.free_slots(index);
    let restarted = book.request(manager.arbiter(), index, CaptureStreamRequest::new(9));

    assert!(held_before >= 16);
    assert_eq!(held_after, 0);
    assert_eq!(release, (true, false));
    assert_eq!(free, CaptureStreamBook::RING);
    assert_eq!(restarted, Ok(()));
}

#[test]
fn close_while_a_copy_is_in_flight_drops_it_and_frees_its_buffer() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(1)), Ok(()));
    let id = offer(&mut book, &manager)[0];
    assert!(book.buffer(id, 2, 1).is_some());

    book.close(index);

    assert_eq!(book.held_bytes(), 0);
    assert!(!book.report(id, true, RGBA));
    assert!(book.needs_present(), "the overlay is told to drop its stream textures");
    assert!(book.take_release());
    assert!(!book.needs_present());
}

#[test]
fn a_close_of_another_mod_leaves_the_stream_in_use() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let other = overlay_mod(&mut manager, "other");
    let mut book = CaptureStreamBook::new();
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(1)), Ok(()));
    let id = offer(&mut book, &manager)[0];
    fill(&mut book, id, 3, RGBA);

    book.close(other);

    assert!(!book.take_release());
    assert_eq!(book.take(index).and_then(Result::ok).map(|frame| frame.tag), Some(1));
}

#[test]
fn a_size_change_fails_the_copies_in_flight_as_resized_in_order() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    let bigger = RenderSize::new(4, 2);
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(1)), Ok(()));
    assert_eq!(offer(&mut book, &manager).len(), 1);
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(2)), Ok(()));

    book.advance(bigger);
    let mut ids = [0; CaptureStreamBook::RING];
    let count = book.offer(manager.arbiter(), bigger, &mut ids);
    let (buffer, pitch) = book.buffer(ids[0], 4, 2).unwrap_or_else(|| panic!("buffer at the new size"));
    buffer.fill(5);
    assert!(book.report(ids[0], true, RGBA));

    assert_eq!(count, 1);
    assert_eq!(pitch, 16);
    assert_eq!(take_tag(&mut book, index), Some(Err(CaptureStreamFailure::new(1, CaptureStreamFault::Resized))));
    let second = book.take(index).and_then(Result::ok).map(|frame| (frame.tag, frame.width, frame.height));
    assert_eq!(second, Some((2, 4, 2)));
}

#[test]
fn a_device_loss_with_no_size_change_stays_device_lost() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(1)), Ok(()));
    offer(&mut book, &manager);

    book.device_lost();
    let before_present = take_tag(&mut book, index);
    book.advance(SIZE);

    assert_eq!(before_present, None);
    assert_eq!(take_tag(&mut book, index), Some(Err(CaptureStreamFailure::new(1, CaptureStreamFault::DeviceLost))));
}

#[test]
fn a_frame_keeps_the_number_of_the_present_that_offered_it() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(1)), Ok(()));
    let mut ids = [0; CaptureStreamBook::RING];
    let count = book.offer_at(manager.arbiter(), SIZE, 41, &mut ids);
    fill(&mut book, ids[0], 1, RGBA);
    let first = book.take(index).map(|taken| taken.map(|frame| (frame.tag, frame.present_seq)));

    assert_eq!(count, 1);
    assert_eq!(first, Some(Ok((1, 41))));

    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(2)), Ok(()));
    let ids = offer(&mut book, &manager);
    fill(&mut book, ids[0], 2, RGBA);
    let second = book.take(index).map(|taken| taken.map(|frame| (frame.tag, frame.present_seq)));

    assert_eq!(second, Some(Ok((2, 0))), "offer without a number gives 0");
}

fn offer_at(book: &mut CaptureStreamBook, manager: &ManagerService, present_seq: u64) -> Vec<u32> {
    let mut ids = [0; CaptureStreamBook::RING];
    let count = book.offer_at(manager.arbiter(), SIZE, present_seq, &mut ids);
    ids[..count].to_vec()
}

#[test]
fn named_request_waits_for_its_present() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::at(1, 12)), Ok(()));

    let at_ten = offer_at(&mut book, &manager, 10);
    let at_eleven = offer_at(&mut book, &manager, 11);
    let pending = book.needs_present();
    let at_twelve = offer_at(&mut book, &manager, 12);
    fill(&mut book, at_twelve[0], 1, RGBA);
    let taken = book.take(index).map(|taken| taken.map(|frame| (frame.tag, frame.present_seq)));

    assert_eq!((at_ten.len(), at_eleven.len(), at_twelve.len()), (0, 0, 1));
    assert!(pending);
    assert_eq!(taken, Some(Ok((1, 12))));
}

#[test]
fn named_request_past_its_present_fails_missed() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::at(7, 12)), Ok(()));

    let offered = offer_at(&mut book, &manager, 13);

    assert!(offered.is_empty());
    assert_eq!(take_tag(&mut book, index), Some(Err(CaptureStreamFailure::new(7, CaptureStreamFault::Missed))));
    assert_eq!(book.free_slots(index), CaptureStreamBook::RING);
}

#[test]
fn request_without_number_takes_the_next_present() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(1)), Ok(()));

    let ids = offer_at(&mut book, &manager, 57);
    fill(&mut book, ids[0], 1, RGBA);

    let taken = book.take(index).map(|taken| taken.map(|frame| (frame.tag, frame.present_seq)));
    assert_eq!(taken, Some(Ok((1, 57))));
}

#[test]
fn named_requests_keep_request_order() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    for (tag, target) in [(1, 12), (2, 13)] {
        assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::at(tag, target)), Ok(()));
    }

    let at_twelve = offer_at(&mut book, &manager, 12);
    let at_thirteen = offer_at(&mut book, &manager, 13);
    fill(&mut book, at_thirteen[0], 2, RGBA);
    let before_first = take_tag(&mut book, index);
    fill(&mut book, at_twelve[0], 1, RGBA);
    let first = book.take(index).map(|taken| taken.map(|frame| (frame.tag, frame.present_seq)));
    let second = book.take(index).map(|taken| taken.map(|frame| (frame.tag, frame.present_seq)));

    assert_eq!((at_twelve.len(), at_thirteen.len()), (1, 1));
    assert_eq!(before_first, None);
    assert_eq!(first, Some(Ok((1, 12))));
    assert_eq!(second, Some(Ok((2, 13))));
}

// one frame from request to lend: the pixels hold `byte`
fn lend_one(
    book: &mut CaptureStreamBook,
    manager: &ManagerService,
    index: ModIndex,
    tag: u64,
    byte: u8,
) -> (u64, usize) {
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(tag)), Ok(()));
    let ids = offer(book, manager);
    fill(book, ids[0], byte, RGBA);
    let lent = book.lend(index).unwrap_or_else(|| panic!("lend {tag}")).unwrap_or_else(|failure| panic!("{failure:?}"));
    assert_eq!(lent.pixels, vec![byte; 8]);
    (lent.lend_id, lent.pixels.as_ptr() as usize)
}

#[test]
fn lent_buffer_is_not_reused_until_given_back() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();

    let (first_id, first_ptr) = lend_one(&mut book, &manager, index, 1, 1);
    let (second_id, second_ptr) = lend_one(&mut book, &manager, index, 2, 2);
    // the first lent pixels are still what the mod was told, after the second frame was written
    let first_still = book.lent_pixels(first_id).map(<[u8]>::to_vec);
    assert_eq!(book.take(index).map(|taken| taken.is_ok()), None);

    assert_ne!(first_ptr, second_ptr);
    assert_ne!(first_id, second_id);
    assert_eq!(first_still, Some(vec![1u8; 8]));
    assert_eq!(book.lent_pixels(second_id).map(<[u8]>::to_vec), Some(vec![2u8; 8]));
}

#[test]
fn lend_past_max_is_busy() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    let mut ids = Vec::new();
    for tag in 0..CaptureStreamBook::MAX_LENT as u64 {
        ids.push(lend_one(&mut book, &manager, index, tag + 1, 1).0);
    }
    assert_eq!(book.request(manager.arbiter(), index, CaptureStreamRequest::new(99)), Ok(()));
    let offered = offer(&mut book, &manager);
    fill(&mut book, offered[0], 9, RGBA);

    let refused = book.lend(index).map(|lent| lent.is_ok());
    assert!(book.give_back(index, ids[0]));
    let accepted = book.lend(index).map(|lent| lent.map(|frame| frame.tag));

    assert_eq!(CaptureStreamBook::MAX_LENT, 8);
    assert_eq!(refused, None);
    assert_eq!(accepted, Some(Ok(99)));
}

#[test]
fn given_back_buffer_is_reused_without_allocating() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    let (first, _) = lend_one(&mut book, &manager, index, 1, 1);
    assert!(book.give_back(index, first));
    let flat = book.held_bytes();

    for tag in 2..22u64 {
        let (id, _) = lend_one(&mut book, &manager, index, tag, tag as u8);
        assert!(book.give_back(index, id));
        assert_eq!(book.held_bytes(), flat, "tag {tag}");
    }
}

#[test]
fn give_back_after_close_frees_the_buffer() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    let (id, _) = lend_one(&mut book, &manager, index, 1, 1);

    book.close(index);
    let kept = book.lent_pixels(id).map(<[u8]>::to_vec);
    let held_after_close = book.held_bytes();
    let given = book.give_back(index, id);

    assert_eq!(kept, Some(vec![1u8; 8]));
    assert!(held_after_close >= 8);
    assert!(given);
    assert_eq!(book.held_bytes(), 0);
    assert!(!book.give_back(index, id));
}

#[test]
fn give_back_of_an_unknown_id_is_refused() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let other = overlay_mod(&mut manager, "other");
    let mut book = CaptureStreamBook::new();
    let (id, _) = lend_one(&mut book, &manager, index, 1, 1);

    assert!(!book.give_back(index, 0));
    assert!(!book.give_back(index, id + 100));
    assert!(!book.give_back(other, id));
    assert!(book.give_back(index, id));
    assert!(!book.give_back(index, id));
}

#[test]
fn unregister_frees_lent_buffers() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let index = overlay_mod(&mut manager, "render");
    let mut book = CaptureStreamBook::new();
    let (id, _) = lend_one(&mut book, &manager, index, 1, 1);
    lend_one(&mut book, &manager, index, 2, 2);

    book.unregister(index);

    assert_eq!(book.held_bytes(), 0);
    assert!(book.lent_pixels(id).is_none());
    assert!(!book.give_back(index, id));
}
