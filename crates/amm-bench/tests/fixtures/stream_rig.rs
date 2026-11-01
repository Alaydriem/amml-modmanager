use amm_bench::fixtures::StreamRig;
use amm_core::services::capture::CaptureStreamBook;

#[test]
fn each_4k_frame_is_copied_reported_and_taken_so_the_ring_never_fills() {
    let mut rig = StreamRig::new();

    for _ in 0..CaptureStreamBook::RING * 2 {
        rig.take_one();
    }

    assert_eq!(rig.taken(), CaptureStreamBook::RING * 2);
}
