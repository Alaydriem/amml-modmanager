use amml_modmanager::services::GamePointer;

#[test]
fn the_host_gets_the_buffer_point_and_client_size_and_its_point_is_used_as_it_is() {
    // the back buffer is twice the client area
    let mapped = GamePointer::map((100, 50), (640, 360), (1280, 720), |x, y, client_w, client_h| {
        assert_eq!((x, y, client_w, client_h), (200.0, 100.0, 640, 360));
        Some((320.4, 179.6))
    });
    assert_eq!(mapped, Some((320, 180)));
}

#[test]
fn outside_the_viewport_or_before_a_bind_nothing_moves() {
    assert_eq!(GamePointer::map((1, 1), (640, 360), (640, 360), |_, _, _, _| None), None);
    assert_eq!(GamePointer::map((1, 1), (640, 360), (0, 0), |_, _, _, _| Some((5.0, 5.0))), None);
}

#[test]
fn a_minimised_window_moves_nothing() {
    assert_eq!(GamePointer::map((1, 1), (0, 0), (640, 360), |_, _, _, _| Some((5.0, 5.0))), None);
}
