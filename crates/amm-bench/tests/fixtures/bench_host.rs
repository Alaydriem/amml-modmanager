use amm_bench::fixtures::BenchHost;
use amm_ffi::types::AmmHostLayer;

#[test]
fn overlay_clients_each_get_a_layer_with_all_their_vertices() {
    let host = BenchHost::with_overlay_clients(4, 10_000);

    let frame = host.frame(16);
    // SAFETY: the host keeps layer_count layers at layers until the next frame
    let layers = unsafe { std::slice::from_raw_parts(frame.layers, frame.layer_count as usize) };

    assert_eq!(layers.len(), 4);
    assert!(layers.iter().all(|layer| layer.mesh.vertex_count == 10_000));
}

#[test]
fn a_denied_camera_leaves_one_toast_and_the_panel_layer() {
    let host = BenchHost::with_denied_camera();

    let frame = host.frame(1_000);
    // SAFETY: the host keeps layer_count layers at layers until the next frame
    let layers = unsafe { std::slice::from_raw_parts(frame.layers, frame.layer_count as usize) };

    assert_eq!(layers.iter().map(|layer| layer.owner).collect::<Vec<_>>(), vec![AmmHostLayer::MANAGER]);
}

#[test]
fn a_forced_size_sends_its_size_message_and_then_keeps_quiet() {
    let host = BenchHost::with_forced_size(3840, 2160);

    let first = host.frame_at(16, 3840, 2160);
    let second = host.frame_at(32, 3840, 2160);

    assert_eq!((first.size_message_width, first.size_message_height), (3840, 2160));
    assert_eq!((second.size_message_width, second.size_message_height), (0, 0));
}
