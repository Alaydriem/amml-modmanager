use amm_abi::types::{AMM_ABI_MAJOR, AMM_ABI_MINOR, AmmApi};

use super::{
    amm_acquire, amm_capture_stream_close, amm_capture_stream_free, amm_capture_stream_give_back,
    amm_capture_stream_lend, amm_capture_stream_request, amm_capture_stream_take, amm_config_document,
    amm_game_picture, amm_held_by_other, amm_holder, amm_key_down, amm_locale, amm_log, amm_poll_events,
    amm_present_count, amm_register, amm_register_menu_entry, amm_release, amm_render_size, amm_request_screenshot,
    amm_request_screenshot_layers, amm_send_key, amm_set_cursor_mode, amm_set_game_viewport, amm_set_icon,
    amm_set_render_size, amm_set_setting, amm_texture_budget, amm_unregister, amm_watch_keys,
};

static API: AmmApi = AmmApi {
    size: size_of::<AmmApi>() as u32,
    abi_major: AMM_ABI_MAJOR,
    abi_minor: AMM_ABI_MINOR,
    register: Some(amm_register),
    unregister: Some(amm_unregister),
    config_document: Some(amm_config_document),
    acquire: Some(amm_acquire),
    release: Some(amm_release),
    holder: Some(amm_holder),
    poll_events: Some(amm_poll_events),
    key_down: Some(amm_key_down),
    set_setting: Some(amm_set_setting),
    set_cursor_mode: Some(amm_set_cursor_mode),
    log: Some(amm_log),
    request_screenshot: Some(amm_request_screenshot),
    held_by_other: Some(amm_held_by_other),
    texture_budget: Some(amm_texture_budget),
    request_screenshot_layers: Some(amm_request_screenshot_layers),
    register_menu_entry: Some(amm_register_menu_entry),
    locale: Some(amm_locale),
    set_icon: Some(amm_set_icon),
    game_picture: Some(amm_game_picture),
    set_game_viewport: Some(amm_set_game_viewport),
    set_render_size: Some(amm_set_render_size),
    render_size: Some(amm_render_size),
    capture_stream_request: Some(amm_capture_stream_request),
    capture_stream_take: Some(amm_capture_stream_take),
    capture_stream_free: Some(amm_capture_stream_free),
    capture_stream_close: Some(amm_capture_stream_close),
    present_count: Some(amm_present_count),
    capture_stream_lend: Some(amm_capture_stream_lend),
    capture_stream_give_back: Some(amm_capture_stream_give_back),
    watch_keys: Some(amm_watch_keys),
    send_key: Some(amm_send_key),
};

/// The function table for `abi_major`, or null when this manager does not speak that major version. The
/// export `amm_get_api` of the AMML mod returns this. The table is static and lives as long as the DLL.
pub extern "C" fn amm_api_for(abi_major: u32) -> *const AmmApi {
    if abi_major == AMM_ABI_MAJOR { &API } else { std::ptr::null() }
}
