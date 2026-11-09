pub mod capture;
pub mod capture_stream;
pub mod events;
pub mod icon;
pub mod input_query;
pub mod key_output;
pub mod leases;
pub mod locale;
pub mod log;
pub mod menu;
pub mod picture;
pub mod pointer_output;
pub mod register;
pub mod render_size;
pub mod settings;
pub mod table;

pub use capture::{amm_held_by_other, amm_request_screenshot, amm_request_screenshot_layers, amm_texture_budget};
pub use capture_stream::{
    amm_capture_stream_close, amm_capture_stream_free, amm_capture_stream_give_back, amm_capture_stream_lend,
    amm_capture_stream_request, amm_capture_stream_take, amm_present_count,
};
pub use events::amm_poll_events;
pub use icon::amm_set_icon;
pub use input_query::{amm_key_down, amm_panel_open};
pub use key_output::{amm_send_key, amm_watch_keys};
pub use leases::{amm_acquire, amm_holder, amm_release, amm_set_cursor_mode};
pub use locale::amm_locale;
pub use log::amm_log;
pub use menu::amm_register_menu_entry;
pub use picture::{amm_game_picture, amm_set_game_viewport};
pub use pointer_output::amm_send_pointer;
pub use register::{amm_config_document, amm_register, amm_unregister};
pub use render_size::{amm_render_size, amm_set_render_size};
pub use settings::amm_set_setting;
pub use table::amm_api_for;
