use amm_core::structs::input::GameViewport;

const WINDOW: (f32, f32) = (1920.0, 1080.0);

fn viewport() -> GameViewport {
    GameViewport::new([100.0, 50.0, 1060.0, 590.0]).unwrap_or_else(|| panic!("a valid rect"))
}

#[test]
fn the_corners_and_the_centre_of_the_viewport_map_to_the_corners_and_the_centre_of_the_window() {
    let viewport = viewport();

    assert_eq!(viewport.map((100.0, 50.0), WINDOW), Some((0.0, 0.0)));
    assert_eq!(viewport.map((580.0, 320.0), WINDOW), Some((960.0, 540.0)));
    assert_eq!(viewport.map((1059.0, 589.0), WINDOW), Some((1918.0, 1078.0)));
}

#[test]
fn a_16_10_window_in_a_viewport_of_its_own_aspect_maps_the_centre() {
    let viewport = GameViewport::new([0.0, 0.0, 960.0, 600.0]).unwrap_or_else(|| panic!("a valid rect"));

    assert_eq!(viewport.map((480.0, 300.0), (1920.0, 1200.0)), Some((960.0, 600.0)));
}

#[test]
fn a_point_outside_the_viewport_does_not_map() {
    let viewport = viewport();

    assert_eq!(viewport.map((99.0, 300.0), WINDOW), None);
    assert_eq!(viewport.map((1060.0, 300.0), WINDOW), None);
    assert_eq!(viewport.map((500.0, 590.0), WINDOW), None);
}

#[test]
fn mapping_the_same_point_twice_gives_the_same_point() {
    let viewport = viewport();

    assert_eq!(viewport.map((333.0, 222.0), WINDOW), viewport.map((333.0, 222.0), WINDOW));
}

#[test]
fn an_empty_or_nan_rect_is_no_viewport() {
    assert_eq!(GameViewport::new([10.0, 10.0, 10.0, 20.0]), None);
    assert_eq!(GameViewport::new([f32::NAN, 0.0, 10.0, 10.0]), None);
}
