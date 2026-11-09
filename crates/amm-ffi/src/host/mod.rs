pub mod capture;
pub mod capture_stream;
pub mod frame;
pub mod gamepad;
pub mod input;
pub mod lifecycle;
pub mod logger;
pub mod player_turn;
pub mod render_size;
pub mod tick;
pub mod world;

pub use capture::{amm_host_capture_buffer, amm_host_capture_done};
pub use capture_stream::{amm_host_stream_buffer, amm_host_stream_done};
pub use frame::{amm_host_frame, amm_host_textures_lost};
pub use gamepad::amm_host_blocks_gamepad;
pub use input::{
    amm_host_char, amm_host_focus_lost, amm_host_game_point, amm_host_key, amm_host_pointer,
    amm_host_release_fake_keys, amm_host_take_fake_keys, amm_host_take_fake_pointer, amm_host_take_focus_loss_owed,
};
pub use lifecycle::{amm_host_create, amm_host_destroy};
pub use logger::{amm_host_log, amm_host_set_console};
pub use player_turn::amm_host_blocks_player_turn;
pub use render_size::{
    amm_host_render_size, amm_host_render_size_shutdown, amm_host_window_changed, amm_host_window_size,
};
pub use tick::amm_host_tick;
pub use world::amm_host_world;
