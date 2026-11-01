use amm_core::config::Budgets;

#[test]
fn defaults_are_inside_every_bound() {
    let (clamped, fields) = Budgets::default().clamped();

    assert_eq!(clamped, Budgets::default());
    assert!(fields.is_empty());
}

#[test]
fn values_past_a_maximum_are_clamped_and_named() {
    let huge = Budgets {
        event_queue: 100_000_000,
        io_queue: 1_000_000,
        vertices_per_mod: u32::MAX,
        upload_bytes_per_frame: u64::MAX,
        texture_deltas_per_frame: usize::MAX,
        ..Budgets::default()
    };

    let (clamped, fields) = huge.clamped();

    assert_eq!(clamped.event_queue, Budgets::MAX_EVENT_QUEUE);
    assert_eq!(clamped.io_queue, Budgets::MAX_IO_QUEUE);
    assert_eq!(clamped.vertices_per_mod, Budgets::MAX_VERTICES_PER_MOD);
    assert_eq!(clamped.upload_bytes_per_frame, Budgets::MAX_UPLOAD_BYTES_PER_FRAME);
    assert_eq!(clamped.texture_deltas_per_frame, Budgets::MAX_TEXTURE_DELTAS_PER_FRAME);
    assert_eq!(
        fields,
        vec!["event_queue", "vertices_per_mod", "upload_bytes_per_frame", "io_queue", "texture_deltas_per_frame"]
    );
}

#[test]
fn values_under_a_minimum_are_raised() {
    let zero = Budgets {
        max_mods: 0,
        event_queue: 0,
        toasts: 0,
        io_queue: 0,
        frame_failure_limit: 0,
        texture_deltas_per_frame: 0,
        textures_per_mod: 0,
        max_textures_per_mod: 0,
        ..Budgets::default()
    };

    let (clamped, fields) = zero.clamped();

    assert_eq!(clamped.max_mods, 1);
    assert_eq!(clamped.event_queue, Budgets::MIN_EVENT_QUEUE);
    assert_eq!(clamped.toasts, 1);
    assert_eq!(clamped.io_queue, 1);
    assert_eq!(clamped.frame_failure_limit, 1);
    assert_eq!(clamped.texture_deltas_per_frame, 1);
    assert_eq!(clamped.textures_per_mod, 1);
    assert_eq!(clamped.max_textures_per_mod, 1);
    assert_eq!(fields.len(), 8);
}

#[test]
fn the_texture_maxima_are_clamped_to_their_hard_maximum() {
    let huge = Budgets {
        texture_bytes_per_mod: u64::MAX,
        max_texture_bytes_per_mod: u64::MAX,
        max_textures_per_mod: usize::MAX,
        ..Budgets::default()
    };

    let (clamped, fields) = huge.clamped();

    assert_eq!(clamped.texture_bytes_per_mod, 1024 * 1024 * 1024);
    assert_eq!(clamped.max_texture_bytes_per_mod, 2048 * 1024 * 1024);
    assert_eq!(clamped.max_textures_per_mod, 8128);
    assert_eq!(fields, vec!["texture_bytes_per_mod", "max_texture_bytes_per_mod", "max_textures_per_mod"]);
}

#[test]
fn the_texture_maxima_default_to_1024_mib_and_4096_textures() {
    let budgets = Budgets::default();

    assert_eq!(budgets.max_texture_bytes_per_mod, 1024 * 1024 * 1024);
    assert_eq!(budgets.max_textures_per_mod, 4096);
}
